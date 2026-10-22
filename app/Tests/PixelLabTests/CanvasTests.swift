import AppKit
import CoreGraphics
import Foundation
import Testing

@testable import PixelLab

/// docs/intents/2026-10-floating-library/spec.md: the canvas-first window.
@MainActor @Suite struct ZoomTests {
    /// An app with a scene open, the stage 1000×700 and the zoom at a level.
    func app(zoom: Zoom = .level(1), kind: DocRef.Kind = .scene) -> AppModel {
        let app = AppModel()
        app.document = DocumentModel(ref: DocRef(kind, "a"))
        app.mode = kind
        app.stageViewport = CGSize(width: 1000, height: 700)
        app.zoom = zoom
        return app
    }

    @Test("R1: a pinch steps one level and keeps the anchor")
    func testR1PinchStepsOneLevelAndKeepsTheAnchor() {
        let window = CGRect(x: -400, y: -300, width: 800, height: 600)
        let viewport = CGSize(width: 1000, height: 700)
        let old = StageGeometry(window: window, zoom: 1, viewport: viewport, tileOrigin: .zero)
        let new = StageGeometry(window: window, zoom: 2, viewport: viewport, tileOrigin: .zero)
        // The pointer is 300 points from the clip's left and 200 from its top; the clip starts at (150, 80).
        let clip = CGPoint(x: 150, y: 80), inClip = CGPoint(x: 300, y: 200)
        let scene = old.scenePoint(fromView: CGPoint(x: clip.x + inClip.x, y: clip.y + inClip.y))
        let origin = new.clipOrigin(keeping: scene, at: inClip)
        let after = new.viewPoint(fromScene: scene)
        #expect(abs(after.x - origin.x - inClip.x) < 1)
        #expect(abs(after.y - origin.y - inClip.y) < 1)
        // And the app takes exactly one level per step.
        let a = app(zoom: .level(2))
        #expect(a.stepZoom(by: 1))
        #expect(a.effectiveZoom == 3)
        #expect(a.stepZoom(by: -1))
        #expect(a.stepZoom(by: -1))
        #expect(a.effectiveZoom == 1)
    }

    @Test("R1: the anchor is clamped at the content's edge, not past it")
    func testR1AnchorClampsAtTheEdge() {
        let window = CGRect(x: 0, y: 0, width: 400, height: 300)
        let g = StageGeometry(window: window, zoom: 2, viewport: CGSize(width: 1000, height: 700), tileOrigin: .zero)
        let o = g.clipOrigin(keeping: CGPoint(x: 1, y: 1), at: CGPoint(x: 500, y: 500))
        #expect(o.x >= 0 && o.y >= 0)
    }

    @Test("R1: never past the kind's levels")
    func testR1NeverPastTheKindsLevels() {
        let scene = app(zoom: .level(4))
        #expect(!scene.stepZoom(by: 1), "a scene stops at 4×")
        #expect(scene.effectiveZoom == 4)
        scene.zoom = .level(1)
        #expect(!scene.stepZoom(by: -1), "and at 1×")
        let object = app(zoom: .level(16), kind: .object)
        #expect(!object.stepZoom(by: 1), "an object stops at 16×")
        #expect(object.stepZoom(by: -1))
        #expect(object.effectiveZoom == 12, "objects step 4×, 8×, 12×, 16×")
        object.zoom = .level(4)
        #expect(!object.stepZoom(by: -1))
    }

    @Test("R2: ⌘= and ⌘− step from Fit")
    func testR2StepsFromFit() {
        let a = app(zoom: .fit)
        a.stageViewport = CGSize(width: 100, height: 60)
        let fit = a.effectiveZoom
        #expect(fit == 2, "the 32×16 empty scene fits the 100×60 stage at 2×")
        #expect(a.stepZoom(by: 1))
        #expect(a.zoom == .level(3), "a step leaves Fit for the level next to its effective one")
        a.zoom = .fit
        #expect(a.stepZoom(by: -1))
        #expect(a.zoom == .level(1))
    }

