import SwiftUI

/// File › New Scene… (⌘N) and New Object… (⇧⌘N): the lab's New dialog. The
/// name is checked as you type; an object is a sprite of a width and height,
/// a block, or a sprite that extends character. Create makes it in the
/// session (`scene.new` / `object.new`) and opens it, unsaved until Save.
struct NewDocSheet: View {
    var draft: NewDocDraft
    @Environment(AppModel.self) private var model
    @FocusState private var nameFocused: Bool

    var body: some View {
        let taken = model.takenNames
        let problem = draft.problem(taken: taken)
        let showProblem = draft.serverError != nil || (problem != nil && (draft.tried || !draft.name.isEmpty))
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 8) {
                Image(systemName: draft.kind == .scene ? "square.stack.3d.up" : "cube").foregroundStyle(Theme.accent)
                Text(draft.kind == .scene ? "New Scene" : "New Object").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
            }
            VStack(alignment: .leading, spacing: 5) {
                Text("Name").font(.caption).foregroundStyle(Theme.inkMuted)
                TextField("Name", text: Binding(get: { draft.name }, set: { draft.name = $0; draft.serverError = nil }), prompt: Text(draft.kind == .scene ? "my-scene" : "small-rock"))
                    .textFieldStyle(.roundedBorder)
                    .font(Theme.monoFont(13))
                    .focused($nameFocused)
                    .onSubmit { Task { await model.createNewDoc() } }
                    .accessibilityLabel("Name")
                    .accessibilityHint(draft.help(taken: taken))
                Text(draft.help(taken: taken))
                    .font(.caption)
                    .foregroundStyle(showProblem ? Theme.ink : Theme.inkMuted)
                    .lineLimit(2)
                    .padding(.leading, showProblem ? 0 : 0)
                    .overlay(alignment: .leading) {
                        if showProblem {
                            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.gold).imageScale(.small).offset(x: -16)
                        }
                    }
            }
            if draft.kind == .object {
                Picker("Kind", selection: Binding(get: { draft.objectKind }, set: { draft.objectKind = $0 })) {
                    Text("Sprite").tag(NewDocDraft.ObjectKind.sprite)
                    Text("Block").tag(NewDocDraft.ObjectKind.block)
                }
                .pickerStyle(.segmented)
                .accessibilityLabel("Kind")
                if draft.objectKind == .sprite {
                    Toggle("Extends character (an outfit)", isOn: Binding(get: { draft.extendsCharacter }, set: { draft.extendsCharacter = $0 }))
                        .toggleStyle(.checkbox)
                        .foregroundStyle(Theme.ink)
                    HStack(spacing: 10) {
                        sizeField("Width", text: Binding(get: { draft.width }, set: { draft.width = $0 }))
                        Text("×").foregroundStyle(Theme.inkMuted).padding(.top, 16)
                        sizeField("Height", text: Binding(get: { draft.height }, set: { draft.height = $0 }))
                        Spacer()
                    }
                    .disabled(draft.extendsCharacter)
                    .opacity(draft.extendsCharacter ? 0.5 : 1)
                    Text(draft.extendsCharacter
                         ? "It takes character's size and layers; paint rows to override them."
                         : "1 to 64 pixels each, anchored at the bottom center so it stands on a tile.")
                        .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
                } else {
                    Text("A one-level grass block, lit like the island: light top, mid left, shadow right.")
                        .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
                }
            } else {
                Text("An empty 128×128 view around tile 0,0,0.")
                    .font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(2)
            }
            HStack {
                Spacer()
                Button("Cancel", role: .cancel) { model.newDoc = nil }
                    .keyboardShortcut(.cancelAction)
                Button {
                    Task { await model.createNewDoc() }
                } label: {
                    Text(draft.creating ? "Creating…" : "Create")
                }
                .buttonStyle(AccentButtonStyle())
                .keyboardShortcut(.defaultAction)
                .disabled(draft.creating || (draft.tried && problem != nil))
            }
            .padding(.top, 4)
        }
        .padding(22)
        .frame(width: 400)
        .background(Theme.sidebar)
        .onAppear { nameFocused = true }
    }

    private func sizeField(_ label: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).font(.caption).foregroundStyle(Theme.inkMuted)
            TextField(label, text: text)
                .textFieldStyle(.roundedBorder)
                .font(Theme.monoFont(12))
                .frame(width: 70)
                .accessibilityLabel(label)
        }
    }
}
