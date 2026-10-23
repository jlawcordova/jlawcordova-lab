import Foundation

/// A tool call as a plain sentence, and as the raw call the Details toggle
/// shows (docs/intents/2026-10-agent-chat/spec.md, D9).
enum StepText {
    /// The sentence. `tool` without its `mcp__pixellab__` prefix; `result` the
    /// result's text (pixellab's tools return JSON); `objectAt` names the
    /// object at a path when the result doesn't.
    static func human(tool: String, input: JSONValue, result: String?, ok: Bool, objectAt: (String) -> String? = { _ in nil }) -> String {
        let r = result.flatMap { $0.data(using: .utf8) }.flatMap { try? JSONDecoder().decode(JSONValue.self, from: $0) }
        if !ok {
            let why = (r?["error"]?["message"]?.stringValue ?? result ?? "").split(separator: "\n").first.map(String.init) ?? ""
            return "Couldn't \(verb(tool))" + (why.isEmpty ? "" : ": \(why)")
        }
        let s = { (k: String) in input[k]?.stringValue }
        let item = s("item")
        let object = r?["item"]?["object"]?.stringValue ?? r?["object"]?.stringValue ?? item.flatMap(objectAt)
        let label = [object, item].compactMap { $0 }.joined(separator: " ")
        let named = label.isEmpty ? "the item" : label
        let count = { (v: JSONValue?) in v?.arrayValue?.count ?? 0 }
        switch tool {
        case "scene_items":
            return "Looked through the \(count(r?["items"])) items in \(r?["scene"]?.stringValue ?? s("name") ?? "the scene")"
        case "scene_render":
            return "Rendered \(r?["scene"]?.stringValue ?? s("name") ?? "the scene") to check how it looks"
        case "scene_show":
            return "Looked at \(s("name") ?? "the scene")"
        case "scene_list":
            return "Listed the \(count(r?["scenes"])) scenes"
        case "object_list":
            return "Listed the \(count(r?["objects"])) objects"
        case "document_info":
            return "Read the document's details"
        case "palette_get":
            return "Read the palette"
        case "object_show", "object_render":
            let name = r?["name"]?.stringValue ?? r?["object"]?.stringValue ?? s("name") ?? "the object"
            if let size = r?["size"]?.arrayValue, size.count == 2, let w = size[0].doubleValue, let h = size[1].doubleValue {
                return "Looked at the \(name) \(r?["kind"]?.stringValue == "block" ? "block" : "sprite") (\(Int(w))×\(Int(h)))"
            }
            return "Looked at \(name)"
        case "scene_move":
            if let n = ints(input["nudge"]) { return "Nudged \(named) by \(pixels(n))" }
            return "Moved \(named) \(direction(ints(input["by"]) ?? []))"
        case "scene_set":
            if let level = input["level"]?.doubleValue { return "Set \(named)'s level to \(Int(level))" }
            if let id = s("id") { return "Gave \(named) the id \(id)" }
            if let c = s("class") { return "Gave \(named) the class \(c)" }
            if let depth = s("depth") { return depth == "front" ? "Drew \(named) over the rest" : depth == "back" ? "Drew \(named) under the rest" : "Put \(named) back in depth order" }
            if let tile = ints(input["tile"]) { return "Moved \(named) to \(tile.map(String.init).joined(separator: ","))" }
            return r?["summary"]?.stringValue.map(sentence) ?? "Changed \(named)"
        case "scene_place":
            let o = s("object") ?? object ?? "an item"
            if let at = ints(input["at"]) ?? ints(input["tile"]) { return "Placed \(article(o)) \(o) at \(at.map(String.init).joined(separator: ","))" }
            if let px = ints(input["px"]) { return "Placed \(article(o)) \(o) at x \(px.first ?? 0), y \(px.last ?? 0)" }
            return "Placed \(article(o)) \(o)"
        case "scene_remove":
            return "Removed \(named)"
        case "scene_duplicate":
            return "Duplicated \(named)"
        case "scene_layer_new":
            let name = r?["layer"]?["name"]?.stringValue ?? s("layerName") ?? "a group"
            let n = r?["items"]?.doubleValue.map { Int($0) } ?? count(input["items"])
            return "Grouped \(n) item\(n == 1 ? "" : "s") as \(name)"
        case "scene_layer_move":
            return "Moved \(s("layer") ?? "the group") \(direction(ints(input["by"]) ?? []))"
        case "scene_layer_rename":
            return "Renamed \(s("layer") ?? "the group") to \(s("to") ?? "")"
        case "scene_layer_ungroup":
            return "Ungrouped \(s("layer") ?? "the group")"
        case "scene_layer_assign":
            let n = count(input["items"]) + count(input["layers"])
            return "Moved \(n) thing\(n == 1 ? "" : "s") into \((s("to") ?? "").isEmpty ? "the top level" : s("to")!)"
        case "object_paint":
            let n = count(input["pixels"])
            return "Painted \(n) pixel\(n == 1 ? "" : "s") \(s("color") ?? "") in \(s("name") ?? "the object")".replacingOccurrences(of: "  ", with: " ")
        case "object_erase":
            let n = count(input["pixels"])
            return "Erased \(n) pixel\(n == 1 ? "" : "s") in \(s("name") ?? "the object")"
        case "object_fill":
            return "Filled \(s("color") ?? "a color") in \(s("name") ?? "the object")"
        case "object_resize":
            let w = input["width"]?.doubleValue.map { String(Int($0)) } ?? "?", h = input["height"]?.doubleValue.map { String(Int($0)) } ?? "?"
            return "Made \(s("name") ?? "the object") \(w)×\(h)"
        case "object_new":
            return "Made a new object, \(s("name") ?? "")"
        case "doc_save":
            return "Saved \(s("doc").flatMap { DocRef(id: $0)?.name } ?? "it")"
        case "history_undo":
            return "Took back version \(input["version"]?.doubleValue.map { String(Int($0)) } ?? "")"
        case "history_restore":
            return "Restored version \(input["version"]?.doubleValue.map { String(Int($0)) } ?? "")"
        case "check":
            let name = r?["target"]?["name"]?.stringValue ?? s("name") ?? "the document"
            let n = count(r?["problems"])
            return "Checked \(name): \(n == 0 ? "no problems" : "\(n) problem\(n == 1 ? "" : "s")")"
        default:
            if let summary = r?["summary"]?.stringValue, !summary.isEmpty { return sentence(summary) }
            return "Used \(tool.replacingOccurrences(of: "_", with: " "))"
        }
    }

