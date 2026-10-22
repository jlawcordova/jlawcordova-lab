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
