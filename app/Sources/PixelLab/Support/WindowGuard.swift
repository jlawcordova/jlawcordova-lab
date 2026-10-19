import AppKit
import SwiftUI

/// Hands the hosting NSWindow to `found` once the view is in a window.
struct WindowAccessor: NSViewRepresentable {
    var found: @MainActor (NSWindow) -> Void

    func makeNSView(context: Context) -> NSView {
        let v = AccessorView()
        v.found = found
        return v
    }

    func updateNSView(_ nsView: NSView, context: Context) {}

    private final class AccessorView: NSView {
        var found: (@MainActor (NSWindow) -> Void)?
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let window { found?(window) }
        }
    }
}

/// Asks Save / Don't Save / Cancel before the main window closes with unsaved
/// changes. It sits in front of SwiftUI's own window delegate and forwards
/// everything else to it.
@MainActor
final class WindowCloseGuard: NSObject, NSWindowDelegate {
    nonisolated(unsafe) weak var inner: NSWindowDelegate?
    private var allowClose = false

    static func install(on window: NSWindow) {
        if window.delegate is WindowCloseGuard { return }
        let guardian = WindowCloseGuard()
        guardian.inner = window.delegate
        window.delegate = guardian
        objc_setAssociatedObject(window, &associationKey, guardian, .OBJC_ASSOCIATION_RETAIN)
    }

    nonisolated(unsafe) private static var associationKey: UInt8 = 0

    override func responds(to aSelector: Selector!) -> Bool {
        super.responds(to: aSelector) || (inner?.responds(to: aSelector) ?? false)
    }

    override func forwardingTarget(for aSelector: Selector!) -> Any? {
        if let inner, inner.responds(to: aSelector) { return inner }
        return super.forwardingTarget(for: aSelector)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        let model = AppModel.shared
        if allowClose || model.discardConfirmed || model.dirtyDocs.isEmpty {
            return inner?.windowShouldClose?(sender) ?? true
        }
        Task { @MainActor in
            if await model.confirmUnsaved(window: sender) {
                allowClose = true
                sender.performClose(nil)
            }
        }
        return false
    }
}