    @Test("R3: the readout shows the level and clears")
    func testR3TheReadoutClears() async throws {
        let a = app(zoom: .level(1))
        a.readoutSeconds = 0.05
        a.stepZoom(by: 1)
        #expect(a.zoomReadout?.text == "2×")
        try await Task.sleep(for: .milliseconds(300))
        #expect(a.zoomReadout == nil)
    }

    @Test("R3: a newer readout isn't cleared by the older one's timer")
    func testR3ANewerReadoutStays() async throws {
        let a = app(zoom: .level(1))
        a.readoutSeconds = 0.15
        a.stepZoom(by: 1)
        try await Task.sleep(for: .milliseconds(100))
        a.stepZoom(by: 1)
        try await Task.sleep(for: .milliseconds(100))
        #expect(a.zoomReadout?.text == "3×", "the first step's timer ran out, the second's hasn't")
        try await Task.sleep(for: .milliseconds(200))
        #expect(a.zoomReadout == nil)
    }
}

@MainActor @Suite struct ScrollerTests {
    @Test("R5: the stage keeps the system's scroller style")
    func testR5TheStageKeepsTheSystemsScrollerStyle() {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        let style = scroll.scrollerStyle
        QuietScrollers.configure(scroll)
        #expect(scroll.scrollerStyle == style, "AppKit follows \"Show scroll bars\": the style is never forced")
        #expect(scroll.autohidesScrollers)
        #expect(scroll.verticalScroller?.controlSize == .small)
        #expect(scroll.horizontalScroller?.controlSize == .small)
        QuietScrollers.configure(nil)
    }
}

@MainActor @Suite struct ToolTests {
    func scene() -> AppModel {
        let app = AppModel()
        app.document = DocumentModel(ref: DocRef(.scene, "a"))
        return app
    }

    @Test("R7: picking a Library object starts placing; Esc and V stop it")
    func testR7PickingAnObjectStartsPlacingAndEscAndVStopIt() {
        let app = scene()
        let editor = app.editor
        #expect(!editor.isPlacing)
        editor.choosePlaceObject("pine")
        #expect(editor.isPlacing)
        #expect(editor.tool == .select, "placing is not a tool")
        #expect(editor.handleKey(KeyInput(key: .escape)))
        #expect(!editor.isPlacing)
        #expect(editor.placeObject == nil)
        editor.choosePlaceObject("pine")
        #expect(editor.handleKey(KeyInput(key: .char("v"))))
        #expect(editor.placeObject == nil, "V is Select, which puts the object away")
    }

    @Test("R7: Hand pauses placing; Space is a temporary hand; the A key is gone")
    func testR7HandPausesPlacing() {
        let editor = scene().editor
        editor.choosePlaceObject("pine")
        editor.setTool(.pan)
        #expect(!editor.isPlacing, "the hand drags the view, it doesn't place")
        #expect(editor.placeObject == "pine")
        editor.setTool(.select)
        editor.choosePlaceObject("pine")
        editor.spaceHeld = true
        #expect(!editor.isPlacing)
        editor.spaceHeld = false
        #expect(editor.isPlacing)
        #expect(StageTool.allCases == [.select, .pan])
        #expect(!editor.handleKey(KeyInput(key: .char("a"))), "A no longer chooses a tool")
        #expect(StageTool.pan.title == "Hand")
    }

    @Test("R7: a drop places one without starting to place")
    func testR7ADropDoesNotStartPlacing() {
        let editor = scene().editor
        #expect(editor.placeObject == nil)
        editor.drop("pine", at: .zero, geometry: StageGeometry(window: CGRect(x: 0, y: 0, width: 100, height: 100), zoom: 1,
                                                               viewport: CGSize(width: 100, height: 100), tileOrigin: .zero), option: false)
        #expect(editor.placeObject == nil)
        #expect(!editor.isPlacing)
    }