    /// `scene_move name=harbor item=134.0 by=1,0`.
    static func raw(tool: String, input: JSONValue, result: String?) -> String {
        var parts = [tool]
        if case .object(let o) = input {
            for key in o.keys.sorted(by: { order($0) < order($1) || (order($0) == order($1) && $0 < $1) }) {
                parts.append("\(key)=\(value(o[key]!))")
            }
        }
        return parts.joined(separator: " ")
    }

    /// name, doc and item first, the rest alphabetical.
    private static func order(_ key: String) -> Int { ["name": 0, "doc": 1, "item": 2, "object": 3][key] ?? 9 }

    private static func value(_ v: JSONValue) -> String {
        switch v {
        case .string(let s): return s.contains(" ") ? "\"\(s)\"" : s
        case .number(let n): return n.rounded() == n ? String(Int(n)) : String(n)
        case .bool(let b): return String(b)
        case .null: return "null"
        case .array(let a) where a.allSatisfy({ $0.doubleValue != nil }): return a.map(value).joined(separator: ",")
        default:
            let data = (try? JSONEncoder().encode(v)) ?? Data()
            return String(decoding: data, as: UTF8.self)
        }
    }

    private static func ints(_ v: JSONValue?) -> [Int]? {
        if let a = v?.arrayValue, !a.isEmpty, a.allSatisfy({ $0.doubleValue != nil }) { return a.map { Int($0.doubleValue!) } }
        if let s = v?.stringValue {
            let parts = s.split(separator: ",").compactMap { Int($0.trimmingCharacters(in: .whitespaces)) }
            return parts.isEmpty ? nil : parts
        }
        return nil
    }

    /// The app's nudge directions: col +1 down-right, col −1 up-left, row +1 down-left, row −1 up-right.
    static func direction(_ by: [Int]) -> String {
        let c = by.count > 0 ? by[0] : 0, r = by.count > 1 ? by[1] : 0, l = by.count > 2 ? by[2] : 0
        let tiles = { (n: Int, way: String) in "\(n == 1 ? "one tile" : "\(n) tiles") \(way)" }
        switch (c, r, l) {
        case let (c, 0, 0) where c > 0: return tiles(c, "down-right")
        case let (c, 0, 0) where c < 0: return tiles(-c, "up-left")
        case let (0, r, 0) where r > 0: return tiles(r, "down-left")
        case let (0, r, 0) where r < 0: return tiles(-r, "up-right")
        case let (0, 0, l) where l > 0: return l == 1 ? "up a level" : "up \(l) levels"
        case let (0, 0, l) where l < 0: return l == -1 ? "down a level" : "down \(-l) levels"
        default: return "by \(by.map(String.init).joined(separator: ","))"
        }
    }

    private static func pixels(_ d: [Int]) -> String {
        let x = d.first ?? 0, y = d.count > 1 ? d[1] : 0
        var parts: [String] = []
        if x != 0 { parts.append("\(abs(x)) px \(x > 0 ? "right" : "left")") }
        if y != 0 { parts.append("\(abs(y)) px \(y > 0 ? "down" : "up")") }
        return parts.isEmpty ? "0 px" : parts.joined(separator: " and ")
    }

    private static func article(_ word: String) -> String { "aeiou".contains(word.first ?? "x") ? "an" : "a" }

    /// A summary as a sentence: its first letter capitalized.
    private static func sentence(_ s: String) -> String { s.prefix(1).uppercased() + s.dropFirst() }

    private static func verb(_ tool: String) -> String {
        switch tool {
        case "scene_move", "scene_layer_move": "move it"
        case "scene_set": "change it"
        case "scene_place": "place it"
        case "scene_remove": "remove it"
        case "doc_save": "save it"
        case "object_paint", "object_fill", "object_erase": "paint it"
        case "scene_render", "object_render": "render it"
        default: "run \(tool.replacingOccurrences(of: "_", with: " "))"
        }
    }
}
