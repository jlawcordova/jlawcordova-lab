import AppKit
import SwiftUI

/// The stage's pointer and keyboard: an AppKit view over the scrollable
/// content (same coordinates as `StageGeometry`'s view points), so clicks,
/// drags, modifier keys, cursors, key repeat and drops behave like a Mac
/// editor. It forwards everything to the `SceneEditor`.
struct StageInput: NSViewRepresentable {
    var geometry: StageGeometry
    var editor: SceneEditor
    var objects: ObjectEditor
    var tool: StageTool
    var placing: Bool
    /// Object mode's tool (the cursor follows it).
    var paintTool: PaintTool
    var isScene: Bool

    func makeNSView(context: Context) -> StageInputView {
        let v = StageInputView()
        v.editor = editor
        v.objects = objects
        editor.stageView = v
        return v
    }

    func updateNSView(_ v: StageInputView, context: Context) {
        v.geometry = geometry
        v.editor = editor
        v.objects = objects
        if v.isScene != isScene || v.paintTool != paintTool {
            v.isScene = isScene
            v.paintTool = paintTool
            v.window?.invalidateCursorRects(for: v)
        }
        if editor.stageView !== v { editor.stageView = v }
        if v.tool != tool || v.placing != placing || v.handHeld != objects.handHeld {
            v.handHeld = objects.handHeld
            v.tool = tool
            v.placing = placing
            v.window?.invalidateCursorRects(for: v)
        }
    }
}

final class StageInputView: NSView {
    var geometry: StageGeometry? {
        didSet { keepScroll(from: oldValue) }
    }
    weak var editor: SceneEditor?
    weak var objects: ObjectEditor?
    var tool: StageTool = .select
    var placing = false
    var handHeld = false
    var paintTool: PaintTool = .pencil
    var isScene = true
    private var panStart: (window: CGPoint, origin: CGPoint)?
    private var tracking: NSTrackingArea?

    /// A zoom step's anchor: a scene point, and where it sits in the visible area (relative to the clip's origin).
    private var zoomAnchor: (scene: CGPoint, inClip: CGPoint)?
    /// The last anchor a zoom kept, for the script hook to check it stayed put.
    private var keptAnchor: (scene: CGPoint, inClip: CGPoint)?

    /// The next zoom keeps the scene point under a view point (the pointer), or the middle of the
    /// visible area, where it is on screen (docs/intents/2026-10-floating-library/spec.md, D2).
    func anchorNextZoom(atView p: CGPoint?) {
        guard let g = geometry, let clip = enclosingScrollView?.contentView else { return }
        let b = clip.bounds
        let view = p ?? CGPoint(x: b.midX, y: b.midY)
        zoomAnchor = (g.scenePoint(fromView: view), CGPoint(x: view.x - b.minX, y: view.y - b.minY))
    }

    /// How far, in points, the last anchored zoom left its scene point from where it was on screen.
    var anchorDrift: CGFloat? {
        guard let g = geometry, let kept = keptAnchor, let clip = enclosingScrollView?.contentView else { return nil }
        let p = g.viewPoint(fromScene: kept.scene)
        return hypot(p.x - clip.bounds.minX - kept.inClip.x, p.y - clip.bounds.minY - kept.inClip.y)
    }

    /// A zoom step by `steps` levels anchored at a view point (nil: the middle of the stage).
    func zoomStep(_ steps: Int, atView p: CGPoint?) {
        editor?.app?.stepZoom(by: steps, anchor: p)
    }

    /// The workspace changed size or place (it grew, or the viewport did), or the zoom did: scroll
    /// so the scene point at the middle of the view (or the one a zoom step anchored) stays where it
    /// is on screen, and what the owner is looking at stays put.
    private func keepScroll(from old: StageGeometry?) {
        guard let old, let new = geometry, let scroll = enclosingScrollView else { return }
        let zoomed = old.zoom != new.zoom
        guard zoomed || (isScene && (old.viewBox != new.viewBox || old.contentSize != new.contentSize)) else { return }
        let clip = scroll.contentView
        let size = clip.bounds.size
        let anchor: (scene: CGPoint, inClip: CGPoint)
        if zoomed, let given = zoomAnchor {
            anchor = given
        } else {
            let mid = CGPoint(x: clip.bounds.origin.x + size.width / 2, y: clip.bounds.origin.y + size.height / 2)
            anchor = (old.scenePoint(fromView: mid), CGPoint(x: size.width / 2, y: size.height / 2))
        }
        zoomAnchor = nil
        DispatchQueue.main.async { [weak self] in
            guard let scroll = self?.enclosingScrollView else { return }
            let clip = scroll.contentView
            clip.scroll(to: new.clipOrigin(keeping: anchor.scene, at: anchor.inClip))
            scroll.reflectScrolledClipView(clip)
            if zoomed { self?.keptAnchor = anchor }
            self?.applyReveal()
        }
    }

