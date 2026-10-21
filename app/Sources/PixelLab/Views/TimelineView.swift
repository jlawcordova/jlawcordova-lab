import SwiftUI

/// The strip under the stage: Play/Pause, a scrubber over the scene's motion
/// loop with the gold playhead, the time, and (on demand) a row per animated
/// item with its keys.
struct TimelineStrip: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let timeline = model.timeline
        let isScene = model.document?.ref.kind == .scene
        let active = isScene && timeline.hasMotion
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                HStack(spacing: 10) {
                SectionLabel(title: "Timeline")
                Button {
                    timeline.toggle()
                } label: {
                    Image(systemName: timeline.playing ? "pause.fill" : "play.fill").frame(width: 18)
                }
                .buttonStyle(.borderless)
                .disabled(!active || timeline.reduceMotion)
                .keyboardShortcut("p", modifiers: [.command, .option])
                .help(timeline.reduceMotion ? "Reduce Motion is on: scrub to preview" : (timeline.playing ? "Pause (⌥⌘P)" : "Play (⌥⌘P)"))
                .accessibilityLabel(timeline.playing ? "Pause" : "Play")
                }
                .frame(width: TimelineStrip.leading, alignment: .leading)

                if active, let duration = timeline.duration {
                    Scrubber(time: timeline.time, duration: duration, keys: selectedKeys) { timeline.scrub(to: $0) }
                        .frame(height: 22)
                    HStack(spacing: 8) {
                    Spacer(minLength: 0)
                    Text("\(TimelineModel.label(timeline.time)) / \(TimelineModel.label(duration))\(timeline.hasAmbient ? " · loops" : "")")
                        .font(Theme.monoFont(11))
                        .foregroundStyle(Theme.inkMuted)
                        .monospacedDigit()
                        .fixedSize()
                    Button {
                        timeline.showTracks.toggle()
                    } label: {
                        Label("Tracks", systemImage: timeline.showTracks ? "chevron.down" : "list.bullet.indent")
                            .labelStyle(.iconOnly)
                    }
                    .buttonStyle(.borderless)
                    .help(timeline.showTracks ? "Hide the tracks" : "Show a track per animated item (ambient loops play on their own and have none)")
                    .accessibilityLabel(timeline.showTracks ? "Hide tracks" : "Show tracks")
                    }
                    .frame(width: TimelineStrip.trailing)
                } else {
                    Capsule().fill(Theme.border).frame(height: 3).frame(maxWidth: .infinity)
                    Text(isScene ? "No motion or loops" : "—")
                        .font(Theme.monoFont(11))
                        .foregroundStyle(Theme.inkMuted)
                }
            }
            .padding(.horizontal, 14)
            .frame(height: 40)

            if active, timeline.showTracks, let duration = timeline.duration {
                TracksList(tracks: timeline.tracks, duration: duration, time: timeline.time)
                    .frame(height: 132)
            }
        }
        .background(Theme.sidebar)
        .overlay(alignment: .top) { Rectangle().fill(Theme.border).frame(height: 1) }
    }

    static let leading: CGFloat = 118
    static let trailing: CGFloat = 132

    /// The selected item's key times, marked on the scrubber.
    private var selectedKeys: [Double] {
        guard let id = model.document?.selectedItem?.id else { return [] }
        return model.timeline.tracks.first { $0.item == id }?.keys ?? []
    }
}

/// The time slider: ticks every second, the selected item's keys, the gold
/// playhead. Click or drag anywhere to scrub; arrow keys step 100 ms.
struct Scrubber: View {
    var time: Double
    var duration: Double
    var keys: [Double]
    var scrub: (Double) -> Void

