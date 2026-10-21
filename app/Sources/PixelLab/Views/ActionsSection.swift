import SwiftUI

/// ACTIONS: one row per action of the selected scene item (its own and those
/// of its object's map layers) or of the selected object layer: its name, its
/// length and a Play button that shows it once on the stage
/// (docs/intents/2026-10-rust-engine/spec.md, R23, D10). Nothing shows for an
/// element with no actions.
struct ActionsSection: View {
    var rows: [ActionRow]
    @Environment(AppModel.self) private var model

    var body: some View {
        if !rows.isEmpty {
            InspectorSection("Actions") {
                VStack(spacing: 4) {
                    ForEach(rows) { row in
                        ActionRowView(row: row, playing: model.timeline.playingAction == row.name) {
                            Task { await model.timeline.playAction(row) }
                        }
                    }
                }
                Text(model.timeline.reduceMotion
                     ? "Reduce Motion is on: Play shows the rest frame."
                     : "Play shows it once on the stage, then the stage goes back to rest.")
                    .font(.caption)
                    .foregroundStyle(Theme.inkMuted)
                    .lineLimit(3)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct ActionRowView: View {
    var row: ActionRow
    var playing: Bool
    var play: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 1) {
                Text(row.name)
                    .font(Theme.nameFont(12.5))
                    .foregroundStyle(Theme.ink)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Text("\(row.durationLabel) · \(row.source)")
                    .font(Theme.monoFont(10.5))
                    .foregroundStyle(Theme.inkMuted)
                    .lineLimit(1)
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("\(row.name), \(row.durationLabel)")
            Spacer(minLength: 0)
            Button(action: play) {
                Image(systemName: "play.fill")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(playing ? Theme.onAccent : Theme.accent)
                    .frame(width: 36, height: 30)
                    .background(playing ? Theme.accent : Theme.card, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(playing ? Theme.accent : Theme.border, lineWidth: 1))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Play \(row.name) once on the stage")
            .accessibilityLabel("Play \(row.name)")
            .accessibilityHint("Shows the action once on the stage")
        }
        .padding(.horizontal, 8)
        .frame(minHeight: 40)
        .background(Theme.card.opacity(0.5), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
    }
}
