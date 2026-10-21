import SwiftUI

/// The pieces of the conversation (docs/intents/2026-10-agent-chat/spec.md,
/// D12, D14): bubbles with one square corner, the asterisk avatar, folded
/// steps, questions, proposal cards, notes and the typing bubble.

/// Claude's avatar: a plain asterisk on terracotta (not Anthropic's logo or mascot; spec D14).
struct ClaudeAvatar: View {
    var size: CGFloat = 28

    var body: some View {
        ZStack {
            Circle().fill(Theme.claude)
            Asterisk()
                .stroke(Theme.onClaude, style: StrokeStyle(lineWidth: max(1.4, size * 0.068), lineCap: .round))
                .frame(width: size * 0.54, height: size * 0.54)
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// Four strokes through the middle: an eight-armed asterisk.
struct Asterisk: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        let c = CGPoint(x: r.midX, y: r.midY), h = min(r.width, r.height) / 2
        let d = h * 0.707
        for (dx, dy) in [(0.0, h), (h, 0.0), (d, d), (d, -d)] {
            p.move(to: CGPoint(x: c.x - dx, y: c.y - dy))
            p.addLine(to: CGPoint(x: c.x + dx, y: c.y + dy))
        }
        return p
    }
}

/// A bubble's shape: radius 18, no tail, the one square corner at the bottom
/// on the sender's side (bottom-right for mine, bottom-left for Claude's).
struct BubbleShape: Shape {
    var mine: Bool
    func path(in r: CGRect) -> Path {
        UnevenRoundedRectangle(topLeadingRadius: 18, bottomLeadingRadius: mine ? 18 : 0, bottomTrailingRadius: mine ? 0 : 18, topTrailingRadius: 18, style: .continuous)
            .path(in: r)
    }
}

/// My message: its chips above, the accent bubble.
struct UserBubble: View {
    var message: ChatMessage

    var body: some View {
        VStack(alignment: .trailing, spacing: 3) {
            if let chips = message.chips, !chips.isEmpty {
                FlowLayout(spacing: 4, alignment: .trailing) {
                    ForEach(Array(chips.prefix(7).enumerated()), id: \.offset) { _, c in
                        Text(c).font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted)
                            .padding(.horizontal, 6).padding(.vertical, 1)
                            .overlay(Capsule().strokeBorder(Theme.border, lineWidth: 1))
                    }
                    if chips.count > 7 {
                        Text("+\(chips.count - 7) more").font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted)
                    }
                }
            }
            Text(message.text ?? "")
                .font(.system(size: 13))
                .foregroundStyle(Theme.onAccent)
                .textSelection(.enabled)
                .padding(.horizontal, 13).padding(.vertical, 8)
                .background(Theme.accent, in: BubbleShape(mine: true))
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("You: \(message.text ?? "")" + ((message.chips ?? []).isEmpty ? "" : ", about \((message.chips ?? []).joined(separator: ", "))"))
    }
}

/// Claude's words, item and layer names as links (spec D10).
struct ClaudeBubble: View {
    var message: ChatMessage
    var chat: ChatModel

    var body: some View {
        Text(attributed)
            .font(.system(size: 13))
            .lineSpacing(2)
            .foregroundStyle(Theme.ink)
            .tint(Theme.ink)
            .textSelection(.enabled)
            .padding(.horizontal, 13).padding(.vertical, 8)
            .background(Theme.card, in: BubbleShape(mine: false))
            .environment(\.openURL, OpenURLAction { url in
                guard url.scheme == ChatLinkURL.scheme, let t = ChatLinkURL.target(url) else { return .systemAction }
                chat.select(t)
                return .handled
            })
            .accessibilityLabel("Claude: \(message.text ?? "")")
    }

    private var attributed: AttributedString {
        var out = AttributedString()
        for seg in chat.segments(message.text ?? "") {
            var a = AttributedString(seg.text)
            if let t = seg.target {
                a.link = ChatLinkURL.url(t)
                a.font = .system(size: 12.5, weight: .semibold, design: .monospaced)
                a.underlineStyle = Text.LineStyle(pattern: .solid, color: Theme.accent)
            }
            out += a
        }
        return out
    }
}

/// The links' URLs: `pixellab-select:layer:boats`, `pixellab-select:14.0`.
enum ChatLinkURL {
    static let scheme = "pixellab-select"
    static func url(_ t: LayerTarget) -> URL? { URL(string: "\(scheme):\(t.description.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? "")") }
    static func target(_ url: URL) -> LayerTarget? {
        let spec = String(url.absoluteString.dropFirst(scheme.count + 1)).removingPercentEncoding ?? ""
        return LayerTarget(spec: spec)
    }
}

