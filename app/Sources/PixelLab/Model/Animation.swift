import Foundation

// Ambient loops and actions as the stage and the Inspector read them
// (docs/intents/2026-10-rust-engine/spec.md, R12, R13, R23, D10). The program
// places both at render time (`time`, `play`); the app only needs to know how
// long they last and which actions an element has.

/// One action of an element: its name and length.
struct ActionRow: Hashable, Identifiable, Sendable {
    var name: String
    /// ms.
    var duration: Double
    /// Where it comes from: "item", or "layer 2" of the item's object.
    var source: String
    var id: String { name }

    /// "480 ms", or "1.5 s" from a second up.
    var durationLabel: String {
        duration >= 1000 ? String(format: "%.1f s", duration / 1000) : "\(Int(duration)) ms"
    }
}

/// What a scene item, or a map layer of an object, carries: an ambient loop
/// (its length in ms) and its actions.
struct ElementAnimation: Hashable, Sendable {
    var ambient: Double?
    var actions: [ActionRow] = []

    var isEmpty: Bool { ambient == nil && actions.isEmpty }
}

enum Animation {
    /// The timeline scrubs at most this long (ms).
    static let cap = 60_000.0
    /// The ms between the frames an action's Play shows.
    static let frameStep = 40.0
    /// An action shows at most this many frames, so a long one still plays quickly.
    static let maxFrames = 90

    /// The timeline's length: the longest of the scene's motion and its ambient
    /// loops, capped at 60 s. nil when there is neither.
    static func timelineLength(motion: Double?, ambient: [Double]) -> Double? {
        let longest = ([motion ?? 0] + ambient).max() ?? 0
        return longest > 0 ? min(longest, cap) : nil
    }

    /// The ambient loop and actions written on one source element
    /// (`{ "ambient": { "duration" }, "actions": { "hop": { "duration" } } }`).
    static func parse(_ element: JSONValue?, source: String) -> ElementAnimation {
        guard let element else { return ElementAnimation() }
        let ambient = element["ambient"]?["duration"]?.doubleValue
        var actions: [ActionRow] = []
        if case .object(let all)? = element["actions"] {
            for name in all.keys.sorted() {
                guard let d = all[name]?["duration"]?.doubleValue else { continue }
                actions.append(ActionRow(name: name, duration: d, source: source))
            }
        }
        return ElementAnimation(ambient: ambient, actions: actions)
    }

    /// The map layers of an object's source, by layer index.
    static func layers(of object: JSONValue?) -> [Int: ElementAnimation] {
        var out: [Int: ElementAnimation] = [:]
        for (i, layer) in (object?["layers"]?.arrayValue ?? []).enumerated() {
            let a = parse(layer, source: "layer \(i + 1)")
            if !a.isEmpty { out[i] = a }
        }
        return out
    }

    /// The scene item at a path ("12", or "5.2" inside a group) in the scene's source items.
    static func item(at path: String, in items: [JSONValue]?) -> JSONValue? {
        var list = items
        var found: JSONValue?
        for part in path.split(separator: ".") {
            guard let i = Int(part), let l = list, l.indices.contains(i) else { return nil }
            found = l[i]
            list = found?["items"]?.arrayValue
        }
        return found
    }

    /// Every placed item of the scene's source, with its path, groups walked.
    static func walk(_ items: [JSONValue]?, prefix: String = "", _ visit: (String, JSONValue) -> Void) {
        for (i, item) in (items ?? []).enumerated() {
            let path = prefix.isEmpty ? "\(i)" : "\(prefix).\(i)"
            if let inner = item["items"]?.arrayValue { walk(inner, prefix: path, visit) } else { visit(path, item) }
        }
    }

    /// The ACTIONS rows: one per name, the item's own and its object's layers'.
    /// A name two of them carry shows once, as long as the longer.
    static func rows(own: ElementAnimation?, layers: [ElementAnimation]) -> [ActionRow] {
        var byName: [String: ActionRow] = [:]
        var order: [String] = []
        for row in (own?.actions ?? []) + layers.flatMap(\.actions) {
            if var seen = byName[row.name] {
                seen.duration = max(seen.duration, row.duration)
                byName[row.name] = seen
            } else {
                byName[row.name] = row
                order.append(row.name)
            }
        }
        return order.compactMap { byName[$0] }
    }

    /// The ms into an action at which Play shows a frame: 0, then every
    /// `frameStep` (more for a long action), all inside the action. After the
    /// last one the stage returns to rest.
    static func sampleTimes(duration: Double) -> [Int] {
        guard duration > 0 else { return [0] }
        let step = max(frameStep, (duration / Double(maxFrames)).rounded(.up))
        return Array(stride(from: 0.0, to: duration, by: step).map { Int($0) })
    }
}
