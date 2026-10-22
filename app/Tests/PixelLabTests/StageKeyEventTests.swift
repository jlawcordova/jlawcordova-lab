import AppKit
import Testing

@testable import PixelLab

/// The stage's keys as real key-down events (what `NSApp.sendEvent` delivers and the script
/// hook's `key` dispatches): each one reads back as the key it was made from (R14–R17).
@MainActor @Suite struct StageKeyEventTests {
    private func roundTrip(_ spec: String) -> KeyInput? {
        guard let k = KeyInput(spec: spec) else { return nil }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 100, height: 100), styleMask: [.titled], backing: .buffered, defer: true)
        guard let event = StageInputView.keyEvent(for: k, window: window), event.type == .keyDown else { return nil }
        return StageInputView.keyInput(event)
    }

    @Test func testR15TabAndShiftTabSurviveAsEvents() {
        #expect(roundTrip("tab") == KeyInput(key: .tab))
        #expect(roundTrip("shift+tab") == KeyInput(key: .tab, shift: true))
    }

    @Test func testR16CommandDownSurvivesAsAnEvent() {
        #expect(roundTrip("cmd+down") == KeyInput(key: .down, command: true))
    }

    @Test func testR14TheExistingKeysSurviveAsEvents() {
        #expect(roundTrip("return") == KeyInput(key: .returnKey))
        #expect(roundTrip("pageup") == KeyInput(key: .pageUp))
        #expect(roundTrip("pagedown") == KeyInput(key: .pageDown))
        #expect(roundTrip("shift+up") == KeyInput(key: .up, shift: true))
        #expect(roundTrip("shift+down") == KeyInput(key: .down, shift: true))
        #expect(roundTrip("opt+left") == KeyInput(key: .left, option: true))
        #expect(roundTrip("right") == KeyInput(key: .right))
        #expect(roundTrip("delete") == KeyInput(key: .delete))
    }
}
