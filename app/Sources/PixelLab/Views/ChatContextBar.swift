import SwiftUI

/// What goes with the next message (docs/intents/2026-10-agent-chat/spec.md, D5, R9–R11): the
/// chips (pinned first, each with pin and remove), the unsaved warning with Save now, and
/// "What Claude receives", the block exactly as it will be sent.
struct ChatContextBar: View {
    @Environment(AppModel.self) private var model
    var chat: ChatModel

    var body: some View {
        let chips = chat.chips
        let isObject = model.document?.ref.kind == .object
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                SectionLabel(title: "Context")
                Spacer()
                Text(chat.contextSummary).font(.system(size: 11)).foregroundStyle(Theme.inkMuted).lineLimit(1)
            }
            ScrollView {
                FlowLayout(spacing: 6) {
                    if chips.isEmpty {
                        wholeChip(isObject ? "whole object" : "whole scene")
                    }
                    ForEach(chips) { chip($0) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(maxHeight: 96)
            .fixedSize(horizontal: false, vertical: true)
            .scrollBounceBehavior(.basedOnSize)
            if !chat.unsavedNames.isEmpty { unsavedWarning }
            DisclosureGroup {
                ScrollView {
                    Text(chat.context?.text ?? "…")
                        .font(Theme.monoFont(11)).foregroundStyle(Theme.ink)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 10).padding(.vertical, 8)
                }
                .frame(maxHeight: 140)
                .fixedSize(horizontal: false, vertical: true)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .padding(.top, 6)
            } label: {
                Text("What Claude receives").font(.system(size: 12)).foregroundStyle(Theme.inkMuted)
            }
            .accessibilityHint("Shows the selection block sent with your message")
        }
        .task(id: chat.wantedContextKey) { await chat.refreshContext() }
    }

    private func wholeChip(_ what: String) -> some View {
        HStack(spacing: 6) {
            Text(model.document?.ref.name ?? "").font(Theme.monoFont(12)).foregroundStyle(Theme.ink)
            Text(what).font(.system(size: 12)).foregroundStyle(Theme.inkMuted)
        }
        .padding(.horizontal, 10)
        .frame(minHeight: 28)
        .overlay(Capsule().strokeBorder(Theme.border, style: StrokeStyle(lineWidth: 1, dash: [3, 2])))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(model.document?.ref.name ?? "the document"), \(what)")
    }

    private func chip(_ c: ChatChip) -> some View {
        HStack(spacing: 6) {
            Text(c.label).font(Theme.nameFont(12)).foregroundStyle(Theme.ink).lineLimit(1)
            Text(c.sub).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted).lineLimit(1)
            Button { chat.togglePin(c.target) } label: {
                Image(systemName: c.pinned ? "pin.fill" : "pin").font(.system(size: 11))
                    .foregroundStyle(c.pinned ? Theme.accent : Theme.inkMuted)
                    .frame(width: 24, height: 24)
                    .background(c.pinned ? Theme.selection : Color.clear, in: Circle())
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .help(c.pinned ? "Unpin from this chat" : "Pin to this chat: it stays in the context while you select something else")
            .accessibilityLabel(c.pinned ? "Unpin \(c.label) \(c.sub) from this chat" : "Pin \(c.label) \(c.sub) to this chat")
            Button { chat.removeChip(c.target) } label: {
                Image(systemName: "xmark").font(.system(size: 9, weight: .semibold)).foregroundStyle(Theme.inkMuted)
                    .frame(width: 24, height: 24).contentShape(Circle())
            }
            .buttonStyle(.plain)
            .help("Remove from the context")
            .accessibilityLabel("Remove \(c.label) \(c.sub) from the context")
        }
        .padding(.leading, 10).padding(.trailing, 2)
        .frame(minHeight: 28)
        .background(c.unsaved ? Color.clear : Theme.card, in: Capsule())
        .overlay(Capsule().strokeBorder(c.unsaved ? Theme.gold : Theme.border, style: StrokeStyle(lineWidth: 1, dash: c.unsaved ? [3, 2] : [])))
        .opacity(c.unsaved ? 0.8 : 1)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(c.label), \(c.sub)\(c.pinned ? ", pinned" : "")")
    }

    private var unsavedWarning: some View {
        let names = chat.unsavedNames
        let list = names.count <= 2 ? names.joined(separator: " and ") : names.prefix(2).joined(separator: ", ") + " and \(names.count - 2) more"
        return HStack(alignment: .center, spacing: 8) {
            Rectangle().fill(Theme.gold).frame(width: 9, height: 9).accessibilityHidden(true)
            Text("\(list) \(names.count == 1 ? "isn’t" : "aren’t") saved yet, so Claude can’t see \(names.count == 1 ? "it" : "them").")
                .font(.system(size: 12)).foregroundStyle(Theme.ink).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 4)
            Button("Save now") { Task { await chat.saveNow() } }
                .buttonStyle(CapsuleButtonStyle())
                .disabled(!model.canSave)
        }
        .padding(.horizontal, 10).padding(.vertical, 8)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .accessibilityElement(children: .contain)
    }
}
