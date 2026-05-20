import ApplicationServices
import AppKit
import AVFoundation
import Foundation

private let commandKeyMask = CGEventFlags.maskCommand
private let virtualKeyCodeV: CGKeyCode = 0x09

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
    private let dotSize: CGFloat = 8.0
    private let horizontalPadding: CGFloat = 14.0
    private let contentGap: CGFloat = 8.0
    private let font = NSFont.systemFont(ofSize: 13.0, weight: .semibold)

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

        let displayText = snapshot.text.trimmingCharacters(in: .whitespacesAndNewlines)
        let text = displayText.isEmpty ? " " : displayText
        let attributes: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: NSColor.white,
        ]
        let textWidth = min(
            ceil((text as NSString).size(withAttributes: attributes).width),
            max(0.0, bounds.width - horizontalPadding * 2.0 - dotSize - contentGap)
        )
        let contentWidth = dotSize + contentGap + textWidth
        let contentX = max(horizontalPadding, (bounds.width - contentWidth) / 2.0)
        let dotRect = NSRect(
            x: contentX,
            y: (bounds.height - dotSize) / 2.0,
            width: dotSize,
            height: dotSize
        )
        snapshot.status.accentColor.setFill()
        NSBezierPath(ovalIn: dotRect).fill()

        let textRect = NSRect(
            x: dotRect.maxX + contentGap,
            y: (bounds.height - 18.0) / 2.0,
            width: textWidth,
            height: 18.0
        )
        text.draw(in: textRect, withAttributes: attributes)
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

    private let maxWidth: CGFloat = 420.0
    private let height: CGFloat = 40.0
    private let bottomMargin: CGFloat = 48.0
    private let windowLevel = NSWindow.Level.statusBar
    private let collectionBehavior: NSWindow.CollectionBehavior = [
        .canJoinAllSpaces,
        .fullScreenAuxiliary,
        .stationary,
        .ignoresCycle,
    ]
    private let dotSize: CGFloat = 8.0
    private let horizontalPadding: CGFloat = 14.0
    private let contentGap: CGFloat = 8.0
    private let font = NSFont.systemFont(ofSize: 13.0, weight: .semibold)

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
            guard self.createPanelIfNeeded() else {
                return
            }
            self.updatePanelFrame(display: changed)
            if changed {
                self.overlayView?.needsDisplay = true
            }
            self.presentPanel()
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
        newPanel.level = windowLevel
        newPanel.collectionBehavior = collectionBehavior

        let view = StatusOverlayView(frame: NSRect(origin: .zero, size: frame.size))
        view.wantsLayer = true
        view.layer?.masksToBounds = false
        newPanel.contentView = view

        panel = newPanel
        overlayView = view
        lastFrame = frame
        return true
    }

    private func presentPanel() {
        guard let panel else {
            return
        }

        panel.level = windowLevel
        panel.collectionBehavior = collectionBehavior
        panel.orderFrontRegardless()
        panel.displayIfNeeded()

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { [weak self] in
            guard let self, self.visible, let panel = self.panel else {
                return
            }
            self.updatePanelFrame(display: true)
            panel.level = self.windowLevel
            panel.collectionBehavior = self.collectionBehavior
            panel.orderFrontRegardless()
            panel.displayIfNeeded()
        }
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
        let rawText = snapshot().text.trimmingCharacters(in: .whitespacesAndNewlines)
        let text = (rawText.isEmpty ? " " : rawText) as NSString
        let attributes: [NSAttributedString.Key: Any] = [
            .font: font,
        ]
        let textWidth = ceil(text.size(withAttributes: attributes).width)
        let idealWidth = horizontalPadding * 2.0 + dotSize + contentGap + textWidth
        return min(maxWidth, ceil(idealWidth))
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
    microphoneAuthorizationStatusCode()
}

@_cdecl("macos_request_microphone_permission_code")
public func macos_request_microphone_permission_code() -> Int32 {
    if Thread.isMainThread {
        let semaphore = DispatchSemaphore(value: 0)
        var result: Int32 = -1
        DispatchQueue.global(qos: .userInitiated).async {
            result = requestMicrophonePermissionBlocking()
            semaphore.signal()
        }
        semaphore.wait()
        return result
    }

    return requestMicrophonePermissionBlocking()
}

@_cdecl("macos_accessibility_permission_status_code")
public func macos_accessibility_permission_status_code() -> Int32 {
    accessibilityPermissionStatusCode(prompt: false)
}

@_cdecl("macos_request_accessibility_permission_code")
public func macos_request_accessibility_permission_code() -> Int32 {
    accessibilityPermissionStatusCode(prompt: true)
}

@_cdecl("macos_send_paste_shortcut")
public func macos_send_paste_shortcut(_ promptForAccessibility: Int32) -> Int32 {
    guard ensureAccessibilityPermission(prompt: promptForAccessibility != 0) else {
        return 1
    }
    guard let keyDown = CGEvent(
        keyboardEventSource: nil,
        virtualKey: virtualKeyCodeV,
        keyDown: true
    ), let keyUp = CGEvent(
        keyboardEventSource: nil,
        virtualKey: virtualKeyCodeV,
        keyDown: false
    ) else {
        return -1
    }

    keyDown.flags = commandKeyMask
    keyUp.flags = commandKeyMask
    keyDown.post(tap: .cghidEventTap)
    keyUp.post(tap: .cghidEventTap)
    return 0
}

private func ensureAccessibilityPermission(prompt: Bool) -> Bool {
    guard prompt else {
        return AXIsProcessTrusted()
    }

    let options = [
        kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true
    ] as CFDictionary
    return AXIsProcessTrustedWithOptions(options)
}

private func accessibilityPermissionStatusCode(prompt: Bool) -> Int32 {
    if prompt {
        _ = ensureAccessibilityPermission(prompt: true)
        for _ in 0..<12 {
            if AXIsProcessTrusted() {
                return 1
            }
            Thread.sleep(forTimeInterval: 0.15)
        }
    }

    return AXIsProcessTrusted() ? 1 : 0
}

private func requestMicrophonePermissionBlocking() -> Int32 {
    guard #available(macOS 10.14, *) else {
        return 3
    }

    let currentStatus = microphoneAuthorizationStatusCode()
    guard currentStatus == 0 else {
        return currentStatus
    }

    let semaphore = DispatchSemaphore(value: 0)
    var result: Int32 = -1
    AVCaptureDevice.requestAccess(for: .audio) { granted in
        result = granted ? 3 : 2
        semaphore.signal()
    }
    semaphore.wait()
    return result
}

private func microphoneAuthorizationStatusCode() -> Int32 {
    guard #available(macOS 10.14, *) else {
        return 3
    }

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
