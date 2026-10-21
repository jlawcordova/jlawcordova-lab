import Foundation

/// A chip above the input: one per pinned or selected layer or item
/// (docs/intents/2026-10-agent-chat/spec.md, D5, R9).
struct ChatChip: Equatable, Identifiable, Sendable {
    var target: LayerTarget
    /// A layer's name, or an item's object.
    var label: String
    /// "12 items" for a layer, the path for an item; "· not saved" when it's left out.
    var sub: String
    var pinned: Bool
    var unsaved: Bool
    var id: String { target.description }
}

/// The chips, the CONTEXT line and the placeholder's suggestion (pure, unit-tested).
enum ChatContext {
    /// The chat's pins for the open scene first, then the selection's targets not already pinned.
    /// Targets the scene no longer has are left out.
    static func chips(selection: [LayerTarget], pins: [LayerTarget], tree: LayerTree, items: [String: SceneItemInfo], unsaved: Set<String>) -> [ChatChip] {
        var out: [ChatChip] = []
        var seen = Set<LayerTarget>()
        for (t, pinned) in pins.map({ ($0, true) }) + selection.map({ ($0, false) }) where !seen.contains(t) {
            seen.insert(t)
            let left = unsaved.contains(t.description)
            switch t {
            case .layer(let id):
                guard tree.byID[id] != nil else { continue }
                let n = tree.paths(of: t).count
                out.append(ChatChip(target: t, label: tree.name(of: id), sub: "\(n) item\(n == 1 ? "" : "s")" + (left ? " · not saved" : ""), pinned: pinned, unsaved: left))
            case .item(let p):
                guard let item = items[p] else { continue }
                out.append(ChatChip(target: t, label: item.object, sub: p + (left ? " · not saved" : ""), pinned: pinned, unsaved: left))
            }
        }
        return out
    }

    /// The CONTEXT line's right side: "The whole scene", "1 group, 2 items · 1 pinned to this chat".
    static func countLine(_ chips: [ChatChip]) -> String {
        guard !chips.isEmpty else { return "The whole scene" }
        let groups = chips.filter { $0.target.layerID != nil }.count, items = chips.count - groups
        var parts: [String] = []
        if groups > 0 { parts.append("\(groups) group\(groups == 1 ? "" : "s")") }
        if items > 0 { parts.append("\(items) item\(items == 1 ? "" : "s")") }
        let pinned = chips.filter(\.pinned).count
        return parts.joined(separator: ", ") + (pinned > 0 ? " · \(pinned) pinned to this chat" : "")
    }

    /// The label a sent message keeps for a chip: "pine 14.0", "Boats · 12 items", with "(not sent)".
    static func messageLabel(_ c: ChatChip) -> String {
        let base = c.target.layerID != nil ? "\(c.label) · \(c.sub.replacingOccurrences(of: " · not saved", with: ""))" : "\(c.label) \(c.target.description)"
        return base + (c.unsaved ? " (not sent)" : "")
    }

    /// Whether an object reads as a tree for the suggestions.
    static func isTree(_ object: String) -> Bool {
        object.range(of: #"(^|-)(pine|tree|palm|bush|fir|oak|birch|willow)(-|$)"#, options: .regularExpression) != nil
    }

    /// The placeholder's suggestion (R18): it fits what's selected, and skips what was already asked in this chat.
    /// `objects` are the selected items' objects (a layer's items count); `blocks` says which are blocks.
    static func suggestion(objects: [String], isBlock: (String) -> Bool, asked: [String]) -> String? {
        guard !objects.isEmpty else { return nil }
        let one = objects.count == 1
        let ideas: [String]
        if objects.allSatisfy(isBlock) {
            ideas = one ? ["Raise it a level", "Swap it for another block", "Move it one tile down-right"]
                : ["Raise them a level", "Swap them for another block", "Move these one tile down-right"]
        } else if objects.allSatisfy(isTree) {
            ideas = one ? ["Make this taller", "Swap it for another tree", "Move it one tile up-left"]
                : ["Make these taller", "Swap them for another tree", "Move these one tile up-left"]
        } else {
            ideas = one ? ["What is this?", "Move it one tile down-right", "Make this taller"]
                : ["What are these?", "Move these one tile down-right", "Make these taller"]
        }
        let done = Set(asked.map { $0.lowercased() })
        return ideas.first { !done.contains($0.lowercased()) } ?? ideas[0]
    }
}