/// A folded run of steps: the latest one's sentence and "· N steps"; it opens to all of them,
/// with the raw call under each while Details is on (spec D9).
struct StepsRow: View {
    var rowID: String
    var steps: [ChatMessage]
    var chat: ChatModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let open = chat.expanded.contains(rowID)
        let failed = steps.contains { $0.ok == false }
        VStack(alignment: .leading, spacing: 3) {
            Button {
                if open { chat.expanded.remove(rowID) } else { chat.expanded.insert(rowID) }
            } label: {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Image(systemName: steps.last?.done == false ? "circle.dotted" : failed ? "exclamationmark.circle" : "checkmark")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(failed ? Theme.gold : Theme.accent)
                        .frame(width: 12)
                    Text(ChatRow.stepsLabel(steps))
                        .font(.system(size: 12))
                        .foregroundStyle(Theme.inkMuted)
                        .multilineTextAlignment(.leading)
                    if steps.count > 1 {
                        Image(systemName: "chevron.down")
                            .font(.system(size: 9, weight: .semibold))
                            .foregroundStyle(Theme.inkMuted)
                            .rotationEffect(.degrees(open ? 180 : 0))
                            .animation(reduceMotion ? nil : .easeOut(duration: 0.15), value: open)
                    }
                }
                .padding(.horizontal, 8).padding(.vertical, 4)
                .frame(minHeight: 24)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(steps.count < 2 && !chat.details)
            .accessibilityLabel(steps.count > 1 ? "\(steps.count) steps, latest: \(steps.last?.human ?? "")" : (steps.last?.human ?? ""))
            .accessibilityHint(steps.count > 1 ? (open ? "Hides the steps" : "Shows every step") : "")
            if steps.count == 1, chat.details, let raw = steps[0].raw {
                Text(raw).font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted).textSelection(.enabled)
                    .padding(.leading, 26)
            }
            if open, steps.count > 1 {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(steps) { s in
                        VStack(alignment: .leading, spacing: 1) {
                            Text(s.human ?? "").font(.system(size: 12)).foregroundStyle(s.ok == false ? Theme.ink : Theme.inkMuted)
                            if chat.details, let raw = s.raw {
                                Text(raw).font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted).textSelection(.enabled)
                            }
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
                .padding(.leading, 26).padding(.trailing, 8)
            }
        }
    }
}

/// A question with an answer button each (spec D8): the first answer in the accent.
struct QuestionCard: View {
    var message: ChatMessage
    var chat: ChatModel

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(message.text ?? "").font(.system(size: 13)).foregroundStyle(Theme.ink).fixedSize(horizontal: false, vertical: true)
            if let chosen = message.chosen {
                if !chosen.isEmpty {
                    Label(chosen, systemImage: "arrowshape.turn.up.left").font(.system(size: 12)).foregroundStyle(Theme.inkMuted)
                }
            } else {
                FlowLayout(spacing: 6) {
                    ForEach(Array((message.options ?? []).enumerated()), id: \.offset) { i, o in
                        Button(o) { Task { await chat.answer(message.id, o) } }
                            .buttonStyle(CapsuleButtonStyle(prominent: i == 0))
                            .disabled(chat.cantSend != nil)
                            .accessibilityHint("Sends “\(o)” to Claude")
                    }
                }
            }
        }
        .padding(.horizontal, 13).padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.card, in: BubbleShape(mine: false))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Claude asks: \(message.text ?? "")")
    }
}

/// A 30 pt capsule button: the accent (prominent) or bordered.
struct CapsuleButtonStyle: ButtonStyle {
    var prominent = false
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12.5, weight: prominent ? .semibold : .regular))
            .foregroundStyle(prominent ? Theme.onAccent : Theme.ink)
            .padding(.horizontal, 13)
            .frame(minHeight: 30)
            .background(prominent ? Theme.accent : Color.clear, in: Capsule())
            .overlay(Capsule().strokeBorder(prominent ? Color.clear : Theme.border, lineWidth: 1))
            .opacity(isEnabled ? (configuration.isPressed ? 0.75 : 1) : 0.45)
            .contentShape(Capsule())
    }
}

/// A proposal card (spec D7, R16). PR 3 adds its actions.
struct ProposalCard: View {
    var message: ChatMessage
    var chat: ChatModel

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                SectionLabel(title: "Proposal \(message.number.map { "#\($0)" } ?? "")")
                Spacer()
                Text(chat.statusText(message)).font(.system(size: 11)).foregroundStyle(Theme.ink)
                    .lineLimit(1).fixedSize()
                    .padding(.horizontal, 7).padding(.vertical, 1)
                    .background(Theme.selection, in: Capsule())
            }
            Text(message.text ?? "Proposal").font(Theme.nameFont(13)).foregroundStyle(Theme.ink).fixedSize(horizontal: false, vertical: true)
            let edits = message.edits ?? []
            if !edits.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(edits.prefix(8).enumerated()), id: \.offset) { _, e in
                        Text(e).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted).fixedSize(horizontal: false, vertical: true)
                    }
                    if edits.count > 8 { Text("… and \(edits.count - 8) more").font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted) }
                }
            }
            ProposalCardActions(message: message, chat: chat)
        }
        .padding(.horizontal, 13).padding(.vertical, 12)
        .background(Theme.card, in: BubbleShape(mine: false))
        .overlay(BubbleShape(mine: false).stroke(Theme.border, lineWidth: 1))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Proposal \(message.number.map { "number \($0)" } ?? ""), \(chat.statusText(message)): \(message.text ?? "")")
    }
}

