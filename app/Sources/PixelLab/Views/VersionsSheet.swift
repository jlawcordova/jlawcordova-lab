import SwiftUI

/// File › Revert To › Browse All Versions… (docs/intents/2026-10-autosave/spec.md, R12, R13, D5): the
/// document's versions, newest first under day headings, beside the stage so the selected version's
/// ghost shows on it. The detail has every edit, the docs it touched, the diff a click away, and Restore
/// This Version, Restore <doc> Only and Undo These Changes. ↑ / ↓ move, Return restores (asking first),
/// ⌘⌫ undoes these changes, Esc closes. A panel over the stage rather than a sheet over the window, so
/// the ghost is never hidden (plan.md, Departures). Board 03 tokens, night and day.
struct VersionsSheet: View {
    @Environment(AppModel.self) private var model
    @FocusState private var listFocused: Bool

    var body: some View {
        let versions = model.versions
        VStack(spacing: 0) {
            header(versions)
            Rectangle().fill(Theme.border).frame(height: 1)
            if versions.versions.isEmpty {
                Text("No versions yet. Every change to the document, yours or an agent's, is kept here as it saves.")
                    .font(.callout).foregroundStyle(Theme.inkMuted)
                    .padding(16)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Spacer()
            } else {
                list(versions)
                    .frame(maxHeight: .infinity)
                Rectangle().fill(Theme.border).frame(height: 1)
                VersionDetail(versions: versions)
                    .frame(height: 300)
            }
        }
        .frame(width: 380)
        .background(Theme.sidebar)
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .shadow(color: .black.opacity(0.22), radius: 12, y: 3)
        .background(keys(versions))
        .onAppear { listFocused = true }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Versions")
    }