    /// A scene rect to keep in view, applied again once the workspace has grown to hold it.
    private var pendingReveal: CGRect?

    /// Scrolls so a rect in scene pixels is visible, with a margin, at the zoom the owner has (R18).
    func reveal(sceneRect: CGRect) {
        pendingReveal = sceneRect
        applyReveal()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in
            self?.applyReveal()
            self?.pendingReveal = nil
        }
    }

    private func applyReveal() {
        guard let g = geometry, let r = pendingReveal else { return }
        scrollToVisible(g.viewRect(fromScene: r).insetBy(dx: -48, dy: -48))
    }

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override init(frame: NSRect) {
        super.init(frame: frame)
        registerForDraggedTypes([.string])
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityLabel("Stage")
        setAccessibilityHelp("Scenes: V select, A place, H pan. A click picks a layer, a double-click goes inside it, Shift-click adds to the selection; the Layers list selects from the keyboard. Arrows move the selection by a tile, Option-arrows one item by a pixel, Shift-up and down by a level. Command-G groups, Shift-Command-G ungroups. Tab and Shift-Tab select the next and previous item. Command-Down drops the selection onto what is below it. Return places the library object at the cursor, on top of a block that is there. Objects: B pencil, E eraser, G fill, I picker; arrows move the pixel cursor, Return paints at it, Delete erases it, [ and ] change layer, Page Up and Page Down change frame.")
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    // MARK: Focus

    override func becomeFirstResponder() -> Bool {
        editor?.stageFocused = true
        return true
    }

    override func resignFirstResponder() -> Bool {
        editor?.stageFocused = false
        editor?.spaceHeld = false
        return true
    }

    // MARK: Cursor and hover

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let t = NSTrackingArea(rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect, .cursorUpdate], owner: self)
        addTrackingArea(t)
        tracking = t
    }

    private var cursor: NSCursor {
        guard let editor else { return .arrow }
        if editor.panning { return .closedHand }
        if !isScene { return editor.spaceHeld || objects?.handHeld == true ? .openHand : .crosshair }
        switch editor.effectiveTool {
        case .pan: return .openHand
        case .select: return editor.isPlacing ? .crosshair : .arrow
        }
    }

    override func resetCursorRects() {
        addCursorRect(visibleRect, cursor: cursor)
    }

    override func cursorUpdate(with event: NSEvent) { cursor.set() }

    private func point(_ event: NSEvent) -> CGPoint { convert(event.locationInWindow, from: nil) }

    override func mouseMoved(with event: NSEvent) {
        if !isScene, let geometry, let objects {
            objects.pointerMoved(point(event), geometry: geometry)
            return
        }
        guard isScene, let geometry, let editor else { return }
        editor.pointerMoved(point(event), geometry: geometry, option: event.modifierFlags.contains(.option))
    }

    // MARK: Zoom

    /// A pinch's running total; one level per `pinchStep`, reset when the gesture ends (R1).
    private var pinch: CGFloat = 0
    private var wheel: CGFloat = 0
    static let pinchStep: CGFloat = 0.25
    static let wheelStep: CGFloat = 24

    override func magnify(with event: NSEvent) {
        pinch += event.magnification
        if abs(pinch) >= Self.pinchStep {
            zoomStep(pinch > 0 ? 1 : -1, atView: point(event))
            pinch = 0
        }
        if event.phase == .ended || event.phase == .cancelled { pinch = 0 }
    }

    /// ⌘-scroll zooms: a notch of a mouse wheel is a level, a trackpad's running total a level per
    /// threshold. Plain scrolling goes to the scroll view.
    override func scrollWheel(with event: NSEvent) {
        guard event.modifierFlags.contains(.command) else {
            super.scrollWheel(with: event)
            return
        }
        let dy = event.scrollingDeltaY
        if event.hasPreciseScrollingDeltas {
            wheel += dy
            if abs(wheel) >= Self.wheelStep {
                zoomStep(wheel > 0 ? 1 : -1, atView: point(event))
                wheel = 0
            }
            if event.phase == .ended || event.phase == .cancelled { wheel = 0 }
        } else if dy != 0 {
            zoomStep(dy > 0 ? 1 : -1, atView: point(event))
        }
    }

    override func mouseExited(with event: NSEvent) {
        editor?.pointerExited()
        objects?.pointerExited()
    }

    override func flagsChanged(with event: NSEvent) {
        editor?.placeByPixel = event.modifierFlags.contains(.option)
        super.flagsChanged(with: event)
    }

    // MARK: Clicks and drags

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        if !isScene, let geometry, let editor, let objects {
            if editor.spaceHeld || objects.handHeld, let clip = enclosingScrollView?.contentView {
                editor.panning = true
                panStart = (event.locationInWindow, clip.bounds.origin)
                NSCursor.closedHand.set()
                return
            }
            objects.mouseDown(point(event), geometry: geometry, option: event.modifierFlags.contains(.option), shift: event.modifierFlags.contains(.shift))
            return
        }
        guard isScene, let geometry, let editor else { return }
        // ⌃-click is a right-click: the layer menu.
        if event.modifierFlags.contains(.control) {
            rightMouseDown(with: event)
            return
        }
        editor.mouseDown(point(event), geometry: geometry, option: event.modifierFlags.contains(.option), shift: event.modifierFlags.contains(.shift),
                         clickCount: event.clickCount)
        if editor.panning, let clip = enclosingScrollView?.contentView {
            panStart = (event.locationInWindow, clip.bounds.origin)
            NSCursor.closedHand.set()
        }
    }

    override func mouseDragged(with event: NSEvent) {
        guard let editor else { return }
        if let panStart, let scroll = enclosingScrollView {
            let clip = scroll.contentView
            let dx = event.locationInWindow.x - panStart.window.x
            let dy = event.locationInWindow.y - panStart.window.y
            // The window's y runs up; a flipped clip view's runs down.
            let y = clip.isFlipped ? panStart.origin.y + dy : panStart.origin.y - dy
            clip.scroll(to: CGPoint(x: panStart.origin.x - dx, y: y))
            scroll.reflectScrolledClipView(clip)
            return
        }
        guard let geometry else { return }
        if !isScene {
            objects?.mouseDragged(point(event), geometry: geometry)
            return
        }
        editor.mouseDragged(point(event), geometry: geometry, option: event.modifierFlags.contains(.option))
    }

    /// The layer menu (docs/intents/2026-10-scene-layers/spec.md, D9), not
    /// while placing (as in the prototype).
    override func rightMouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        guard isScene, let geometry, let editor, editor.effectiveTool == .select, !editor.isPlacing, let app = editor.app else { return }
        app.layers.stageRightClick(point(event), geometry: geometry, windowPoint: event.locationInWindow)
    }

    override func menu(for event: NSEvent) -> NSMenu? { nil }

    override func mouseUp(with event: NSEvent) {
        panStart = nil
        if !isScene { objects?.mouseUp() }
        editor?.mouseUp()
        window?.invalidateCursorRects(for: self)
    }

    // MARK: Keys

    override func keyDown(with event: NSEvent) {
        guard let editor, let input = Self.keyInput(event) else {
            super.keyDown(with: event)
            return
        }
        if input.key == .space, !event.modifierFlags.contains(.command) {
            if !editor.spaceHeld {
                editor.spaceHeld = true
                window?.invalidateCursorRects(for: self)
                NSCursor.openHand.set()
            }
            return
        }
        // Space + arrows pans, like Space + drag.
        if editor.spaceHeld, let scroll = enclosingScrollView {
            let d: CGPoint? = switch input.key {
            case .left: CGPoint(x: -32, y: 0)
            case .right: CGPoint(x: 32, y: 0)
            case .up: CGPoint(x: 0, y: -32)
            case .down: CGPoint(x: 0, y: 32)
            default: nil
            }
            if let d {
                let clip = scroll.contentView
                clip.scroll(to: CGPoint(x: clip.bounds.origin.x + d.x, y: clip.bounds.origin.y + (clip.isFlipped ? d.y : -d.y)))
                scroll.reflectScrolledClipView(clip)
                return
            }
        }
        if !isScene {
            if objects?.handleKey(input) != true { super.keyDown(with: event) }
            return
        }
        if !editor.handleKey(input) { super.keyDown(with: event) }
    }

    /// Keys the window would otherwise take before `keyDown` (the key-view loop takes Tab, the main
    /// menu takes ⌘↓): while the stage is the first responder in a scene, they are the stage's
    /// (docs/intents/2026-10-scene-canvas/spec.md, R15, R16).
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard event.type == .keyDown, isScene, let editor, window?.firstResponder === self, let input = Self.keyInput(event) else {
            return super.performKeyEquivalent(with: event)
        }
        let plain = !input.command && !input.control && !input.option
        let tab = input.key == .tab && plain
        let drop = input.key == .down && input.command && !input.control && !input.option && !input.shift
        guard tab || drop, editor.handleKey(input) else { return super.performKeyEquivalent(with: event) }
        return true
    }

    override func keyUp(with event: NSEvent) {
        if event.keyCode == 49, let editor, editor.spaceHeld {
            editor.spaceHeld = false
            window?.invalidateCursorRects(for: self)
            cursor.set()
            return
        }
        super.keyUp(with: event)
    }

    /// A key-down event for a key, as the keyboard sends it: what the script hook dispatches through the window.
    static func keyEvent(for k: KeyInput, window: NSWindow) -> NSEvent? {
        let (code, chars): (UInt16, String) = switch k.key {
        case .left: (123, String(UnicodeScalar(NSLeftArrowFunctionKey)!))
        case .right: (124, String(UnicodeScalar(NSRightArrowFunctionKey)!))
        case .down: (125, String(UnicodeScalar(NSDownArrowFunctionKey)!))
        case .up: (126, String(UnicodeScalar(NSUpArrowFunctionKey)!))
        case .pageUp: (116, String(UnicodeScalar(NSPageUpFunctionKey)!))
        case .pageDown: (121, String(UnicodeScalar(NSPageDownFunctionKey)!))
        case .tab: (48, k.shift ? "\u{19}" : "\t")
        case .delete: (51, "\u{7f}")
        case .returnKey: (36, "\r")
        case .escape: (53, "\u{1b}")
        case .space: (49, " ")
        case .char(let c): (0, String(c))
        }
        var flags: NSEvent.ModifierFlags = []
        if k.shift { flags.insert(.shift) }
        if k.option { flags.insert(.option) }
        if k.command { flags.insert(.command) }
        if k.control { flags.insert(.control) }
        switch k.key {
        case .left, .right, .up, .down: flags.formUnion([.numericPad, .function])
        case .pageUp, .pageDown: flags.insert(.function)
        default: break
        }
        return NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                windowNumber: window.windowNumber, context: nil, characters: chars,
                                charactersIgnoringModifiers: k.key == .tab ? "\t" : chars, isARepeat: false, keyCode: code)
    }

    static func keyInput(_ event: NSEvent) -> KeyInput? {
        let f = event.modifierFlags
        let key: KeyInput.Key
        switch event.keyCode {
        case 123: key = .left
        case 124: key = .right
        case 125: key = .down
        case 126: key = .up
        case 48: key = .tab
        case 116: key = .pageUp
        case 121: key = .pageDown
        case 51, 117: key = .delete
        case 36, 76: key = .returnKey
        case 53: key = .escape
        case 49: key = .space
        default:
            guard let c = event.charactersIgnoringModifiers?.lowercased().first else { return nil }
            key = .char(c)
        }
        return KeyInput(key: key, option: f.contains(.option), shift: f.contains(.shift), command: f.contains(.command), control: f.contains(.control))
    }

    // MARK: Drops from the library

    private func draggedObject(_ info: NSDraggingInfo) -> String? {
        guard let s = info.draggingPasteboard.string(forType: .string) else { return editor?.dropObject }
        return s.hasPrefix(LibraryDrag.prefix) ? String(s.dropFirst(LibraryDrag.prefix.count)) : nil
    }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        draggingUpdated(sender)
    }

    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        guard isScene, let geometry, let editor, let object = draggedObject(sender) ?? editor.placeObject else { return [] }
        editor.dropObject = object
        editor.pointerMoved(convert(sender.draggingLocation, from: nil), geometry: geometry, option: NSEvent.modifierFlags.contains(.option))
        return .copy
    }

    override func draggingExited(_ sender: NSDraggingInfo?) {
        editor?.dropObject = nil
        editor?.pointerExited()
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        guard isScene, let geometry, let editor, let object = draggedObject(sender) ?? editor.placeObject else { return false }
        window?.makeFirstResponder(self)
        editor.drop(object, at: convert(sender.draggingLocation, from: nil), geometry: geometry, option: NSEvent.modifierFlags.contains(.option))
        return true
    }
}

/// Library thumbnails drag as the string "pixellab-object:<name>".
enum LibraryDrag {
    static let prefix = "pixellab-object:"
}