/// A note in the middle: "You approved #12 · wrote scenes/harbor.mjs".
struct NoteRow: View {
    var message: ChatMessage
    var body: some View {
        Text(message.text ?? "")
            .font(.system(size: 11))
            .foregroundStyle(Theme.inkMuted)
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity)
            .padding(.top, 8).padding(.bottom, 2)
    }
}

/// Three dots that pulse in turn while Claude works (still, with Reduce Motion).
struct TypingBubble: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(minimumInterval: 0.05, paused: reduceMotion)) { ctx in
            let t = ctx.date.timeIntervalSinceReferenceDate
            HStack(spacing: 4) {
                ForEach(0..<3, id: \.self) { i in
                    Circle().fill(Theme.inkMuted).frame(width: 7, height: 7)
                        .opacity(reduceMotion ? 0.8 : 0.3 + 0.7 * (0.5 + 0.5 * sin((t / 1.2 - Double(i) * 0.2) * 2 * .pi)))
                }
            }
            .padding(.horizontal, 14).padding(.vertical, 11)
            .background(Theme.card, in: BubbleShape(mine: false))
        }
        .accessibilityElement()
        .accessibilityLabel("Claude is working")
        .accessibilityAddTraits(.updatesFrequently)
    }
}

/// Children laid out in rows that wrap.
struct FlowLayout: Layout {
    var spacing: CGFloat = 6
    var alignment: HorizontalAlignment = .leading

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        let w = rows.map(\.width).max() ?? 0
        let h = rows.map(\.height).reduce(0, +) + CGFloat(max(rows.count - 1, 0)) * spacing
        return CGSize(width: min(w, proposal.width ?? w), height: h)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in arrange(width: bounds.width, subviews: subviews) {
            var x = alignment == .trailing ? bounds.maxX - row.width : bounds.minX
            for i in row.items {
                let s = subviews[i].sizeThatFits(.unspecified)
                subviews[i].place(at: CGPoint(x: x, y: y + (row.height - s.height) / 2), proposal: ProposedViewSize(width: min(s.width, bounds.width), height: s.height))
                x += min(s.width, bounds.width) + spacing
            }
            y += row.height + spacing
        }
    }

    private func arrange(width: CGFloat, subviews: Subviews) -> [(items: [Int], width: CGFloat, height: CGFloat)] {
        var rows: [(items: [Int], width: CGFloat, height: CGFloat)] = []
        var cur: (items: [Int], width: CGFloat, height: CGFloat) = ([], 0, 0)
        for i in subviews.indices {
            let s = subviews[i].sizeThatFits(.unspecified)
            let w = min(s.width, width)
            if !cur.items.isEmpty, cur.width + spacing + w > width {
                rows.append(cur)
                cur = ([], 0, 0)
            }
            cur.width += (cur.items.isEmpty ? 0 : spacing) + w
            cur.height = max(cur.height, s.height)
            cur.items.append(i)
        }
        if !cur.items.isEmpty { rows.append(cur) }
        return rows
    }
}

/// A card's buttons (spec R16): Approve, Reject and Show/Hide on stage while it waits, and the diff.
struct ProposalCardActions: View {
    var message: ChatMessage
    var chat: ChatModel
    @Environment(AppModel.self) private var model

    var body: some View {
        let waiting = message.outcome == nil
        let busy = model.agent.working != nil
        VStack(alignment: .leading, spacing: 6) {
            if waiting {
                FlowLayout(spacing: 6) {
                    Button("Approve") { Task { await chat.approve(message) } }
                        .buttonStyle(CapsuleButtonStyle(prominent: true)).disabled(busy)
                        .accessibilityHint("Writes the proposal to the document")
                    Button("Reject") { Task { await chat.reject(message) } }
                        .buttonStyle(CapsuleButtonStyle()).disabled(busy)
                    Button(chat.ghostOn(message) ? "Hide on stage" : "Show on stage") { Task { await chat.toggleGhost(message) } }
                        .buttonStyle(CapsuleButtonStyle())
                        .accessibilityAddTraits(chat.ghostOn(message) ? .isSelected : [])
                }
            }
            Button("Diff in the Agent window") { Task { await chat.showDiff(message) } }
                .buttonStyle(AccentLinkStyle())
        }
        .padding(.top, 2)
    }
}

/// A text link in the accent (board 03 has one accent; the system link blue isn't in it).
struct AccentLinkStyle: ButtonStyle {
    var size: CGFloat = 12
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: size))
            .foregroundStyle(Theme.accent)
            .underline(configuration.isPressed)
            .opacity(isEnabled ? (configuration.isPressed ? 0.7 : 1) : 0.45)
            .frame(minHeight: 24)
            .contentShape(Rectangle())
    }
}
