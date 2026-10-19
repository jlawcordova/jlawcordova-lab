import SwiftUI

/// The bar over the stage when the open doc has unsaved edits and its file
/// changed on disk (docs/intents/2026-10-mcp-gaps/spec.md, R5): Merge (the
/// default), Reload or Keep Mine. Board 03 tokens; nothing animates.
struct StaleBanner: View {
    @Environment(AppModel.self) private var model
    var file: String

    /// What the bar says, and what VoiceOver reads for it.
    nonisolated static func message(file: String) -> String {
        "\(file) changed on disk. You have unsaved edits here."
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
                Text("Merge keeps your edits and brings in the change. Reload drops your edits.")
                    .font(.caption)
                    .foregroundStyle(Theme.inkMuted)
            }
            .lineLimit(2)
            Spacer(minLength: 8)
            Button("Keep Mine") { model.keepMine() }
                .help("Hide this; saving later asks before it overwrites the file")
            Button("Reload") { Task { await model.reloadFromDisk() } }
                .help("Load the file as it is now, dropping your unsaved edits (Undo brings them back)")
            Button("Merge") { Task { await model.mergeFromDisk() } }
                .buttonStyle(.borderedProminent)
                .help("Bring the change on disk into your open copy, keeping your edits")
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
