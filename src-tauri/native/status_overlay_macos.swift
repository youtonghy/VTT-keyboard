import AppKit
import AVFoundation
import Foundation

private enum DictationStatus: Int32 {
    case recording = 0
    case transcribing = 1
    case completed = 2
    case error = 3

    init(rawStatus: Int32) {
        self = DictationStatus(rawValue: rawStatus) ?? .recording
    }

    var accentColor: NSColor {
        switch self {
        case .recording:
            return NSColor(red: 239.0 / 255.0, green: 68.0 / 255.0, blue: 68.0 / 255.0, alpha: 1.0)
        case .transcribing:
            return NSColor(red: 59.0 / 255.0, green: 130.0 / 255.0, blue: 246.0 / 255.0, alpha: 1.0)
        case .completed:
            return NSColor(red: 16.0 / 255.0, green: 185.0 / 255.0, blue: 129.0 / 255.0, alpha: 1.0)
        case .error:
            return NSColor(red: 245.0 / 255.0, green: 158.0 / 255.0, blue: 11.0 / 255.0, alpha: 1.0)
        }
    }
}

private struct OverlaySnapshot {
    let status: DictationStatus
    let text: String
}

private final class StatusOverlayView: NSView {
    override var isOpaque: Bool {
        false
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        NSColor.clear.setFill()
        dirtyRect.fill()

        let snapshot = StatusOverlayController.shared.snapshot()
        let bounds = self.bounds
        let radius = bounds.height / 2.0
        let backgroundPath = NSBezierPath(roundedRect: bounds, xRadius: radius, yRadius: radius)

        NSColor(calibratedWhite: 0.08, alpha: 0.88).setFill()
        backgroundPath.fill()

        NSColor.white.withAlphaComponent(0.12).setStroke()
        backgroundPath.lineWidth = 1.0
        backgroundPath.stroke()

        let dotSize: CGFloat = 8.0
        let dotRect = NSRect(
            x: 16.0,
            y: (bounds.height - dotSize) / 2.0,
            width: dotSize,
            height: dotSize
        )
        snapshot.status.accentColor.setFill()
        NSBezierPath(ovalIn: dotRect).fill()

        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.alignment = .left
        paragraphStyle.lineBreakMode = .byTruncatingTail

        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 13.0, weight: .semibold),
            .foregroundColor: NSColor.white,
            .paragraphStyle: paragraphStyle,
        ]
        let textRect = NSRect(
            x: 34.0,
            y: (bounds.height - 18.0) / 2.0,
            width: max(0.0, bounds.width - 50.0),
            height: 18.0
        )
        snapshot.text.draw(in: textRect, withAttributes: attributes)
    }
}

private final class StatusOverlayController {
    static let shared = StatusOverlayController()

    private let lock = NSLock()
    private var currentStatus = DictationStatus.recording
    private var currentText = ""
    private var panel: NSPanel?
    private var overlayView: StatusOverlayView?
    private var visible = false
    private var lastFrame = NSRect.zero

    private let minWidth: CGFloat = 168.0
    private let maxWidth: CGFloat = 320.0
    private let height: CGFloat = 40.0
    private let bottomMargin: CGFloat = 48.0

    func initialize() -> Bool {
        if Thread.isMainThread {
            return createPanelIfNeeded()
        }

        var initialized = false
        DispatchQueue.main.sync {
            initialized = createPanelIfNeeded()
        }
        return initialized
    }

    func show(status: Int32, text: String) {
        let nextStatus = DictationStatus(rawStatus: status)
        var changed = false

        lock.lock()
        if currentStatus != nextStatus || currentText != text {
            currentStatus = nextStatus
            currentText = text
            changed = true
        }
        lock.unlock()

        performOnMain {
            self.updatePanelFrame(display: changed)
            if changed {
                self.overlayView?.needsDisplay = true
            }
            self.panel?.orderFrontRegardless()
            self.visible = true
        }
    }

    func hide() {
        performOnMain {
            guard self.visible else {
                return
            }
            self.panel?.orderOut(nil)
            self.visible = false
        }
    }

    func cleanup() {
        performOnMain {
            self.panel?.close()
            self.panel = nil
            self.overlayView = nil
            self.visible = false
        }
    }

    func snapshot() -> OverlaySnapshot {
        lock.lock()
        let snapshot = OverlaySnapshot(status: currentStatus, text: currentText)
        lock.unlock()
        return snapshot
    }

    private func createPanelIfNeeded() -> Bool {
        if panel != nil {
            return true
        }

        NSApplication.shared

        let frame = frameForCurrentText()
        let newPanel = NSPanel(
            contentRect: frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        newPanel.backgroundColor = .clear
        newPanel.isOpaque = false
        newPanel.hasShadow = true
        newPanel.ignoresMouseEvents = true
        newPanel.hidesOnDeactivate = false
        newPanel.isReleasedWhenClosed = false
        newPanel.level = .floating
        newPanel.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]

        let view = StatusOverlayView(frame: NSRect(origin: .zero, size: frame.size))
        view.wantsLayer = true
        view.layer?.masksToBounds = false
        newPanel.contentView = view

        panel = newPanel
        overlayView = view
        lastFrame = frame
        return true
    }

    private func updatePanelFrame(display: Bool) {
        guard let panel else {
            return
        }
        let frame = frameForCurrentText()
        if frame != lastFrame {
            panel.setFrame(frame, display: display)
            overlayView?.frame = NSRect(origin: .zero, size: frame.size)
            lastFrame = frame
        }
    }

    private func frameForCurrentText() -> NSRect {
        let size = CGSize(width: widthForCurrentText(), height: height)
        let screenFrame = NSScreen.main?.visibleFrame ?? NSRect(x: 0.0, y: 0.0, width: 1440.0, height: 900.0)
        let origin = CGPoint(
            x: screenFrame.origin.x + (screenFrame.width - size.width) / 2.0,
            y: screenFrame.origin.y + bottomMargin
        )
        return NSRect(origin: origin, size: size)
    }

    private func widthForCurrentText() -> CGFloat {
        let text = snapshot().text as NSString
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 13.0, weight: .semibold),
        ]
        let textWidth = ceil(text.size(withAttributes: attributes).width)
        return min(maxWidth, max(minWidth, textWidth + 68.0))
    }

    private func performOnMain(_ block: @escaping () -> Void) {
        if Thread.isMainThread {
            block()
        } else {
            DispatchQueue.main.async(execute: block)
        }
    }
}

@_cdecl("status_overlay_init")
public func status_overlay_init() -> Int32 {
    StatusOverlayController.shared.initialize() ? 0 : -1
}

@_cdecl("status_overlay_show")
public func status_overlay_show(_ status: Int32, _ textPointer: UnsafePointer<CChar>?) {
    let text = textPointer.map { String(cString: $0) } ?? ""
    StatusOverlayController.shared.show(status: status, text: text)
}

@_cdecl("status_overlay_hide")
public func status_overlay_hide() {
    StatusOverlayController.shared.hide()
}

@_cdecl("status_overlay_cleanup")
public func status_overlay_cleanup() {
    StatusOverlayController.shared.cleanup()
}

@_cdecl("macos_microphone_permission_status_code")
public func macos_microphone_permission_status_code() -> Int32 {
    switch AVCaptureDevice.authorizationStatus(for: .audio) {
    case .notDetermined:
        return 0
    case .restricted:
        return 1
    case .denied:
        return 2
    case .authorized:
        return 3
    @unknown default:
        return -1
    }
}
