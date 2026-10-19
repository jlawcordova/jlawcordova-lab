import SwiftUI

/// Publish (⌘B): runs `art.build`, which compiles every scene with an output
/// and writes the SVGs into the document's publish folder, and shows its
/// report with a clear success or failure state.
struct PublishSheet: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let build = model.build
        VStack(alignment: .leading, spacing: 14) {
            header(build)
            if let r = build?.result, !r.unsaved.isEmpty {
                note(symbol: "exclamationmark.triangle.fill", "Not in this publish (unsaved): \(r.unsaved.compactMap { DocRef(id: $0)?.name ?? $0 }.joined(separator: ", ")). Save, then publish again.")
            }
            if let r = build?.result, !r.overBudget.isEmpty {
                note(symbol: "scalemass.fill", "Over budget: \(r.overBudget.map { $0.file ?? $0.scene }.joined(separator: ", ")). The files were written; make the art smaller, or raise the budget in the document's manifest.")
            }
            report(build)
            HStack {
                if let dir = build?.result?.publishDir ?? model.sessionInfo?.document?.publishDir {
                    Label(dir, systemImage: "folder")
                        .font(Theme.monoFont(10))
                        .foregroundStyle(Theme.inkMuted)
                        .lineLimit(1)
                        .truncationMode(.head)
                        .accessibilityLabel("Publish folder \(dir)")
                }
                Spacer()
                Button("Publish Elsewhere…") { Task { await model.publish(askFolder: true) } }
                    .disabled(build?.running ?? true)
                Button("Publish Again") { Task { await model.publish() } }
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

    private func note(symbol: String, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 7) {
            Image(systemName: symbol).foregroundStyle(Theme.gold)
            Text(text)
                .font(.callout)
                .foregroundStyle(Theme.ink)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private func header(_ build: BuildState?) -> some View {
        HStack(spacing: 12) {
            if build?.running ?? true {
                ProgressView().controlSize(.regular)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Publishing…").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text("Compiling every scene and writing the SVGs").font(.callout).foregroundStyle(Theme.inkMuted)
                }
            } else if let r = build?.result, r.ok {
                Image(systemName: "checkmark.seal.fill").font(.system(size: 30)).foregroundStyle(Theme.accent)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Published").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text("\(r.outputs.count) file\(r.outputs.count == 1 ? "" : "s") written in \(String(format: "%.1f", r.ms / 1000)) s, all lossless and within budget")
                        .font(.callout).foregroundStyle(Theme.inkMuted)
                }
            } else {
                Image(systemName: "xmark.octagon.fill").font(.system(size: 30)).foregroundStyle(Theme.gold)
                VStack(alignment: .leading, spacing: 2) {
                    Text(build?.result?.problems.isEmpty == false ? "Nothing published" : "Published with problems").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text(build?.error ?? failureText(build?.result))
                        .font(.callout).foregroundStyle(Theme.inkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .accessibilityElement(children: .combine)
    }

    private func failureText(_ r: ArtBuildResult?) -> String {
        guard let r else { return "The publish didn't run." }
        if !r.problems.isEmpty { return "\(r.problems.count) problem\(r.problems.count == 1 ? "" : "s") in the sources; nothing was written. Fix them (the Checks panel lists them), then publish again." }
        return "\(r.overBudget.count) output\(r.overBudget.count == 1 ? " is" : "s are") over budget."
    }

    @ViewBuilder
    private func report(_ build: BuildState?) -> some View {
        let lines = build?.result?.report ?? []
        let marked = build?.result?.marked ?? []
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                if lines.isEmpty {
                    Text(build?.running ?? true ? "" : "No output.").foregroundStyle(Theme.inkMuted)
                }
                ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                    HStack(alignment: .top, spacing: 6) {
                        // Gold marks problems and over-budget outputs (gold is a fill, never text on light).
                        Rectangle().fill(marked.contains(line) ? Theme.gold : .clear).frame(width: 2)
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
        .accessibilityLabel("Publish report")
    }
}
