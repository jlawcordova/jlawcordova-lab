import SwiftUI

/// The strip under the stage: Play/Pause, a scrubber over the scene's motion
/// loop with the gold playhead, the time, and (on demand) the animation
/// pane: a row per animated item with its keys, to select, add, delete and
/// drag them (docs/intents/2026-10-animation-pane/spec.md, D3).
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
                    Text("\(TimelineModel.label(timeline.time)) / \(TimelineModel.label(duration))")
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
                    .help(timeline.showTracks ? "Hide the animation pane" : "Show the animation pane: a track per animated item, with its keys")
                    .accessibilityLabel(timeline.showTracks ? "Hide animation pane" : "Show animation pane")
                    }
                    .frame(width: TimelineStrip.trailing)
                } else {
                    Capsule().fill(Theme.border).frame(height: 3).frame(maxWidth: .infinity)
                    Text(isScene ? "No motion" : "—")
                        .font(Theme.monoFont(11))
                        .foregroundStyle(Theme.inkMuted)
                }
            }
            .padding(.horizontal, 14)
            .frame(height: 40)

            if active, timeline.showTracks, let duration = timeline.duration {
                AnimationPane(duration: duration)
                    .frame(height: TimelineStrip.paneHeight)
            }
        }
        .background(Theme.sidebar)
        .overlay(alignment: .top) { Rectangle().fill(Theme.border).frame(height: 1) }
    }

    static let leading: CGFloat = 118
    static let trailing: CGFloat = 132
    static let paneHeight: CGFloat = 160

    /// The selected item's key times, marked on the scrubber.
    private var selectedKeys: [Double] {
        model.timeline.selectedTrack?.times ?? []
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
                        context.fill(KeyGlyph.diamond(at: CGPoint(x: x(k), y: size.height / 2), size: 8), with: .color(Theme.accent))
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

/// The animation pane: its toolbar (the loop's length, previous and next
/// key, Add Key, Delete Key) over a row per motion track. It takes keys when
/// focused (click in it, or Tab): ← → step the playhead, ⇧← ⇧→ walk the keys,
/// K adds, ⌫ deletes, Space plays, Esc deselects.
struct AnimationPane: View {
    var duration: Double
    @Environment(AppModel.self) private var model
    @FocusState private var focused: Bool

    var body: some View {
        let timeline = model.timeline
        VStack(spacing: 0) {
            PaneToolbar(duration: duration)
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(timeline.tracks) { track in
                            TrackRow(track: track, duration: duration) { focused = true }
                                .id(track.item)
                        }
                    }
                }
                // No scroller: it would narrow the rows and misalign the keys with the scrubber.
                .scrollIndicators(.never)
                .onChange(of: timeline.selectedItemID) { _, id in
                    if let id, timeline.track(id) != nil { proxy.scrollTo(id) }
                }
            }
        }
        .focusable()
        .focused($focused)
        .focusEffectDisabled()
        .onChange(of: focused) { _, f in timeline.paneFocused = f }
        .onKeyPress(phases: .down) { press in
            guard let input = KeyInput(press: press) else { return .ignored }
            return timeline.handleKey(input) ? .handled : .ignored
        }
        .overlay {
            // Visible focus, as the stage's.
            Rectangle()
                .strokeBorder(Theme.accent.opacity(focused ? 0.55 : 0), lineWidth: 2)
                .allowsHitTesting(false)
        }
        .overlay(alignment: .top) { Rectangle().fill(Theme.border).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Animation pane")
        .accessibilityHint("Arrow keys step the playhead, Shift-arrows walk the keys, K adds a key, Delete removes it")
    }
}

/// The pane's toolbar: Loop (the length, in seconds), then the key buttons.
struct PaneToolbar: View {
    var duration: Double
    @Environment(AppModel.self) private var model