    @Test("R6: the bar's tools per doc kind")
    func testR6TheBarsToolsPerDocKind() {
        #expect(FloatingBarItems.ids(for: .scene) == ["select", "pan", "library"])
        #expect(FloatingBarItems.ids(for: .object) == ["pencil", "eraser", "fill", "picker", "pan"])
    }

    @Test("R6: the object Hand is a held pan that ends when another tool is picked")
    func testR6TheObjectHandEndsWhenAnotherToolIsPicked() {
        let app = AppModel()
        let objects = app.objectEditor
        objects.setHand(true)
        #expect(objects.handHeld)
        #expect(objects.tool == .pencil, "the tool underneath stays")
        objects.setTool(.eraser)
        #expect(!objects.handHeld)
        #expect(objects.handleKey(KeyInput(key: .char("h"))))
        #expect(objects.handHeld, "H toggles the hand on")
        #expect(objects.handleKey(KeyInput(key: .char("h"))))
        #expect(!objects.handHeld)
    }
}

@MainActor @Suite struct LibraryTests {
    func scene() -> AppModel {
        let app = AppModel()
        app.document = DocumentModel(ref: DocRef(.scene, "a"))
        return app
    }

    @Test("R8: L opens and Esc closes, placing first")
    func testR8LOpensAndEscClosesPlacingFirst() {
        let app = scene()
        let editor = app.editor
        #expect(editor.handleKey(KeyInput(key: .char("l"))))
        #expect(app.libraryOpen)
        editor.choosePlaceObject("pine")
        #expect(editor.handleKey(KeyInput(key: .escape)))
        #expect(!editor.isPlacing, "the first Esc stops placing")
        #expect(app.libraryOpen, "and the popover stays")
        #expect(editor.handleKey(KeyInput(key: .escape)))
        #expect(!app.libraryOpen, "the second closes it")
        _ = editor.handleKey(KeyInput(key: .char("l")))
        _ = editor.handleKey(KeyInput(key: .char("l")))
        #expect(!app.libraryOpen, "L toggles")
    }

    @Test("R8: the popover stays open while placing, and an object has no Library")
    func testR8StaysOpenWhilePlacingAndObjectsHaveNone() {
        let app = scene()
        app.toggleLibrary()
        app.editor.choosePlaceObject("pine")
        #expect(app.libraryOpen)
        let object = AppModel()
        object.document = DocumentModel(ref: DocRef(.object, "house"))
        object.toggleLibrary()
        #expect(!object.libraryOpen)
    }

    @Test("R8: a cell is small and its name is its label")
    func testR8ACellIsSmall() {
        #expect(LibraryCell.width == 44 && LibraryCell.height == 42)
        #expect(LibraryPopover.width == 452 && LibraryPopover.gridHeight == 236 && LibraryPopover.columns == 8)
    }

    @Test("R10: the sidebar has no Library")
    func testR10TheSidebarHasNoLibrary() {
        // The sidebar's lower section is the scene's Layers or the object's Palette: nothing in the models picks a tab.
        let app = scene()
        app.toggleLibrary()
        #expect(app.libraryOpen, "the Library is the popover's state, not a sidebar tab")
        app.closeLibrary()
        #expect(!app.libraryOpen)
    }

    @Test("R11: placing from the Library into a layer opens the popover")
    func testR11PlaceIntoOpensTheLibrary() {
        let app = scene()
        app.layers.placeInto()
        #expect(app.libraryOpen)
        #expect(app.editor.tool == .select)
    }
}

@MainActor @Suite struct BackTests {
    func summary(_ name: String) -> SceneSummary {
        SceneSummary(name: name, file: "scenes/\(name).json", viewBox: [0, 0, 32, 16], origin: [0, 0], items: 0, motion: false)
    }

    func app(on ref: DocRef, scenes: [String]) -> AppModel {
        let app = AppModel()
        app.scenes = scenes.map(summary)
        app.document = DocumentModel(ref: ref)
        return app
    }

