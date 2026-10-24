import SwiftUI

/// A color's swatch, with a hairline border so a dark color shows on the dark sheet.
private struct ColorChip: View {
    var hex: String
    var side: CGFloat = 20
    var body: some View {
        RoundedRectangle(cornerRadius: 4, style: .continuous)
            .fill(Theme.hexColor(hex))
            .frame(width: side, height: side)
            .overlay(RoundedRectangle(cornerRadius: 4, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
            .accessibilityHidden(true)
    }
}

/// A heading and the plain words under it.
private struct SheetSection<Content: View>: View {
    var title: String
    var text: String?
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            SectionLabel(title: title)
            if let text {
                Text(text)
                    .font(.callout)
                    .foregroundStyle(Theme.ink)
                    .fixedSize(horizontal: false, vertical: true)
            }
            content
        }
        .accessibilityElement(children: .contain)
    }
}

/// The reason an import went wrong, in the sheet.
private struct SheetError: View {
    var text: String
    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 7) {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.gold)
            Text(text).font(.callout).foregroundStyle(Theme.ink).fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
        }
        .accessibilityElement(children: .combine)
    }
}

/// File › Import Library… (docs/intents/2026-10-object-libraries/spec.md, D5): the renames, the colors that differ
/// and the colors the document doesn't have, each with the choice it takes.
struct LibraryImportSheet: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        if let draft = model.imports.library {
            let plan = draft.plan
            VStack(alignment: .leading, spacing: 14) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Import \(draft.name)").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text(subtitle(draft))
                        .font(.callout).foregroundStyle(Theme.inkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(.isHeader)
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        if !plan.renames.isEmpty { renames(plan) }
                        if !plan.differing.isEmpty { differing(plan) }
                        if plan.character.swapped { character(plan) }
                        if !plan.missing.isEmpty { missing(draft) }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.trailing, 4)
                }
                .scrollIndicators(.automatic)
                .frame(maxHeight: 330)
                if let error = draft.error { SheetError(text: error) }
                HStack {
                    Spacer()
                    Button("Cancel") { model.imports.cancelLibrary() }
                        .keyboardShortcut(.cancelAction)
                    Button(draft.buttonTitle) { Task { await model.imports.applyLibrary() } }
                        .buttonStyle(AccentButtonStyle())
                        .keyboardShortcut(.defaultAction)
                        .accessibilityLabel(draft.buttonTitle)
                }
            }
            .padding(20)
            .frame(width: 560)
            .background(Theme.page)
        }
    }

    private func subtitle(_ draft: LibraryDraft) -> String {
        let plan = draft.plan
        let n = plan.count
        var text = "\(n) object\(n == 1 ? "" : "s") will be copied into \(model.documentName ?? "this document"), and nothing links it to the library afterwards."
        if !plan.same.isEmpty { text += " \(plan.same.count) already here as they are and won't be copied." }
        return text
    }

    private func renames(_ plan: LibraryPlan) -> some View {
        SheetSection(title: "Renamed", text: "These names are taken here, so the imported objects get new ones. Your own objects keep theirs.") {
            ForEach(plan.renames, id: \.from) { r in
                HStack(spacing: 8) {
                    Text(r.from).font(Theme.monoFont(12)).foregroundStyle(Theme.inkMuted)
                    Image(systemName: "arrow.right").font(.system(size: 10)).foregroundStyle(Theme.inkMuted).accessibilityHidden(true)
                    Text(r.to).font(Theme.monoFont(12)).foregroundStyle(Theme.ink)
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel("\(r.from) becomes \(r.to)")
            }
        }
    }

    private func differing(_ plan: LibraryPlan) -> some View {
        SheetSection(title: "Colors that differ", text: "These objects will look different from the library, because this document's colors aren't the library's.") {
            ForEach(plan.differing) { d in
                HStack(spacing: 8) {
                    ColorChip(hex: d.library)
                    Image(systemName: "arrow.right").font(.system(size: 10)).foregroundStyle(Theme.inkMuted).accessibilityHidden(true)
                    ColorChip(hex: d.document)
                    Text(d.name).font(Theme.monoFont(12)).foregroundStyle(Theme.ink)
                    Text("\(LibraryDraft.lookWord(document: d.document, library: d.library)) here")
                        .font(.callout).foregroundStyle(Theme.inkMuted)
                    Spacer(minLength: 0)
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel("\(d.name) is \(LibraryDraft.lookWord(document: d.document, library: d.library)) here: the library has \(d.library), this document \(d.document)")
            }
        }
    }

    private func character(_ plan: LibraryPlan) -> some View {
        SheetSection(title: "Character", text: "Figures will use this document's character, so they may look different.") {
            if !plan.character.figures.isEmpty {
                Text(plan.character.figures.joined(separator: ", ")).font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
            }
        }
    }

    private func missing(_ draft: LibraryDraft) -> some View {
        SheetSection(
            title: "Colors this document doesn't have",
            text: "Each is converted to one of this document's colors. Converting changes how the imported objects look."
        ) {
            ForEach(draft.plan.missing) { m in
                HStack(spacing: 8) {
                    ColorChip(hex: m.hex)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(m.name).font(Theme.monoFont(12)).foregroundStyle(Theme.ink)
                        Text("used by \(m.usedBy.joined(separator: ", "))").font(.caption).foregroundStyle(Theme.inkMuted).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    Image(systemName: "arrow.right").font(.system(size: 10)).foregroundStyle(Theme.inkMuted).accessibilityHidden(true)
                    conversionMenu(m, draft: draft)
                }
                .accessibilityElement(children: .contain)
            }
        }
    }

    private func conversionMenu(_ m: LibraryPlan.Missing, draft: LibraryDraft) -> some View {
        let options = LibraryDraft.options(for: m, palette: model.palette)
        let chosen = draft.conversions[m.name] ?? m.nearest?.name ?? ""
        let hex = model.paletteByName[chosen]?.hex
        return Menu {
            ForEach(options) { c in
                Button {
                    model.imports.library?.conversions[m.name] = c.name
                } label: {
                    Text(c.name == m.nearest?.name ? "\(c.name)  \(c.hex)  (nearest)" : "\(c.name)  \(c.hex)")
                }
            }
        } label: {
            HStack(spacing: 6) {
                if let hex { ColorChip(hex: hex, side: 16) }
                Text(chosen).font(Theme.monoFont(12))
            }
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .accessibilityLabel("Convert \(m.name) to")
        .accessibilityValue(chosen)
    }
}

/// File › Import Palette… (D6): the new colors, the colors that differ with Keep Mine or Use Theirs, and, when the
/// tiers would run over, which new colors to add.
struct PaletteImportSheet: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        if let draft = model.imports.palette {
            let plan = draft.plan
            VStack(alignment: .leading, spacing: 14) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Import \(draft.name)").font(Theme.nameFont(17)).foregroundStyle(Theme.ink)
                    Text("New colors are added at the end of their tier. No color of this document is removed.")
                        .font(.callout).foregroundStyle(Theme.inkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(.isHeader)
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        if !plan.new.isEmpty { newColors(draft) }
                        if !plan.differing.isEmpty { differing(draft) }
                        if !plan.otherTier.isEmpty { otherTier(plan) }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.trailing, 4)
                }
                .scrollIndicators(.automatic)
                .frame(maxHeight: 330)
                if let error = draft.error { SheetError(text: error) }
                HStack {
                    Spacer()
                    Button("Cancel") { model.imports.cancelPalette() }
                        .keyboardShortcut(.cancelAction)
                    Button("Import") { Task { await model.imports.applyPalette() } }
                        .buttonStyle(AccentButtonStyle())
                        .keyboardShortcut(.defaultAction)
                        .disabled(!draft.canImport)
                        .accessibilityLabel("Import")
                }
            }
            .padding(20)
            .frame(width: 560)
            .background(Theme.page)
        }
    }

    private func newColors(_ draft: PaletteDraft) -> some View {
        let plan = draft.plan
        let over = plan.over > 0
        return SheetSection(
            title: "New colors",
            text: over
                ? "These would put the palette \(draft.overTotal) color\(draft.overTotal == 1 ? "" : "s") over its cap. Uncheck some, until they fit."
                : "\(plan.new.count) color\(plan.new.count == 1 ? "" : "s") this document doesn't have."
        ) {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 8, alignment: .leading)], alignment: .leading, spacing: 8) {
                ForEach(plan.new) { c in
                    let on = draft.selected.contains(c.name)
                    HStack(spacing: 7) {
                        if over {
                            Toggle(isOn: Binding(
                                get: { model.imports.palette?.selected.contains(c.name) ?? false },
                                set: { if $0 { model.imports.palette?.selected.insert(c.name) } else { model.imports.palette?.selected.remove(c.name) } }
                            )) { EmptyView() }
                            .toggleStyle(.checkbox)
                            .labelsHidden()
                            .accessibilityLabel("Add \(c.name)")
                        }
                        ColorChip(hex: c.hex)
                        VStack(alignment: .leading, spacing: 0) {
                            Text(c.name).font(Theme.monoFont(12)).foregroundStyle(on ? Theme.ink : Theme.inkMuted)
                            Text("\(c.tier) · \(c.hex)").font(.caption2).foregroundStyle(Theme.inkMuted)
                        }
                    }
                    .accessibilityElement(children: .contain)
                    .accessibilityLabel("\(c.name), \(c.tier), \(c.hex)")
                }
            }
            if over {
                ForEach(["world", "outfit"], id: \.self) { tier in
                    let t = tier == "world" ? plan.tiers.world : plan.tiers.outfit
                    if draft.adding(tier) > 0 || t.over > 0 {
                        Text("\(tier.capitalized): \(t.have + draft.adding(tier)) of \(t.cap)\(draft.over(tier) > 0 ? " (\(draft.over(tier)) over)" : "")")
                            .font(Theme.monoFont(11)).foregroundStyle(Theme.inkMuted)
                    }
                }
            }
        }
    }

    private func differing(_ draft: PaletteDraft) -> some View {
        SheetSection(title: "Colors that differ", text: "This document has these names with other values. Keep Mine leaves them alone.") {
            ForEach(draft.plan.differing) { d in
                let theirs = draft.useTheirs.contains(d.name)
                HStack(spacing: 8) {
                    ColorChip(hex: d.mine)
                    Image(systemName: "arrow.right").font(.system(size: 10)).foregroundStyle(Theme.inkMuted).accessibilityHidden(true)
                    ColorChip(hex: d.theirs)
                    VStack(alignment: .leading, spacing: 0) {
                        Text(d.name).font(Theme.monoFont(12)).foregroundStyle(Theme.ink)
                        Text("mine \(d.mine) · theirs \(d.theirs)").font(.caption2).foregroundStyle(Theme.inkMuted)
                    }
                    Spacer(minLength: 8)
                    Picker("", selection: Binding(
                        get: { model.imports.palette?.useTheirs.contains(d.name) ?? false },
                        set: { if $0 { model.imports.palette?.useTheirs.insert(d.name) } else { model.imports.palette?.useTheirs.remove(d.name) } }
                    )) {
                        Text("Keep Mine").tag(false)
                        Text("Use Theirs").tag(true)
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .frame(width: 180)
                    .accessibilityLabel("\(d.name): keep mine or use theirs")
                }
                if theirs {
                    Text("Using theirs changes how every object that uses \(d.name) looks.")
                        .font(.caption).foregroundStyle(Theme.ink).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    private func otherTier(_ plan: PalettePlan) -> some View {
        SheetSection(title: "In the other tier", text: "These names are in another tier in the file. They stay as this document has them.") {
            ForEach(plan.otherTier) { c in
                HStack(spacing: 8) {
                    ColorChip(hex: c.mine)
                    Text(c.name).font(Theme.monoFont(12)).foregroundStyle(Theme.ink)
                    Text("\(c.tier) here, \(c.theirsTier) in the file").font(.callout).foregroundStyle(Theme.inkMuted)
                    Spacer(minLength: 0)
                }
                .accessibilityElement(children: .combine)
            }
        }
    }
}
