import CoreGraphics
import Foundation

// The shapes of the proposal commands (docs/cli.md, "Proposals and
// activity") and the ghost preview: a proposal's doc rendered without
// writing, drawn over the stage, and what changed in it.

/// One proposal of `proposal.list` / `proposals.changed`.
struct ProposalSummary: Codable, Sendable, Hashable, Identifiable {
    var id: String
    var doc: String
    var summary: [String]
    var createdAt: String
    var agent: String?
    var baseMatches: Bool

    var ref: DocRef? { DocRef(id: doc) }
    /// A rewrite of the same proposal (the agent saved again) has a new createdAt.
    var version: String { "\(id)@\(createdAt)" }
    var agentName: String { agent ?? "an agent" }
    var editsLabel: String { "\(summary.count) edit\(summary.count == 1 ? "" : "s")" }
}

struct ProposalList: Codable, Sendable { var proposals: [ProposalSummary] }

/// `proposal.show`: the summary plus the source and a diff from the file now.
struct ProposalDetail: Codable, Sendable {
    var id: String
    var doc: String
    var summary: [String]
    var createdAt: String
    var agent: String?
    var baseMatches: Bool
    var baseHash: String?
    var file: String
    var exists: Bool
    var currentHash: String?
    var source: String
    var diff: String
}

/// `proposal.apply`.
struct ProposalApplyResult: Codable, Sendable {
    var id: String
    var doc: String
    var file: String
    var wrote: Bool
    var reloaded: Bool
    /// The proposal was merged into the app's open copy, keeping its unsaved edits.
    var merged: Bool?
    /// The open copy's unsaved edits clash with it: the file is written, the copy is left stale.
    var conflict: Bool?
    var summary: String?
    var forcedProblems: [String]?
}

/// `proposal.render`: a render result (as `scene.render` / `object.render`) of the proposed doc.
struct ProposalRenderResult: Codable, Sendable {
    var id: String
    var doc: String
    var kind: String
    var baseMatches: Bool
    var png: Data
    var scale: Int
    var viewBox: [Double]
    var width: Int
    var height: Int
    var time: Double?
    var frame: Int?
    var anchor: [Double]?
    var items: [RenderItem]
}

/// One line of activity.jsonl.
struct ActivityEntry: Codable, Sendable, Hashable {
    var time: String
    var tool: String
    var ok: Bool
    var summary: String?
}

struct ActivityList: Codable, Sendable { var entries: [ActivityEntry] }

/// The selected proposal, rendered for the doc open on the stage.
struct ProposalGhost {
    /// What it was rendered for: proposal version, doc, time or frame and layers.
    var key: String
    var proposalID: String
    var ref: DocRef
    var image: StageImage
    /// Its pixels (objects), for the changed-pixel marks.
    var colors: PixelColors?
}

/// What a proposal changes on the stage.
struct GhostChange: Hashable, Sendable {
    enum Kind: String, Sendable { case added, moved, removed }
    var kind: Kind
    var object: String
    /// The item's path in the render it comes from (the proposal's, or the current one's for removed items).
    var path: String?
    /// Output-pixel bounds in that render.
    var bounds: [Double]?
}

enum GhostDiff {
    /// An item's identity for matching: object, id and where it is placed.
    static func key(_ item: RenderItem) -> String {
        "\(item.object)|\(item.id ?? "")|\(canonical(item.at))"
    }

    /// JSON with sorted keys, so `{tile: [1, 2, 0]}` reads the same however it was decoded.
    static func canonical(_ v: JSONValue?) -> String {
        guard let v else { return "null" }
        switch v {
        case .null: return "null"
        case .bool(let b): return b ? "true" : "false"
        case .number(let n): return n.rounded() == n && abs(n) < 1e15 ? String(Int64(n)) : String(n)
        case .string(let s): return "\"\(s)\""
        case .array(let a): return "[" + a.map { canonical($0) }.joined(separator: ",") + "]"
        case .object(let o): return "{" + o.keys.sorted().map { "\($0):\(canonical(o[$0]))" }.joined(separator: ",") + "}"
        }
    }