    @Test("R11: Edit Object from a scene, then Back, returns to it")
    func testR11EditObjectThenBackReturnsToTheScene() async {
        let app = app(on: DocRef(.scene, "harbor"), scenes: ["hero-island", "harbor"])
        await app.editObject("house")
        #expect(app.returnScene == DocRef(.scene, "harbor"), "the scene it came from is remembered")
        #expect(app.backTarget == "harbor")
        app.document = DocumentModel(ref: DocRef(.object, "house"))
        await app.editObject("pine")
        #expect(app.returnScene == DocRef(.scene, "harbor"), "from an object to another, Back still goes to the scene")
    }

    @Test("R11: Back with no scene to return to opens the last scene")
    func testR11BackWithNoSceneToReturnToOpensTheLastScene() {
        let a = app(on: DocRef(.object, "house"), scenes: ["hero-island", "harbor", "meadow"])
        a.lastScene = "meadow"
        #expect(a.returnScene == nil)
        #expect(a.backTarget == "meadow")
        a.lastScene = nil
        #expect(a.backTarget == "hero-island")
        let b = app(on: DocRef(.object, "house"), scenes: ["harbor", "meadow"])
        #expect(b.backTarget == "harbor", "else the first scene")
        let c = app(on: DocRef(.object, "house"), scenes: [])
        #expect(c.backTarget == nil)
        // A scene that is gone isn't the target.
        let d = app(on: DocRef(.object, "house"), scenes: ["harbor"])
        d.returnScene = DocRef(.scene, "deleted")
        #expect(d.backTarget == "harbor")
    }

    @Test("R11: ⌘[ goes back and [ still steps the layer")
    func testR11CommandBracketIsLeftToBackAndBracketStillStepsTheLayer() {
        let app = AppModel()
        let objects = app.objectEditor
        #expect(!objects.handleKey(KeyInput(key: .char("["), command: true)), "⌘[ is the menu's Back, not the object editor's")
        #expect(!objects.handleKey(KeyInput(key: .char("]"), command: true)))
        #expect(objects.handleKey(KeyInput(key: .char("["))), "plain [ still steps the layer")
        #expect(objects.handleKey(KeyInput(key: .char("]"))))
    }

    @Test("R11: the stage's menu has Edit Object for the item under the pointer")
    func testR11TheStageMenuHasEditObject() {
        let a = app(on: DocRef(.scene, "harbor"), scenes: ["harbor"])
        a.document?.setItems(SceneItemsResult(scene: "harbor", items: [SceneItemInfo(path: "0", object: "pine", at: ItemAt(tile: [0, 0, 0]))], groups: [], layers: []))
        func edit() -> LayerMenuItem? { a.layers.menuItems.first { $0.action == .editObject } }
        #expect(edit()?.enabled == false)
        a.layers.select([.item("0")], announce: false)
        #expect(edit()?.enabled == true)
        #expect(edit()?.title == "Edit Object “pine”")
        #expect(edit()?.shortcut == "⌘↩")
    }
}

@MainActor @Suite struct RightPanelTests {
    @Test("R14: Save is disabled when clean and its label says so")
    func testR14SaveIsDisabledWhenCleanAndItsLabelSaysSo() {
        let app = AppModel()
        app.document = DocumentModel(ref: DocRef(.scene, "a"))
        #expect(!app.canSave)
        #expect(RightColumn.saveLabel(unsaved: app.canSave) == "Save, all saved")
        app.document?.status = DocStatus(doc: "scene:a", dirty: true, canUndo: true, canRedo: false)
        #expect(app.canSave)
        #expect(RightColumn.saveLabel(unsaved: app.canSave) == "Save, unsaved changes")
    }

    @Test("R14: the Agent tab shows the waiting count")
    func testR14TheAgentTabShowsTheWaitingCount() {
        #expect(RightColumn.tabLabel("Agent", pending: 0) == "Agent")
        #expect(RightColumn.tabLabel("Agent", pending: 1) == "Agent, 1 proposal to review")
        #expect(RightColumn.tabLabel("Agent", pending: 3) == "Agent, 3 proposals to review")
    }
}
