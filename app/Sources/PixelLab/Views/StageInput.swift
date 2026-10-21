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
        if v.tool != tool {
            v.tool = tool
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
    var paintTool: PaintTool = .pencil
    var isScene = true
    private var panStart: (window: CGPoint, origin: CGPoint)?
    private var tracking: NSTrackingArea?

    /// The workspace changed size or place (it grew, or the viewport did): scroll
    /// so the scene pixel at the middle of the view stays there, and what the
    /// owner is looking at stays put.
    private func keepScroll(from old: StageGeometry?) {
        guard isScene, let old, let new = geometry, old.zoom == new.zoom,
              old.viewBox != new.viewBox || old.contentSize != new.contentSize,
              let scroll = enclosingScrollView else { return }
        let clip = scroll.contentView
        let size = clip.bounds.size
        let center = old.scenePoint(fromView: CGPoint(x: clip.bounds.origin.x + size.width / 2, y: clip.bounds.origin.y + size.height / 2))
        DispatchQueue.main.async { [weak self] in
            guard let scroll = self?.enclosingScrollView else { return }
            let clip = scroll.contentView
            let p = new.viewPoint(fromScene: center)
            clip.scroll(to: CGPoint(x: max(p.x - size.width / 2, 0), y: max(p.y - size.height / 2, 0)))
            scroll.reflectScrolledClipView(clip)
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
        if !isScene { return editor.spaceHeld ? .openHand : .crosshair }
        switch editor.effectiveTool {
        case .pan: return .openHand
        case .place: return .crosshair
        case .select: return .arrow
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
            if editor.spaceHeld, let clip = enclosingScrollView?.contentView {
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
        guard isScene, let geometry, let editor, editor.effectiveTool == .select, let app = editor.app else { return }
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

    override func keyUp(with event: NSEvent) {
        if event.keyCode == 49, let editor, editor.spaceHeld {
            editor.spaceHeld = false
            window?.invalidateCursorRects(for: self)
            cursor.set()
            return
        }
        super.keyUp(with: event)
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
