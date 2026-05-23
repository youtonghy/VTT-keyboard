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

private enum OverlayActions: Int32 {
    case none = 0
    case retry = 1
    case retryCancel = 2

    init(rawActions: Int32) {
        self = OverlayActions(rawValue: rawActions) ?? .none
    }
}

private enum OverlayAction: Int32 {
    case retry = 0
    case cancel = 1
}

public typealias StatusOverlayActionCallback = @convention(c) (Int32) -> Void
private var statusOverlayActionCallback: StatusOverlayActionCallback?

private struct OverlaySnapshot {
    let status: DictationStatus
    let text: String
    let actions: OverlayActions
}

private final class StatusOverlayView: NSView {
    private let dotSize: CGFloat = 8.0
    private let horizontalPadding: CGFloat = 14.0
    private let contentGap: CGFloat = 8.0
    private let actionGap: CGFloat = 10.0
    private let buttonGap: CGFloat = 6.0
    private let buttonHeight: CGFloat = 26.0
    private let buttonHorizontalPadding: CGFloat = 12.0
    private let font = NSFont.systemFont(ofSize: 13.0, weight: .semibold)
    private let buttonFont = NSFont.systemFont(ofSize: 12.0, weight: .semibold)
    private let retryLabel = "重试"
    private let cancelLabel = "取消"

    override var isOpaque: Bool {
        false
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
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
        let buttonRects = actionButtonRects(in: bounds, snapshot: snapshot)
        let buttonStartX = buttonRects.first?.rect.minX ?? bounds.maxX
        let hasButtons = !buttonRects.isEmpty
        let textLimit = hasButtons
            ? max(0.0, buttonStartX - actionGap - horizontalPadding - dotSize - contentGap)
            : max(0.0, bounds.width - horizontalPadding * 2.0 - dotSize - contentGap)
        let textWidth = min(ceil((text as NSString).size(withAttributes: attributes).width), textLimit)
        let contentWidth = dotSize + contentGap + textWidth
        let contentX = hasButtons
            ? horizontalPadding
            : max(horizontalPadding, (bounds.width - contentWidth) / 2.0)
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

        for item in buttonRects {
            drawButton(label: item.label, rect: item.rect)
        }
    }

    override func mouseUp(with event: NSEvent) {
        let snapshot = StatusOverlayController.shared.snapshot()
        guard snapshot.actions != .none else {
            return
        }
        let point = convert(event.locationInWindow, from: nil)
        for item in actionButtonRects(in: bounds, snapshot: snapshot) {
            if item.rect.contains(point) {
                StatusOverlayController.shared.perform(action: item.action)
                return
            }
        }
    }

    private func drawButton(label: String, rect: NSRect) {
        let path = NSBezierPath(roundedRect: rect, xRadius: 13.0, yRadius: 13.0)
        NSColor.white.withAlphaComponent(0.16).setFill()
        path.fill()
        NSColor.white.withAlphaComponent(0.22).setStroke()
        path.lineWidth = 1.0
        path.stroke()

        let attributes: [NSAttributedString.Key: Any] = [
            .font: buttonFont,
            .foregroundColor: NSColor.white,
        ]
        let textSize = (label as NSString).size(withAttributes: attributes)
        let textRect = NSRect(
            x: rect.midX - textSize.width / 2.0,
            y: rect.midY - textSize.height / 2.0,
            width: textSize.width,
            height: textSize.height
        )
        label.draw(in: textRect, withAttributes: attributes)
    }

    private func actionButtonRects(
        in bounds: NSRect,
        snapshot: OverlaySnapshot
    ) -> [(action: OverlayAction, label: String, rect: NSRect)] {
        let items = actionItems(snapshot.actions)
        guard !items.isEmpty else {
            return []
        }

        var reversed: [(action: OverlayAction, label: String, rect: NSRect)] = []
        var cursorX = bounds.width - horizontalPadding
        for item in items.reversed() {
            let width = buttonWidth(label: item.label)
            let rect = NSRect(
                x: cursorX - width,
                y: (bounds.height - buttonHeight) / 2.0,
                width: width,
                height: buttonHeight
            )
            reversed.append((action: item.action, label: item.label, rect: rect))
            cursorX = rect.minX - buttonGap
        }
        return Array(reversed.reversed())
    }

