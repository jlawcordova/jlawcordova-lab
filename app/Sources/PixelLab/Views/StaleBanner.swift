import SwiftUI

/// The bar over the stage when the open doc's file changed on disk and the change clashes with edits
/// here that weren't saved yet (docs/intents/2026-10-autosave/spec.md, R7): a change that merges is
/// merged and saved without asking, so only a clash is left, with Reload or Keep Mine. Board 03
/// tokens; nothing animates.
struct StaleBanner: View {
    @Environment(AppModel.self) private var model
    var file: String

    /// What the bar says, and what VoiceOver reads for it.
    nonisolated static func message(file: String) -> String {
        "\(file) changed on disk, on the same lines as your edits here."
    }

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(Theme.gold)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 1) {
                Text(Self.message(file: file))
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Theme.ink)
                Text("Reload takes the file and drops your edits (Undo brings them back). Keep Mine saves your copy over it.")
                    .font(.caption)
                    .foregroundStyle(Theme.inkMuted)
            }
            .lineLimit(2)
            Spacer(minLength: 8)
            Button("Reload") { Task { await model.reloadFromDisk() } }
                .help("Load the file as it is now, dropping your unsaved edits (Undo brings them back)")
            Button("Keep Mine") { Task { await model.keepMine() } }
                .buttonStyle(.borderedProminent)
                .help("Save your open copy over the file")
        }
        .controlSize(.regular)
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(Theme.card)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.border).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Self.message(file: file))
    }
}