    var body: some View {
        GeometryReader { proxy in
            let w = proxy.size.width
            let x = { (t: Double) in w * CGFloat(t / max(duration, 1)) }
            ZStack(alignment: .leading) {
                Capsule().fill(Theme.border).frame(height: 3)
                Capsule().fill(Theme.gold.opacity(0.45)).frame(width: max(x(time), 0), height: 3)
                Canvas { context, size in
                    let seconds = Int(duration / 1000)
                    let every = seconds > 40 ? 5 : 1
                    for s in stride(from: 0, through: seconds, by: every) {
                        let tx = x(Double(s) * 1000)
                        let h: CGFloat = s % (every * 5) == 0 ? 9 : 5
                        context.fill(Path(CGRect(x: tx - 0.5, y: (size.height - h) / 2, width: 1, height: h)), with: .color(Theme.border))
                    }
                    for k in keys {
                        let kx = x(k)
                        var p = Path()
                        p.move(to: CGPoint(x: kx, y: size.height / 2 - 4))
                        p.addLine(to: CGPoint(x: kx + 4, y: size.height / 2))
                        p.addLine(to: CGPoint(x: kx, y: size.height / 2 + 4))
                        p.addLine(to: CGPoint(x: kx - 4, y: size.height / 2))
                        p.closeSubpath()
                        context.fill(p, with: .color(Theme.accent))
                    }
                }
                RoundedRectangle(cornerRadius: 1)
                    .fill(Theme.gold)
                    .frame(width: 3, height: 18)
                    .offset(x: x(time) - 1.5)
            }
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0).onChanged { v in
                    scrub(Double(min(max(v.location.x / max(w, 1), 0), 1)) * duration)
                }
            )
        }
        .accessibilityElement()
        .accessibilityLabel("Time")
        .accessibilityValue(TimelineModel.label(time))
        .accessibilityAdjustableAction { direction in
            scrub(time + (direction == .increment ? 100 : -100))
        }
    }
}

/// A row per motion track: the item's id and its keys on the same scale as
/// the scrubber. Clicking a row selects the item.
struct TracksList: View {
    var tracks: [MotionTrack]
    var duration: Double
    var time: Double
    @Environment(AppModel.self) private var model

    var body: some View {
        let selectedID = model.document?.selectedItem?.id
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(tracks) { track in
                    Button {
                        model.editor.select(id: track.item)
                    } label: {
                        HStack(spacing: 12) {
                            Text(track.item)
                                .font(Theme.monoFont(10.5))
                                .foregroundStyle(track.item == selectedID ? Theme.ink : Theme.inkMuted)
                                .lineLimit(1)
                                .frame(width: TimelineStrip.leading, alignment: .leading)
                            GeometryReader { proxy in
                                let w = proxy.size.width
                                Canvas { context, size in
                                    context.fill(Path(CGRect(x: 0, y: size.height / 2 - 0.5, width: size.width, height: 1)), with: .color(Theme.border))
                                    for k in track.keys {
                                        let kx = w * CGFloat(k / max(duration, 1))
                                        context.fill(Path(ellipseIn: CGRect(x: kx - 2.5, y: size.height / 2 - 2.5, width: 5, height: 5)),
                                                     with: .color(track.item == selectedID ? Theme.accent : Theme.inkMuted.opacity(0.7)))
                                    }
                                    let px = w * CGFloat(time / max(duration, 1))
                                    context.fill(Path(CGRect(x: px - 0.5, y: 0, width: 1, height: size.height)), with: .color(Theme.gold))
                                }
                            }
                            Text("\(track.keys.count) keys")
                                .font(Theme.monoFont(10))
                                .foregroundStyle(Theme.inkMuted.opacity(0.8))
                                .frame(width: TimelineStrip.trailing, alignment: .trailing)
                        }
                        .padding(.horizontal, 14)
                        .frame(height: 20)
                        .background(track.item == selectedID ? Theme.selection : .clear)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Track \(track.item), \(track.keys.count) keys")
                    .accessibilityAddTraits(track.item == selectedID ? .isSelected : [])
                }
            }
        }
        // No scroller: it would narrow the rows and misalign the keys with the scrubber.
        .scrollIndicators(.never)
        .overlay(alignment: .top) { Rectangle().fill(Theme.border).frame(height: 1) }
    }
}
