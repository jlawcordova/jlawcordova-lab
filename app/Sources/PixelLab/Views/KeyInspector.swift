import SwiftUI

/// ANIMATION, under POSITION for a selected scene item: Animate (a track,
/// and a 4 s loop if the scene has none) or the track's key count and Remove
/// Track; an item without an id gets the hint to give it one
/// (docs/intents/2026-10-animation-pane/spec.md, R8).
struct AnimationSection: View {
    var item: SceneItemInfo
    @Environment(AppModel.self) private var model

    var body: some View {
        let timeline = model.timeline
        InspectorSection("Animation") {
            if item.id == nil {
                Text("Give it an ID to animate it.")
                    .font(.caption)
                    .foregroundStyle(Theme.inkMuted)
            } else if let track = timeline.selectedTrack {
                HStack(spacing: 6) {
                    Text("\(track.keys.count) key\(track.keys.count == 1 ? "" : "s") · loop \(TimelineModel.label(timeline.duration ?? 0))")
                        .font(.caption)
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(1)
                    Spacer(minLength: 4)
                    ActionButton(title: "Remove Track", symbol: "minus.circle", help: "Remove the item's motion track and all its keys") {
                        Task { await timeline.removeTrack() }
                    }
                    .fixedSize()
                }
            } else {
                HStack(spacing: 6) {
                    Text(timeline.hasMotion ? "Not animated." : "Not animated; the scene has no motion yet.")
                        .font(.caption)
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(2)
                    Spacer(minLength: 4)
                    ActionButton(title: "Animate", symbol: "point.topleft.down.to.point.bottomright.curvepath", help: timeline.hasMotion ? "Add a motion track: a rest key at 0; then Add Key at the playhead" : "Add a motion track and a 4 s loop; then Add Key at the playhead") {
                        Task { await timeline.addTrack() }
                    }
                    .fixedSize()
                }
            }
        }
    }
}

/// KEY, under ANIMATION when the pane's selected key belongs to the selected
/// item: its time, Move X and Y, Show and Pose, each set on the key or
/// inherited from the key before (dimmed, "inherits", with Clear to go back
/// to inheriting), and the track's off-grid warnings (R7).
struct KeyInspector: View {
    var item: SceneItemInfo
    var key: MotionKey
    var filled: FilledKey
    @Environment(AppModel.self) private var model

    var body: some View {
        let timeline = model.timeline
        InspectorSection("Key") {
            HStack(spacing: 6) {
                if key.t == 0 {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Time").font(.caption).foregroundStyle(Theme.inkMuted)
                        Text("0 ms · rest frame")
                            .font(Theme.monoFont(12))
                            .foregroundStyle(Theme.inkMuted)
                            .padding(.horizontal, 7)
                            .frame(height: 24)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(Theme.card, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                    }
                    .accessibilityElement(children: .combine)
                    .accessibilityLabel("Time 0, the rest frame")
                } else {
                    CoordField(label: "Time (ms)", value: key.t) { v in Task { await timeline.moveKey(to: v) } } step: { d in
                        Task { await timeline.moveKey(to: key.t + d * Int(TimelineModel.step)) }
                    }
                }
            }
            HStack(alignment: .bottom, spacing: 6) {
                CoordField(label: "Move X", value: filled.move[0], dimmed: !filled.ownMove) { v in
                    Task { await timeline.setKey(move: [v, filled.move[1]]) }
                } step: { d in
                    Task { await timeline.setKey(move: [filled.move[0] + d, filled.move[1]]) }
                }
                CoordField(label: "Move Y", value: filled.move[1], dimmed: !filled.ownMove) { v in
                    Task { await timeline.setKey(move: [filled.move[0], v]) }
                } step: { d in
                    Task { await timeline.setKey(move: [filled.move[0], filled.move[1] + d]) }
                }
                clearButton(filled.ownMove, "Inherit the move from the key before") { Task { await timeline.setKey(clear: ["move"]) } }
            }
            if !filled.ownMove {
                Text("Inherits the move from the key before. Moving the item on the stage at this time sets it.")
                    .font(.caption)
                    .foregroundStyle(Theme.inkMuted)
                    .lineLimit(3)
            }
            HStack(spacing: 6) {
                Text("Show").font(.caption).foregroundStyle(Theme.inkMuted).frame(width: 34, alignment: .leading)
                Picker("Show", selection: Binding<Int>(get: { filled.ownShow ? (filled.show ? 1 : 2) : 0 }, set: { v in
                    Task {
                        switch v {
                        case 1: await timeline.setKey(show: true)
                        case 2: await timeline.setKey(show: false)
                        default: await timeline.setKey(clear: ["show"])
                        }
                    }
                })) {
                    Text("Inherit (\(filled.show ? "shown" : "hidden"))").tag(0)
                    Text("Shown").tag(1)
                    Text("Hidden").tag(2)
                }
                .labelsHidden()
                .controlSize(.small)
                .help("Whether the item shows from this key on; Hidden keeps it off the stage until a later key shows it")
                .accessibilityLabel("Show")
            }
            HStack(spacing: 6) {
                Text("Pose").font(.caption).foregroundStyle(Theme.inkMuted).frame(width: 34, alignment: .leading)
                Picker("Pose", selection: Binding<String>(get: { filled.ownPose ? (filled.pose ?? "") : "" }, set: { v in
                    Task { if v.isEmpty { await timeline.setKey(clear: ["pose"]) } else { await timeline.setKey(pose: v) } }
                })) {
                    Text("Inherit (\(filled.pose ?? item.object))").tag("")
                    Divider()
                    ForEach(model.objects) { object in
                        Text(object.name).tag(object.name)
                    }
                }
                .labelsHidden()
                .controlSize(.small)
                .help("The object the item draws from this key on")
                .accessibilityLabel("Pose")
                Thumb(image: model.thumbnails.images[filled.pose ?? item.object], side: 24)
            }
            if !timeline.lastWarnings.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(timeline.lastWarnings, id: \.self) { w in
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.gold).imageScale(.small)
                            Text(Self.trim(w)).font(.caption).foregroundStyle(Theme.ink).fixedSize(horizontal: false, vertical: true)
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
                .padding(.top, 2)
            }
        }
    }

    private func clearButton(_ own: Bool, _ help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: "xmark.circle").frame(width: 22, height: 24).contentShape(Rectangle())
        }
        .buttonStyle(.borderless)
        .disabled(!own)
        .help(help)
        .accessibilityLabel("Clear the move")
    }

    /// "scenes/x.mjs: motion.tracks[2] (cart): 4200 → 5000 ms moves …" → "4200 → 5000 ms moves …".
    static func trim(_ warning: String) -> String {
        guard let r = warning.range(of: "): ") else { return DocumentWindow.trimFile(warning) }
        return String(warning[r.upperBound...])
    }
}