    private func actionItems(_ actions: OverlayActions) -> [(action: OverlayAction, label: String)] {
        switch actions {
        case .none:
            return []
        case .retry:
            return [(action: .retry, label: retryLabel)]
        case .retryCancel:
            return [
                (action: .retry, label: retryLabel),
                (action: .cancel, label: cancelLabel),
            ]
        }
    }

    private func buttonWidth(label: String) -> CGFloat {
        let attributes: [NSAttributedString.Key: Any] = [.font: buttonFont]
        let textWidth = ceil((label as NSString).size(withAttributes: attributes).width)
        return max(46.0, textWidth + buttonHorizontalPadding * 2.0)
    }
}

private final class StatusOverlayController {
    static let shared = StatusOverlayController()

    private let lock = NSLock()
    private var currentStatus = DictationStatus.recording
    private var currentText = ""
    private var currentActions = OverlayActions.none
    private var panel: NSPanel?
    private var overlayView: StatusOverlayView?
    private var visible = false
    private var lastFrame = NSRect.zero

    private let maxWidth: CGFloat = 420.0
    private let maxActionWidth: CGFloat = 560.0
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
    private let actionGap: CGFloat = 10.0
    private let buttonGap: CGFloat = 6.0
    private let buttonHorizontalPadding: CGFloat = 12.0
    private let font = NSFont.systemFont(ofSize: 13.0, weight: .semibold)
    private let buttonFont = NSFont.systemFont(ofSize: 12.0, weight: .semibold)
    private let retryLabel = "重试"
    private let cancelLabel = "取消"

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

    func show(status: Int32, text: String, actions: Int32 = 0) {
        let nextStatus = DictationStatus(rawStatus: status)
        let nextActions = OverlayActions(rawActions: actions)
        var changed = false

        lock.lock()
        if currentStatus != nextStatus || currentText != text || currentActions != nextActions {
            currentStatus = nextStatus
            currentText = text
            currentActions = nextActions
            changed = true
        }
        lock.unlock()

        performOnMain {
            guard self.createPanelIfNeeded() else {
                return
            }
            self.panel?.ignoresMouseEvents = nextActions == .none
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
        let snapshot = OverlaySnapshot(
            status: currentStatus,
            text: currentText,
            actions: currentActions
        )
        lock.unlock()
        return snapshot
    }

    func perform(action: OverlayAction) {
        statusOverlayActionCallback?(action.rawValue)
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
        newPanel.ignoresMouseEvents = currentActions == .none
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
        let snapshot = snapshot()
        let rawText = snapshot.text.trimmingCharacters(in: .whitespacesAndNewlines)
        let text = (rawText.isEmpty ? " " : rawText) as NSString
        let attributes: [NSAttributedString.Key: Any] = [
            .font: font,
        ]
        let textWidth = ceil(text.size(withAttributes: attributes).width)
        let buttonGroupWidth = actionButtonGroupWidth(snapshot.actions)
        let actionWidth = buttonGroupWidth > 0.0 ? actionGap + buttonGroupWidth : 0.0
        let idealWidth = horizontalPadding * 2.0 + dotSize + contentGap + textWidth + actionWidth
        return min(snapshot.actions == .none ? maxWidth : maxActionWidth, ceil(idealWidth))
    }

    private func actionButtonGroupWidth(_ actions: OverlayActions) -> CGFloat {
        let labels: [String]
        switch actions {
        case .none:
            labels = []
        case .retry:
            labels = [retryLabel]
        case .retryCancel:
            labels = [retryLabel, cancelLabel]
        }
        guard !labels.isEmpty else {
            return 0.0
        }
        let widths = labels.map { buttonWidth(label: $0) }.reduce(0.0, +)
        return widths + buttonGap * CGFloat(labels.count - 1)
    }

    private func buttonWidth(label: String) -> CGFloat {
        let attributes: [NSAttributedString.Key: Any] = [.font: buttonFont]
        let textWidth = ceil((label as NSString).size(withAttributes: attributes).width)
        return max(46.0, textWidth + buttonHorizontalPadding * 2.0)
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

@_cdecl("status_overlay_show_actions")
public func status_overlay_show_actions(
    _ status: Int32,
    _ textPointer: UnsafePointer<CChar>?,
    _ actions: Int32
) {
    let text = textPointer.map { String(cString: $0) } ?? ""
    StatusOverlayController.shared.show(status: status, text: text, actions: actions)
}

@_cdecl("status_overlay_set_action_callback")
public func status_overlay_set_action_callback(_ callback: StatusOverlayActionCallback?) {
    statusOverlayActionCallback = callback
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
