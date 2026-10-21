import AppKit
import SwiftUI

/// The chat's input (docs/intents/2026-10-agent-chat/spec.md, D12, R18): a
/// rounded field whose placeholder holds the suggestion, a "tab" key cap
/// while it can be taken, and send or Stop.
struct ChatInputBar: View {
    var chat: ChatModel

    var body: some View {
        let running = chat.running?.chatID == chat.currentID && chat.isRunning
        let suggestion = chat.draft.isEmpty && !chat.isRunning ? chat.suggestion : nil
        HStack(alignment: .bottom, spacing: 6) {
            ZStack(alignment: .topLeading) {
                if chat.draft.isEmpty {
                    Text(chat.placeholder)
                        .font(.system(size: 13))
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(1)
                        .padding(.top, 6)
                        .allowsHitTesting(false)
                        .accessibilityHidden(true)
                }
                ChatInputView(chat: chat, suggestion: suggestion)
                    .frame(height: chat.inputHeight)
            }
            .padding(.leading, 12)
            if suggestion != nil {
                Text("tab")
                    .font(.system(size: 10.5)).foregroundStyle(Theme.inkMuted)
                    .padding(.horizontal, 6).padding(.vertical, 1)
                    .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(Theme.border, lineWidth: 1))
                    .padding(.bottom, 8)
                    .accessibilityHidden(true)
            }
            if running {
                Button { chat.stop() } label: {
                    Image(systemName: "stop.fill").font(.system(size: 10)).foregroundStyle(Theme.card)
                        .frame(width: 30, height: 30).background(Theme.ink, in: Circle())
                }
                .buttonStyle(.plain)
                .help("Stop (Esc)")
                .accessibilityLabel("Stop")
            } else {
                let empty = chat.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                Button { Task { await chat.send() } } label: {
                    Image(systemName: "arrow.up").font(.system(size: 13, weight: .bold))
                        .foregroundStyle(empty ? Theme.inkMuted : Theme.onAccent)
                        .frame(width: 30, height: 30)
                        .background(empty ? Theme.border : Theme.accent, in: Circle())
                }
                .buttonStyle(.plain)
                .disabled(empty || chat.cantSend != nil)
                .help(chat.cantSend ?? "Send (Return)")
                .accessibilityLabel("Send")
            }
        }
        .padding(4)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
    }

}

/// The text view: Return sends, ⇧Return and ⌥Return add a line, Tab takes the
/// suggestion while the input is empty (else moves the focus), Esc stops a turn.
struct ChatInputView: NSViewRepresentable {
    var chat: ChatModel
    var suggestion: String?

    func makeCoordinator() -> Coordinator { Coordinator(chat: chat) }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        let text = ChatTextView()
        text.delegate = context.coordinator
        text.coordinator = context.coordinator
        text.isRichText = false
        text.allowsUndo = true
        text.drawsBackground = false
        text.font = .systemFont(ofSize: 13)
        text.textColor = NSColor(Theme.ink)
        text.insertionPointColor = NSColor(Theme.accent)
        text.textContainerInset = NSSize(width: 0, height: 6)
        text.textContainer?.lineFragmentPadding = 0
        text.isVerticallyResizable = true
        text.isHorizontallyResizable = false
        text.autoresizingMask = [.width]
        text.textContainer?.widthTracksTextView = true
        text.isAutomaticQuoteSubstitutionEnabled = false
        text.isAutomaticDashSubstitutionEnabled = false
        text.setAccessibilityLabel("Message Claude")
        scroll.documentView = text
        context.coordinator.textView = text
        chat.inputView = text
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let text = scroll.documentView as? ChatTextView else { return }
        context.coordinator.chat = chat
        context.coordinator.suggestion = suggestion
        if text.string != chat.draft {
            text.string = chat.draft
            context.coordinator.measure()
        }
        text.setAccessibilityHelp(suggestion.map { "Press Tab to use the suggestion: \($0). Return sends, Shift-Return adds a line." } ?? "Return sends, Shift-Return adds a line.")
        if context.coordinator.focusSeen != chat.focusRequest {
            context.coordinator.focusSeen = chat.focusRequest
            DispatchQueue.main.async { text.window?.makeFirstResponder(text) }
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        var chat: ChatModel
        var suggestion: String?
        weak var textView: ChatTextView?
        var focusSeen = 0

        init(chat: ChatModel) {
            self.chat = chat
            focusSeen = chat.focusRequest
        }

        func textDidChange(_ notification: Notification) {
            guard let t = textView else { return }
            if chat.draft != t.string { chat.draft = t.string }
            measure()
        }

        /// One to five lines tall.
        func measure() {
            guard let t = textView, let lm = t.layoutManager, let tc = t.textContainer else { return }
            lm.ensureLayout(for: tc)
            let line = (t.font?.boundingRectForFont.height ?? 16)
            let h = min(max(lm.usedRect(for: tc).height, line), line * 5 + 2) + 12
            if abs(chat.inputHeight - h) > 0.5 { chat.inputHeight = max(30, h) }
        }

        /// Returns whether it used the key.
        func key(_ event: NSEvent) -> Bool {
            let mods = event.modifierFlags.intersection([.shift, .option, .command, .control])
            switch event.keyCode {
            case 36, 76: // Return
                if mods.contains(.shift) || mods.contains(.option) { return false }
                if mods.isEmpty {
                    Task { await chat.send() }
                    return true
                }
                return false
            case 48: // Tab
                if mods.isEmpty, chat.draft.isEmpty, let s = suggestion {
                    chat.draft = s
                    return true
                }
                if mods.isEmpty { textView?.window?.selectNextKeyView(nil); return true }
                if mods == .shift { textView?.window?.selectPreviousKeyView(nil); return true }
                return false
            case 53: // Esc
                if chat.isRunning { chat.stop(); return true }
                return false
            default:
                return false
            }
        }
    }
}

final class ChatTextView: NSTextView {
    weak var coordinator: ChatInputView.Coordinator?

    override func keyDown(with event: NSEvent) {
        if let c = coordinator, MainActor.assumeIsolated({ c.key(event) }) { return }
        super.keyDown(with: event)
    }

    override func insertNewline(_ sender: Any?) {
        // ⇧Return and ⌥Return reach here: a line break.
        insertText("\n", replacementRange: selectedRange())
    }
}
