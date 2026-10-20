import Foundation
import Testing

@testable import PixelLab

/// The animation pane's pure logic (docs/intents/2026-10-animation-pane/spec.md):
/// keys hit-tested on a row, drags snapped to the timeline's steps, a key's
/// inherited values, what the pane's keys do, and the shapes the server sends.
@Suite struct TimelineTests {
    let track = MotionTrack(item: "cart", keys: [
        MotionKey(t: 0, move: nil, show: nil, pose: nil),
        MotionKey(t: 7200, move: nil, show: nil, pose: nil),
        MotionKey(t: 10400, move: [-32, -16], show: nil, pose: nil),
        MotionKey(t: 10700, move: nil, show: nil, pose: "mine-cart-tipped"),
        MotionKey(t: 11900, move: nil, show: false, pose: "mine-cart"),
        MotionKey(t: 13500, move: [0, 0], show: true, pose: nil),
    ])

    @Test func R2_aKeyIsHitWithinSixPointsAndTheNearestWins() {
        let keys = track.keys.map(\.t)
        let w: CGFloat = 400, d = 28000.0
        // 10400 ms sits at x 148.57; 10700 at 152.86.
        #expect(TimelineModel.keyIndex(atX: 148, width: w, keys: keys, duration: d) == 2)
        #expect(TimelineModel.keyIndex(atX: 152, width: w, keys: keys, duration: d) == 3)
        #expect(TimelineModel.keyIndex(atX: 0, width: w, keys: keys, duration: d) == 0)
        #expect(TimelineModel.keyIndex(atX: 60, width: w, keys: keys, duration: d) == nil)
        #expect(TimelineModel.keyIndex(atX: 110, width: w, keys: keys, duration: d) == nil, "7200 ms is at x 102.86: 7 points away")
        #expect(TimelineModel.keyIndex(atX: 108, width: w, keys: keys, duration: d) == 1)
    }

    @Test func R5_aDragSnapsToTheHundredMillisecondStepsInsideTheLoop() {
        #expect(TimelineModel.snap(8460, duration: 28000) == 8500)
        #expect(TimelineModel.snap(8440, duration: 28000) == 8400)
        #expect(TimelineModel.snap(0, duration: 28000) == 100, "never onto the rest frame")
        #expect(TimelineModel.snap(27999, duration: 28000) == 27900, "never past the loop")
        #expect(TimelineModel.snap(1950, duration: 2000) == 1900)
        #expect(TimelineModel.snap(500, duration: 150) == 100, "a loop shorter than two steps still has one place to go")
        let x = TimelineModel.x(of: 14000, width: 400, duration: 28000)
        #expect(x == 200)
        #expect(TimelineModel.time(atX: 200, width: 400, duration: 28000) == 14000)
        #expect(TimelineModel.time(atX: -5, width: 400, duration: 28000) == 0)
        #expect(TimelineModel.time(atX: 900, width: 400, duration: 28000) == 28000)
    }

    @Test func R7_aKeyInheritsMoveShowAndPoseFromTheKeysBefore() throws {
        let rest = try #require(track.filled(at: 0))
        #expect(rest.move == [0, 0] && rest.show && rest.pose == nil)
        #expect(!rest.ownMove && !rest.ownShow && !rest.ownPose)
        let tipped = try #require(track.filled(at: 10700))
        #expect(tipped.move == [-32, -16], "the move carries on from 10400")
        #expect(!tipped.ownMove && tipped.ownPose && tipped.pose == "mine-cart-tipped")
        let hidden = try #require(track.filled(at: 11900))
        #expect(!hidden.show && hidden.ownShow && hidden.pose == "mine-cart" && hidden.move == [-32, -16])
        let back = try #require(track.filled(at: 13500))
        #expect(back.move == [0, 0] && back.ownMove && back.show && back.ownShow && back.pose == "mine-cart" && !back.ownPose)
        #expect(track.filled(at: 5000) == nil, "no key there")
        let n = track.neighbors(of: 10700)
        #expect(n.previous?.t == 10400 && n.next?.t == 11900)
        #expect(track.neighbors(of: 0).previous == nil)
        #expect(track.neighbors(of: 13500).next == nil)
        #expect(track.key(at: 11900)?.spoken == "hidden, pose mine-cart")
        #expect(track.key(at: 0)?.spoken == "rest frame")
        #expect(track.key(at: 7200)?.spoken == "nothing set")
        #expect(track.key(at: 10400)?.spoken == "move -32, -16")
    }