    private func header(_ versions: VersionsModel) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "clock.arrow.circlepath").foregroundStyle(Theme.accent).accessibilityHidden(true)
            Text("Versions").font(Theme.nameFont(16)).foregroundStyle(Theme.ink)
            Text("\(versions.total)").font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
            Spacer()
            if let w = versions.working {
                ProgressView().controlSize(.small)
                Text(w).font(.caption).foregroundStyle(Theme.inkMuted)
            }
            Button {
                versions.close()
            } label: {
                Image(systemName: "xmark").frame(width: 24, height: 24).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .foregroundStyle(Theme.inkMuted)
            .keyboardShortcut(.escape, modifiers: [])
            .help("Close (Esc)")
            .accessibilityLabel("Close Versions")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Theme.hero)
    }

    private func list(_ versions: VersionsModel) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
                    ForEach(Self.days(versions.versions), id: \.day) { group in
                        Section {
                            ForEach(group.versions) { v in
                                VersionRow(version: v, selected: versions.selectedNumber == v.number) { versions.select(v.number) }
                                    .id(v.number)
                            }
                        } header: {
                            Text(group.day.uppercased())
                                .font(.system(size: 10.5, weight: .semibold))
                                .tracking(0.6)
                                .foregroundStyle(Theme.inkMuted)
                                .padding(.horizontal, 14).padding(.vertical, 5)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(Theme.sidebar)
                                .accessibilityAddTraits(.isHeader)
                        }
                    }
                }
                .padding(.bottom, 6)
            }
            .focusable()
            .focused($listFocused)
            .focusEffectDisabled(false)
            .onChange(of: versions.selectedNumber) { _, n in
                if let n { proxy.scrollTo(n, anchor: .center) }
            }
        }
    }

    /// ↑ / ↓, Return and ⌘⌫ as the panel's keys (hidden buttons; the panel has no text fields).
    private func keys(_ versions: VersionsModel) -> some View {
        ZStack {
            Button("Newer Version") { versions.move(by: -1) }.keyboardShortcut(.upArrow, modifiers: [])
            Button("Older Version") { versions.move(by: 1) }.keyboardShortcut(.downArrow, modifiers: [])
            Button("Restore This Version") { Task { await versions.restore() } }.keyboardShortcut(.return, modifiers: [])
            Button("Undo These Changes") { Task { _ = await versions.undoChanges() } }.keyboardShortcut(.delete, modifiers: .command)
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    /// Versions under their day headings, in the list's order (newest first).
    static func days(_ list: [VersionSummary]) -> [(day: String, versions: [VersionSummary])] {
        var out: [(day: String, versions: [VersionSummary])] = []
        for v in list {
            let d = VersionsModel.day(v.endedAt)
            if out.last?.day == d { out[out.count - 1].versions.append(v) } else { out.append((d, [v])) }
        }
        return out
    }
}

/// One version: its time, the author's mark, the first edit and how many more, and the docs it touched.
struct VersionRow: View {
    var version: VersionSummary
    var selected: Bool
    var select: () -> Void

    var body: some View {
        Button(action: select) {
            HStack(alignment: .top, spacing: 10) {
                AuthorMark(author: version.author)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(version.author.display).font(.system(size: 12.5, weight: .semibold)).foregroundStyle(Theme.ink).lineLimit(1)
                        if let chat = version.author.chat {
                            Text(chat.title).font(.system(size: 11)).foregroundStyle(Theme.inkMuted).lineLimit(1)
                        }
                        Spacer(minLength: 6)
                        Text(version.open == true ? "Now" : VersionsModel.time(version.endedAt))
                            .font(Theme.monoFont(10.5)).foregroundStyle(version.open == true ? Theme.accent : Theme.inkMuted)
                    }
                    Text(firstEdit).font(.system(size: 12)).foregroundStyle(Theme.ink).lineLimit(2)
                    if !version.refs.isEmpty {
                        Text(version.refs.map(\.name).joined(separator: ", ")).font(Theme.monoFont(10.5)).foregroundStyle(Theme.inkMuted).lineLimit(1)
                    }
                }
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(selected ? Theme.selection : Color.clear, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .padding(.horizontal, 6)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(VersionsModel.spoken(version))
        .accessibilityAddTraits(selected ? [.isSelected, .isButton] : .isButton)
    }

    private var firstEdit: String {
        guard let first = version.edits.first else { return version.number == 0 ? "The document as first opened" : "No edits listed" }
        let e = AppModel.titled(first)
        return version.edits.count > 1 ? "\(e) +\(version.edits.count - 1) more" : e
    }
}

/// Who made a version, as a small mark: the owner's initial, the chat's asterisk avatar, a terminal glyph
/// for the CLI, a dashed outline for a change from outside Pixel Lab.
struct AuthorMark: View {
    var author: VersionAuthor

    var body: some View {
        Group {
            switch author.kind {
            case "agent": ClaudeAvatar(size: 26)
            case "cli": glyph("terminal")
            case "outside":
                Circle().strokeBorder(Theme.inkMuted, style: StrokeStyle(lineWidth: 1.2, dash: [3, 2]))
                    .overlay(Image(systemName: "arrow.down.doc").font(.system(size: 10)).foregroundStyle(Theme.inkMuted))
            case "restore": glyph("clock.arrow.circlepath")
            case "opened": glyph("doc")
            case "several": glyph("person.2")
            default:
                Circle().fill(Theme.accent)
                    .overlay(Text("Y").font(.system(size: 12, weight: .bold)).foregroundStyle(Theme.onAccent))
            }
        }
        .frame(width: 26, height: 26)
        .accessibilityHidden(true)
    }

    private func glyph(_ name: String) -> some View {
        Circle().fill(Theme.card)
            .overlay(Circle().strokeBorder(Theme.border, lineWidth: 1))
            .overlay(Image(systemName: name).font(.system(size: 11)).foregroundStyle(Theme.ink))
    }
}

/// The selected version: its edits, the docs it touched, Show diff, and the ways back.
struct VersionDetail: View {
    var versions: VersionsModel
    @Environment(AppModel.self) private var model
    private var showDiff: Bool { versions.showDiff }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let v = versions.selected {
                HStack {
                    SectionLabel(title: "Version \(v.number)")
                    Spacer()
                    Text(VersionsModel.time(v.endedAt)).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 3) {
                        ForEach(Array(v.edits.enumerated()), id: \.offset) { _, e in
                            Text(AppModel.titled(e)).font(.system(size: 12)).foregroundStyle(Theme.ink).frame(maxWidth: .infinity, alignment: .leading)
                        }
                        if !v.refs.isEmpty {
                            Text(v.refs.map(\.name).joined(separator: ", ")).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted).padding(.top, 2)
                        }
                        if let e = versions.lastResult {
                            Text(e).font(.caption).foregroundStyle(Theme.inkMuted).padding(.top, 4)
                        }
                    }
                }
                .frame(maxHeight: showDiff ? 60 : .infinity)
                if showDiff {
                    if let d = versions.detail, d.number == v.number {
                        DiffView(lines: DiffLine.parse(d.diffs.map(\.diff).joined(separator: "\n")))
                    } else if let e = versions.detailError {
                        Text(e).font(.caption).foregroundStyle(Theme.inkMuted)
                    } else {
                        ProgressView().controlSize(.small)
                    }
                }
                FlowLayout(spacing: 6) {
                    Button("Restore This Version") { Task { await versions.restore() } }
                        .buttonStyle(CapsuleButtonStyle(prominent: true))
                        .disabled(versions.working != nil)
                        .help("Put the whole document back as it was after this version (Return; asks first)")
                    if let ref = model.document?.ref, versions.touchesDocOnStage {
                        Button("Restore \(ref.name) Only") { Task { await versions.restore(only: ref) } }
                            .buttonStyle(CapsuleButtonStyle())
                            .disabled(versions.working != nil)
                    }
                    Button("Undo These Changes") { Task { _ = await versions.undoChanges() } }
                        .buttonStyle(CapsuleButtonStyle())
                        .disabled(versions.working != nil || v.number == 0)
                        .help("Take back what this version changed, keeping everything since (⌘⌫)")
                    Button(showDiff ? "Hide diff" : "Show diff") { versions.showDiff.toggle() }
                        .buttonStyle(AccentLinkStyle())
                }
            } else {
                Text("Choose a version").font(.callout).foregroundStyle(Theme.inkMuted)
                Spacer()
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.page)
    }
}
