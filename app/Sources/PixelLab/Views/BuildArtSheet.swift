import SwiftUI

/// Build art (⌘B): runs the site's `npm run art` through `art.build` and shows
/// its report, with a clear success or failure state.
struct BuildArtSheet: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let build = model.build
        VStack(alignment: .leading, spacing: 14) {
            header(build)
            if let r = build?.result, !r.unsaved.isEmpty {
                HStack(alignment: .firstTextBaseline, spacing: 7) {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.gold)
                    Text("Not in this build (unsaved): \(r.unsaved.compactMap { DocRef(id: $0)?.name ?? $0 }.joined(separator: ", ")). Save, then build again.")
                        .font(.callout)
                        .foregroundStyle(Theme.ink)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .accessibilityElement(children: .combine)
            }
            if let hint = build?.result?.hint {
                HStack(alignment: .firstTextBaseline, spacing: 7) {
                    Image(systemName: "lightbulb.fill").foregroundStyle(Theme.gold)
                    Text(hint)
                        .font(.callout)
                        .foregroundStyle(Theme.ink)
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .accessibilityElement(children: .combine)
            }
            report(build)
            HStack {
                if let site = model.sessionInfo?.site {
                    Text(site)
                        .font(Theme.monoFont(10))
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(1)
                        .truncationMode(.head)
                }
                Spacer()
                Button("Build Again") { Task { await model.buildArt() } }
                    .disabled(build?.running ?? true)
                Button("Done") { model.build = nil }
                    .keyboardShortcut(.defaultAction)
                    .disabled(build?.running ?? false)
            }
        }
        .padding(20)
        .frame(width: 600, height: 440)
        .background(Theme.page)
    }

    @ViewBuilder
    private func header(_ build: BuildState?) -> some View {
        HStack(spacing: 12) {
            if build?.running ?? true {
                ProgressView().controlSize(.regular)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Building art…").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text("npm run art in the site checkout").font(.callout).foregroundStyle(Theme.inkMuted)
                }
            } else if let r = build?.result, r.ok {
                Image(systemName: "checkmark.seal.fill").font(.system(size: 30)).foregroundStyle(Theme.accent)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Art built").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text("npm run art finished in \(String(format: "%.1f", r.ms / 1000)) s · \(r.report.count) line\(r.report.count == 1 ? "" : "s")")
                        .font(.callout).foregroundStyle(Theme.inkMuted)
                }
            } else {
                Image(systemName: "xmark.octagon.fill").font(.system(size: 30)).foregroundStyle(Theme.gold)
                VStack(alignment: .leading, spacing: 2) {
                    Text("The build failed").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text(build?.error ?? "npm run art exited with code \(build?.result?.code.map(String.init) ?? "?")")
                        .font(.callout).foregroundStyle(Theme.inkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private func report(_ build: BuildState?) -> some View {
        let lines = (build?.result?.report ?? []) + (build?.result?.errors ?? [])
        let errors = Set(build?.result?.errors ?? [])
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                if lines.isEmpty {
                    Text(build?.running ?? true ? "" : "No output.").foregroundStyle(Theme.inkMuted)
                }
                ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                    HStack(alignment: .top, spacing: 6) {
                        // Gold marks stderr lines (gold is a fill, never text on light).
                        Rectangle().fill(errors.contains(line) ? Theme.gold : .clear).frame(width: 2)
                        Text(line).foregroundStyle(Theme.ink)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .font(Theme.monoFont(11))
            .textSelection(.enabled)
            .padding(10)
        }
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .accessibilityLabel("Build report")
    }
}