    /// Matches the proposed render's items to the current render's: an item
    /// with the same object, id and placement is unchanged (paths may shift as
    /// items come and go). Of the rest, a proposed item that pairs with a
    /// leftover current one (same id, else same object; same path first) is
    /// moved, else added; leftover current items are removed.
    static func compare(current: [RenderItem], proposed: [RenderItem]) -> [GhostChange] {
        var pool: [String: [Int]] = [:]
        for (i, item) in current.enumerated() { pool[key(item), default: []].append(i) }
        var leftProposed: [RenderItem] = []
        for item in proposed {
            let k = key(item)
            if var list = pool[k], !list.isEmpty {
                list.removeFirst()
                pool[k] = list
            } else {
                leftProposed.append(item)
            }
        }
        var leftCurrent = pool.values.flatMap { $0 }.sorted().map { current[$0] }
        var out: [GhostChange] = []
        for item in leftProposed {
            let candidates = leftCurrent.indices.filter { j in
                let c = leftCurrent[j]
                if let id = item.id { return c.id == id }
                return c.object == item.object && c.id == nil
            }
            if let j = candidates.first(where: { leftCurrent[$0].path == item.path }) ?? candidates.first {
                leftCurrent.remove(at: j)
                out.append(GhostChange(kind: .moved, object: item.object, path: item.path, bounds: item.bounds))
            } else {
                out.append(GhostChange(kind: .added, object: item.object, path: item.path, bounds: item.bounds))
            }
        }
        for item in leftCurrent {
            out.append(GhostChange(kind: .removed, object: item.object, path: item.path, bounds: item.bounds))
        }
        return out
    }

    /// The pixels (in scene pixels, the object's anchor at 0, 0) whose color
    /// differs between two renders at scale 1 whose top-left pixels sit at
    /// `currentOrigin` and `proposedOrigin`.
    static func changedPixels(current: PixelColors?, currentOrigin: CGPoint, proposed: PixelColors?, proposedOrigin: CGPoint) -> [PixelPoint] {
        let a = rect(current, currentOrigin), b = rect(proposed, proposedOrigin)
        let union = a.union(b)
        guard !union.isNull, union.width > 0, union.height > 0, union.width * union.height <= 1_000_000 else { return [] }
        var out: [PixelPoint] = []
        for y in Int(union.minY)..<Int(union.maxY) {
            for x in Int(union.minX)..<Int(union.maxX) {
                let was = current?.hex(PixelPoint(x - Int(currentOrigin.x), y - Int(currentOrigin.y)))
                let now = proposed?.hex(PixelPoint(x - Int(proposedOrigin.x), y - Int(proposedOrigin.y)))
                if was != now { out.append(PixelPoint(x, y)) }
            }
        }
        return out
    }

    private static func rect(_ c: PixelColors?, _ origin: CGPoint) -> CGRect {
        guard let c else { return .null }
        return CGRect(x: origin.x.rounded(), y: origin.y.rounded(), width: CGFloat(c.width), height: CGFloat(c.height))
    }
}

/// One line of a unified diff, for the Agent window's diff view.
struct DiffLine: Hashable, Sendable, Identifiable {
    enum Kind: Sendable { case file, hunk, added, removed, context }
    var id: Int
    var kind: Kind
    var text: String

    static func parse(_ diff: String) -> [DiffLine] {
        diff.split(separator: "\n", omittingEmptySubsequences: false).enumerated().compactMap { i, sub in
            let s = String(sub)
            if s.isEmpty, i > 0 { return nil }
            let kind: Kind
            if s.hasPrefix("+++") || s.hasPrefix("---") { kind = .file }
            else if s.hasPrefix("@@") { kind = .hunk }
            else if s.hasPrefix("+") { kind = .added }
            else if s.hasPrefix("-") { kind = .removed }
            else { kind = .context }
            return DiffLine(id: i, kind: kind, text: s)
        }
    }
}

/// Times from the server (ISO 8601, UTC) as the Agent window shows them.
enum AgentTime {
    static func date(_ iso: String) -> Date? {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let d = f.date(from: iso) { return d }
        f.formatOptions = [.withInternetDateTime]
        return f.date(from: iso)
    }

    /// "14:23:05" (local).
    static func clock(_ iso: String) -> String {
        guard let d = date(iso) else { return iso }
        return d.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute(.twoDigits).second(.twoDigits))
    }

    /// "14:23" today, else "Oct 18, 14:23".
    static func short(_ iso: String, now: Date = Date()) -> String {
        guard let d = date(iso) else { return iso }
        let time = d.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute(.twoDigits))
        if Calendar.current.isDate(d, inSameDayAs: now) { return time }
        return d.formatted(.dateTime.month(.abbreviated).day()) + ", " + time
    }
}