    @Test @MainActor func R10_thePanesKeysStepThePlayheadAndTheKeysAndDeselect() {
        let m = TimelineModel()
        m.load(SceneShowMotion(name: "x", doc: .init(motion: .init(duration: 28000, tracks: [
            .init(item: "cart", keys: track.keys.map { .init(t: $0.t, move: $0.move, show: $0.show, pose: $0.pose) }),
        ]))))
        #expect(m.hasMotion && m.tracks.count == 1 && m.tracks[0].keys.count == 6)
        #expect(m.handleKey(KeyInput(key: .right)))
        #expect(m.time == 100)
        #expect(m.handleKey(KeyInput(key: .left)))
        #expect(m.handleKey(KeyInput(key: .left)), "stays at 0")
        #expect(m.time == 0)
        // Without an app there's no selected item: ⇧→ walks the first track's keys from the playhead.
        #expect(m.handleKey(KeyInput(key: .right, shift: true)))
        #expect(m.selectedKey == SelectedKey(item: "cart", t: 7200))
        #expect(m.time == 7200, "a selected key takes the playhead there")
        #expect(m.handleKey(KeyInput(key: .right, shift: true)))
        #expect(m.selectedKey?.t == 10400)
        #expect(m.handleKey(KeyInput(key: .left, shift: true)))
        #expect(m.selectedKey?.t == 7200)
        #expect(m.handleKey(KeyInput(key: .escape)))
        #expect(m.selectedKey == nil)
        #expect(!m.handleKey(KeyInput(key: .escape)), "nothing to deselect: the key isn't the pane's")
        #expect(!m.handleKey(KeyInput(key: .char("k"), command: true)), "⌘ keys are the menus'")
        #expect(!m.handleKey(KeyInput(key: .delete)), "no key selected: ⌫ isn't the pane's")
        // A reload that lost the selected key drops it.
        m.selectKey("cart", 13500)
        m.load(SceneShowMotion(name: "x", doc: .init(motion: .init(duration: 28000, tracks: [.init(item: "cart", keys: [.init(t: 0, move: nil, show: nil, pose: nil)])]))))
        #expect(m.selectedKey == nil)
        #expect(m.tracks[0].keys.count == 1)
    }

    @Test func R12_decodesAPinnedKeyAndItsWarnings() throws {
        let json = #"{"doc":"scene:x","changed":true,"summary":"Made cart's key at 8000 ms, pinned at -32, -16","key":{"t":8000,"move":[-32,-16]},"made":true,"pinned":true,"warnings":["scenes/x.mjs: motion.tracks[4] (cart): 8000 → 10400 ms moves (0, 0) in 0 steps"]}"#
        let r = try JSONDecoder().decode(EditResult.self, from: Data(json.utf8))
        #expect(r.pinned == true)
        #expect(r.key == MotionKey(t: 8000, move: [-32, -16], show: nil, pose: nil))
        #expect(r.warnings?.count == 1)
        let moved = #"{"doc":"scene:x","changed":true,"summary":"pine moved at 7.2 s","path":"12","item":{},"keyed":true}"#
        #expect(try JSONDecoder().decode(EditResult.self, from: Data(moved.utf8)).keyed == true)
        let show = #"{"name":"x","doc":{"motion":{"duration":2000,"tracks":[{"item":"cart","keys":[{"t":0},{"t":1000,"move":[8,4],"show":false,"pose":"block"}]}]}}}"#
        let s = try JSONDecoder().decode(SceneShowMotion.self, from: Data(show.utf8))
        #expect(s.doc?.motion?.tracks?[0].keys?[1].pose == "block")
        #expect(s.doc?.motion?.tracks?[0].keys?[1].show == false)
    }
}