    var body: some View {
        let timeline = model.timeline
        let seconds = duration / 1000
        let loop = Binding<Double>(get: { seconds }, set: { s in
            Task { await timeline.setDuration(Int((s * 1000).rounded())) }
        })
        let format = FloatingPointFormatStyle<Double>.number.precision(.fractionLength(1))
        HStack(spacing: 12) {
            HStack(spacing: 6) {
                Text("Loop").font(.caption).foregroundStyle(Theme.inkMuted)
                TextField("Loop", value: loop, format: format)
                    .textFieldStyle(.plain)
                    .font(Theme.monoFont(11))
                    .foregroundStyle(Theme.ink)
                    .multilineTextAlignment(.trailing)
                    .padding(.horizontal, 6)
                    .frame(width: 50, height: 20)
                    .background(Theme.card, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                    // A new field when the loop changes elsewhere, or a length was refused.
                    .id("\(duration)-\(timeline.loopFieldGeneration)")
                    .help("The loop's length in seconds; a length that would cut off keys is refused")
                    .accessibilityLabel("Loop length in seconds")
                Text("s").font(.caption).foregroundStyle(Theme.inkMuted)
            }
            .frame(width: TimelineStrip.leading, alignment: .leading)
            Spacer(minLength: 0)
            HStack(spacing: 2) {
                paneButton("backward.end", "Previous key of the selected item (⇧← in the pane, ⌥⌘←)", "Previous key", enabled: timeline.selectedTrack != nil) { timeline.stepKey(-1) }
                paneButton("forward.end", "Next key of the selected item (⇧→ in the pane, ⌥⌘→)", "Next key", enabled: timeline.selectedTrack != nil) { timeline.stepKey(1) }
                Rectangle().fill(Theme.border).frame(width: 1, height: 14).padding(.horizontal, 4)
                paneButton("plus", "Add a key at the playhead for the selected item, pinned where it is (K in the pane, ⌥⌘K)", "Add key", enabled: timeline.canAddKey) {
                    Task { await timeline.addKey() }
                }
                paneButton("minus", timeline.selectedKey?.t == 0 ? "The key at 0 is the rest frame; Remove Track drops it" : "Delete the selected key (⌫ in the pane, ⌥⌘⌫)", "Delete key", enabled: timeline.canDeleteKey) {
                    Task { await timeline.deleteKey() }
                }
            }
            if let sk = timeline.selectedKey, let key = timeline.track(sk.item)?.key(at: sk.t) {
                Text("\(sk.item) · \(TimelineModel.label(Double(sk.t))) · \(key.spoken)")
                    .font(Theme.monoFont(10.5))
                    .foregroundStyle(Theme.inkMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .frame(width: TimelineStrip.trailing + 120, alignment: .trailing)
            } else {
                Text("No key selected")
                    .font(Theme.monoFont(10.5))
                    .foregroundStyle(Theme.inkMuted.opacity(0.7))
                    .frame(width: TimelineStrip.trailing + 120, alignment: .trailing)
            }
        }
        .padding(.horizontal, 14)
        .frame(height: 28)
    }

    private func paneButton(_ symbol: String, _ help: String, _ label: String, enabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).frame(width: 24, height: 22).contentShape(Rectangle())
        }
        .buttonStyle(.borderless)
        .disabled(!enabled)
        .help(help)
        .accessibilityLabel(label)
    }
}

/// A row per motion track: the item's id, its keys on the scrubber's scale
/// (a diamond each: hollow when the key hides the item, a dot above for a
/// pose; the selected track in the accent, the selected key larger), the
/// playhead, and the key count. Click the name to select the item, a key to
/// select it (and go there), drag a key to move it in time.
struct TrackRow: View {
    var track: MotionTrack
    var duration: Double
    var focus: () -> Void
    @Environment(AppModel.self) private var model

