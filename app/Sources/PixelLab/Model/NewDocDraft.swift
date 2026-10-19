import Foundation
import Observation

/// The New Scene / New Object sheet's fields (the lab's New dialog,
/// dialogs.ts): a name checked as you type, and for an object a sprite of a
/// width and height, a block, or a sprite that extends character.
@MainActor @Observable
final class NewDocDraft: Identifiable {
    enum ObjectKind: String, CaseIterable, Identifiable {
        case sprite, block
        var id: String { rawValue }
    }

    let kind: DocRef.Kind
    var name = ""
    var objectKind: ObjectKind = .sprite
    var extendsCharacter = false
    var width = "16"
    var height = "16"
    /// Set when Create was pressed with a problem, so the sheet shows it.
    var tried = false
    var creating = false
    /// The server refused (it checks the name again).
    var serverError: String?
    nonisolated let id = UUID()

    init(kind: DocRef.Kind) { self.kind = kind }

    var trimmedName: String { name.trimmingCharacters(in: .whitespaces) }
    var isSprite: Bool { kind == .object && objectKind == .sprite }
    var sizeEditable: Bool { isSprite && !extendsCharacter }

    func problem(taken: Set<String>) -> String? {
        if let p = NewDocRules.nameProblem(trimmedName, taken: taken) { return p }
        if sizeEditable, let p = NewDocRules.sizeProblem(width: width, height: height) { return p }
        return nil
    }

    /// The help line under the name: the problem, or where it'll be saved.
    func help(taken: Set<String>) -> String {
        if let e = serverError { return e }
        if let p = problem(taken: taken) { return name.isEmpty && !tried ? "Lowercase words joined by hyphens, like small-rock." : p }
        return "It'll be saved at src/assets/pixel-art/source/\(kind.rawValue)s/\(trimmedName).mjs."
    }

    /// The command that makes it: `scene.new` or `object.new` with its params.
    var command: (method: String, params: [String: JSONValue]) {
        if kind == .scene { return ("scene.new", ["name": .string(trimmedName)]) }
        var p: [String: JSONValue] = ["name": .string(trimmedName), "kind": .string(objectKind.rawValue)]
        if objectKind == .sprite {
            if extendsCharacter {
                p["extends"] = "character"
            } else {
                p["width"] = JSONValue(Int(width.trimmingCharacters(in: .whitespaces)) ?? 16)
                p["height"] = JSONValue(Int(height.trimmingCharacters(in: .whitespaces)) ?? 16)
            }
        }
        return ("object.new", p)
    }
}

extension AppModel {
    /// Every scene and object name (a new doc can't take one).
    var takenNames: Set<String> { Set(scenes.map(\.name) + objects.map(\.name)) }

    /// File › New Scene… / New Object…
    func showNew(_ kind: DocRef.Kind) {
        guard phase == .ready else { return }
        newDoc = NewDocDraft(kind: kind)
    }

    /// Create in the sheet: makes the doc in the session (unsaved until Save)
    /// and opens it. Returns whether it made it.
    @discardableResult
    func createNewDoc() async -> Bool {
        guard let draft = newDoc, let connection else { return false }
        draft.tried = true
        draft.serverError = nil
        if draft.problem(taken: takenNames) != nil { return false }
        draft.creating = true
        defer { draft.creating = false }
        let (method, params) = draft.command
        do {
            let r: EditResult = try await connection.call(method, params)
            newDoc = nil
            await loadLists()
            let ref = DocRef(draft.kind, draft.trimmedName)
            await open(ref)
            lastEdit = (r.summary ?? "Created \(ref.id)") + ". It's a draft until you save it."
            editor.announce("Created \(draft.kind.rawValue) \(ref.name). It's a draft until you save it.")
            return true
        } catch {
            draft.serverError = error.localizedDescription
            return false
        }
    }
}
