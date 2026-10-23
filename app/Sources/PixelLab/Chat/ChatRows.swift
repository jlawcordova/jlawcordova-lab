import Foundation

/// A chat's messages as the conversation shows them
/// (docs/intents/2026-10-agent-chat/spec.md, D9, D12): consecutive steps fold
/// into one row, and rows group by sender, with "Claude" above the first of a
/// group and the avatar beside the last.
struct ChatRow: Identifiable, Equatable {
    enum Kind: Equatable {
        case user(ChatMessage)
        case claude(ChatMessage)
        case steps([ChatMessage])
        case question(ChatMessage)
        case proposal(ChatMessage)
        case change(ChatMessage)
        case note(ChatMessage)
        case typing
    }

    enum Side: Equatable { case me, claude, note }

    var id: String
    var kind: Kind
    var side: Side
    /// The first row of its sender's group (Claude's name shows above it; a gap above it).
    var first: Bool
    /// The last row of its sender's group (Claude's avatar shows beside it).
    var last: Bool

    /// What a folded run of steps reads as: the latest step, and "· 4 steps" for more than one.
    static func stepsLabel(_ steps: [ChatMessage]) -> String {
        let latest = steps.last?.human ?? ""
        return steps.count > 1 ? "\(latest) · \(steps.count) steps" : latest
    }

    static func rows(_ messages: [ChatMessage], typing: Bool) -> [ChatRow] {
        var kinds: [(id: String, kind: Kind)] = []
        for m in messages {
            switch m.kind {
            case .step:
                if case .steps(let run) = kinds.last?.kind {
                    kinds[kinds.count - 1].kind = .steps(run + [m])
                } else {
                    kinds.append((m.id, .steps([m])))
                }
            case .user: kinds.append((m.id, .user(m)))
            case .claude: kinds.append((m.id, .claude(m)))
            case .question: kinds.append((m.id, .question(m)))
            case .proposal: kinds.append((m.id, .proposal(m)))
            case .change: kinds.append((m.id, .change(m)))
            case .note: kinds.append((m.id, .note(m)))
            }
        }
        if typing { kinds.append(("typing", .typing)) }
        let side = { (k: Kind) -> Side in
            switch k {
            case .user: .me
            case .note: .note
            default: .claude
            }
        }
        return kinds.enumerated().map { i, k in
            let me = side(k.kind)
            let before = i > 0 ? side(kinds[i - 1].kind) : nil
            let after = i + 1 < kinds.count ? side(kinds[i + 1].kind) : nil
            return ChatRow(id: k.id, kind: k.kind, side: me, first: before != me, last: after != me)
        }
    }

    /// The date header atop a chat: ("Today", "9:41 AM"), ("Yesterday", …), the
    /// weekday within a week, else the date ("Oct 3", with the year when it isn't this one).
    static func dateHeader(_ date: Date, now: Date = Date(), calendar: Calendar = .current) -> (day: String, time: String) {
        let time = date.formatted(date: .omitted, time: .shortened)
        if calendar.isDate(date, inSameDayAs: now) { return ("Today", time) }
        if let y = calendar.date(byAdding: .day, value: -1, to: now), calendar.isDate(date, inSameDayAs: y) { return ("Yesterday", time) }
        let days = calendar.dateComponents([.day], from: calendar.startOfDay(for: date), to: calendar.startOfDay(for: now)).day ?? 99
        if days < 7, days > 0 { return (date.formatted(.dateTime.weekday(.wide)), time) }
        let sameYear = calendar.component(.year, from: date) == calendar.component(.year, from: now)
        return (sameYear ? date.formatted(.dateTime.month(.abbreviated).day()) : date.formatted(.dateTime.month(.abbreviated).day().year()), time)
    }

    /// The Chats list's "when": the time today, else the day.
    static func when(_ date: Date, now: Date = Date(), calendar: Calendar = .current) -> String {
        let h = dateHeader(date, now: now, calendar: calendar)
        return h.day == "Today" ? h.time : h.day
    }
}

/// Item and layer names in Claude's replies, as links that select them
/// (docs/intents/2026-10-agent-chat/spec.md, D10).
enum ChatLinks {
    struct Segment: Equatable {
        var text: String
        var target: LayerTarget?
    }

    /// `<object> <path>` where the scene has an item at that path using that
    /// object ("pine 134.0"), and a layer's exact name of at least 3
    /// characters standing as a word ("Boats").
    static func segments(_ text: String, objectAt: (String) -> String?, layers: [(id: String, name: String)]) -> [Segment] {
        var marks: [(range: Range<String.Index>, target: LayerTarget)] = []
        let item = /([a-z][a-z0-9-]*) (\d+(?:\.\d+)*)/
        for m in text.matches(of: item) {
            let object = String(m.output.1), path = String(m.output.2)
            guard objectAt(path) == object, isWordBoundary(text, m.range) else { continue }
            marks.append((m.range, .item(path)))
        }
        for layer in layers.sorted(by: { $0.name.count > $1.name.count }) where layer.name.count >= 3 {
            var from = text.startIndex
            while let r = text.range(of: layer.name, range: from..<text.endIndex) {
                if isWordBoundary(text, r), !marks.contains(where: { $0.range.overlaps(r) }) { marks.append((r, .layer(layer.id))) }
                from = r.upperBound
            }
        }
        marks.sort { $0.range.lowerBound < $1.range.lowerBound }
        var out: [Segment] = []
        var at = text.startIndex
        for m in marks {
            if at < m.range.lowerBound { out.append(Segment(text: String(text[at..<m.range.lowerBound]))) }
            out.append(Segment(text: String(text[m.range]), target: m.target))
            at = m.range.upperBound
        }
        if at < text.endIndex { out.append(Segment(text: String(text[at...]))) }
        return out
    }

    private static func isWordBoundary(_ text: String, _ r: Range<String.Index>) -> Bool {
        let before = r.lowerBound > text.startIndex ? text[text.index(before: r.lowerBound)] : " "
        let after = r.upperBound < text.endIndex ? text[r.upperBound] : " "
        let word = { (c: Character) in c.isLetter || c.isNumber || c == "-" || c == "_" }
        // A trailing "." ends a sentence, unless a digit follows (a longer path).
        let afterOK = !word(after) && !(after == "." && r.upperBound < text.index(before: text.endIndex) && text[text.index(after: r.upperBound)].isNumber)
        return !word(before) && afterOK
    }
}