    var body: some View {
        let timeline = model.timeline
        let selected = track.item == timeline.selectedItemID
        let n = track.keys.count
        HStack(spacing: 12) {
            Button {
                focus()
                timeline.selectTrack(track.item)
            } label: {
                Text(track.item)
                    .font(Theme.monoFont(10.5))
                    .foregroundStyle(selected ? Theme.ink : Theme.inkMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .frame(width: TimelineStrip.leading, alignment: .leading)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Select \(track.item) on the stage")
            .accessibilityLabel("Track \(track.item), \(n) key\(n == 1 ? "" : "s")")
            .accessibilityAddTraits(selected ? .isSelected : [])

            GeometryReader { proxy in
                let w = proxy.size.width
                KeysCanvas(track: track, duration: duration, time: timeline.time, selectedTrack: selected,
                           selectedKey: timeline.selectedKey?.item == track.item ? timeline.selectedKey?.t : nil,
                           drag: timeline.keyDrag?.item == track.item ? timeline.keyDrag : nil)
                    .contentShape(Rectangle())
                    .gesture(DragGesture(minimumDistance: 0)
                        .onChanged { v in
                            if timeline.rowPress == nil {
                                focus()
                                let hit = TimelineModel.keyIndex(atX: v.startLocation.x, width: w, keys: track.keys.map(\.t), duration: duration)
                                timeline.rowPress = RowPress(item: track.item, key: hit.map { track.keys[$0].t })
                            }
                            guard let key = timeline.rowPress?.key, key != 0, abs(v.translation.width) > 3 else { return }
                            let to = TimelineModel.snap(TimelineModel.time(atX: v.location.x, width: w, duration: duration), duration: duration)
                            let d = KeyDrag(item: track.item, from: key, to: to)
                            if timeline.keyDrag != d { timeline.keyDrag = d }
                        }
                        .onEnded { _ in
                            let press = timeline.rowPress
                            timeline.rowPress = nil
                            if let drag = timeline.keyDrag {
                                timeline.keyDrag = nil
                                timeline.selectKey(drag.item, drag.from, scrub: drag.to == drag.from)
                                if drag.to != drag.from { Task { await timeline.moveKey(to: drag.to) } }
                            } else if let key = press?.key {
                                timeline.selectKey(track.item, key)
                            } else {
                                timeline.selectTrack(track.item)
                            }
                        })
                    .accessibilityElement(children: .contain)
                    .accessibilityLabel("Keys of \(track.item)")
                    .accessibilityChildren {
                        ForEach(track.keys) { key in
                            Button("Key at \(TimelineModel.label(Double(key.t))): \(key.spoken)") { timeline.selectKey(track.item, key.t) }
                                .accessibilityAddTraits(timeline.selectedKey == SelectedKey(item: track.item, t: key.t) ? .isSelected : [])
                        }
                    }
            }
            Text("\(n) key\(n == 1 ? "" : "s")")
                .font(Theme.monoFont(10))
                .foregroundStyle(Theme.inkMuted.opacity(0.8))
                .frame(width: TimelineStrip.trailing, alignment: .trailing)
                .accessibilityHidden(true)
        }
        .padding(.horizontal, 14)
        .frame(height: 22)
        .background(selected ? Theme.selection : .clear)
    }
}

/// The keys of one row, drawn.
struct KeysCanvas: View {
    var track: MotionTrack
    var duration: Double
    var time: Double
    var selectedTrack: Bool
    var selectedKey: Int?
    var drag: KeyDrag?

    var body: some View {
        Canvas { context, size in
            let w = size.width
            let mid = size.height / 2
            let x = { (t: Int) in TimelineModel.x(of: Double(t), width: w, duration: duration) }
            context.fill(Path(CGRect(x: 0, y: mid - 0.5, width: w, height: 1)), with: .color(Theme.border))
            let color = selectedTrack ? Theme.accent : Theme.inkMuted.opacity(0.7)
            for key in track.keys {
                let isSelected = key.t == selectedKey
                let dragging = drag?.from == key.t
                let p = CGPoint(x: x(key.t), y: mid)
                let glyph = KeyGlyph.diamond(at: p, size: isSelected ? 13 : 9)
                if isSelected { context.stroke(glyph, with: .color(Theme.halo), lineWidth: 3) }
                if key.show == false {
                    context.fill(glyph, with: .color(Theme.sidebar))
                    context.stroke(glyph, with: .color(color.opacity(dragging ? 0.4 : 1)), lineWidth: 1.5)
                } else {
                    context.fill(glyph, with: .color(color.opacity(dragging ? 0.4 : 1)))
                }
                if key.pose != nil {
                    context.fill(Path(ellipseIn: CGRect(x: p.x - 1.5, y: mid - (isSelected ? 11 : 9), width: 3, height: 3)), with: .color(color))
                }
            }
            if let drag {
                let ghost = KeyGlyph.diamond(at: CGPoint(x: x(drag.to), y: mid), size: 13)
                context.fill(ghost, with: .color(Theme.accent.opacity(0.5)))
                context.stroke(ghost, with: .color(Theme.accent), style: StrokeStyle(lineWidth: 1, dash: [2, 2]))
            }
            let px = TimelineModel.x(of: time, width: w, duration: duration)
            context.fill(Path(CGRect(x: px - 0.5, y: 0, width: 1, height: size.height)), with: .color(Theme.gold))
        }
    }
}

enum KeyGlyph {
    /// A diamond `size` points across, centered on `p`.
    static func diamond(at p: CGPoint, size: CGFloat) -> Path {
        let r = size / 2
        var path = Path()
        path.move(to: CGPoint(x: p.x, y: p.y - r))
        path.addLine(to: CGPoint(x: p.x + r, y: p.y))
        path.addLine(to: CGPoint(x: p.x, y: p.y + r))
        path.addLine(to: CGPoint(x: p.x - r, y: p.y))
        path.closeSubpath()
        return path
    }
}

extension KeyInput {
    /// A SwiftUI key press (the pane's `onKeyPress`) as the stage's key input.
    init?(press: KeyPress) {
        let m = press.modifiers
        let key: Key
        switch press.key {
        case .leftArrow: key = .left
        case .rightArrow: key = .right
        case .upArrow: key = .up
        case .downArrow: key = .down
        case .pageUp: key = .pageUp
        case .pageDown: key = .pageDown
        case .delete, .deleteForward: key = .delete
        case .return: key = .returnKey
        case .escape: key = .escape
        case .space: key = .space
        default:
            guard let c = press.characters.lowercased().first else { return nil }
            key = .char(c)
        }
        self.init(key: key, option: m.contains(.option), shift: m.contains(.shift), command: m.contains(.command), control: m.contains(.control))
    }
}
