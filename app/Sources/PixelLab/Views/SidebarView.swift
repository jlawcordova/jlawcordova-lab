import SwiftUI

/// The left sidebar: a search field, the SCENES list (with the objects a search
/// finds), and below it, in a scene, the open scene's LAYERS, or in an object
/// its PALETTE. The Library is a popover over the stage, not a tab here
/// (docs/intents/2026-10-floating-library/spec.md, R10).
struct SidebarView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        @Bindable var model = model
        GeometryReader { geo in
            VStack(spacing: 0) {
                SearchField(text: $model.searchText)
                    .padding(.horizontal, 12)
                    .padding(.top, 10)
                    .padding(.bottom, 8)
                // SCENES: a list of its own, at most about a third of the sidebar, so the
                // Layers tree and the Library below keep their own scroll (D11).
                sectionHeader("Scenes", count: nil)
                    .padding(.horizontal, 10)
                    .padding(.bottom, 3)
                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(spacing: 1) {
                            ForEach(filteredScenes) { scene in
                                SceneRow(scene: scene, selected: model.document?.ref == DocRef(.scene, scene.name)) {
                                    Task { await model.open(DocRef(.scene, scene.name)) }
                                }
                                .id(scene.name)
                            }
                            if filteredScenes.isEmpty && (query.isEmpty || filteredObjects.isEmpty) { emptyLine("No scenes match") }
                            // A search also finds objects: a row opens one (R11).
                            if !query.isEmpty, !filteredObjects.isEmpty {
                                sectionHeader("Objects", count: filteredObjects.count)
                                ForEach(filteredObjects) { object in
                                    ObjectRow(object: object, selected: model.document?.ref == DocRef(.object, object.name)) {
                                        Task { await model.editObject(object.name) }
                                    }
                                    .id("object:\(object.name)")
                                }
                            }
                        }
                        .padding(.horizontal, 10)
                    }
                    .scrollIndicators(.automatic)
                    .frame(height: scenesHeight(sidebar: geo.size.height))
                    // The open scene in view, once the list has laid out (on appear, and on each new scene).
                    .task(id: "\(model.document?.ref.id ?? "")|\(model.scenes.count)") {
                        guard let ref = model.document?.ref, ref.kind == .scene else { return }
                        try? await Task.sleep(for: .milliseconds(80))
                        proxy.scrollTo(ref.name, anchor: .center)
                    }
                }
                lowerHeader
                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 6) {
                            if showItemsTab, let doc = model.document {
                                LayersPanel(doc: doc, query: query)
                            } else if showPaletteTab {
                                PalettePanel()
                            }
                        }
                        .padding(.horizontal, 10)
                        .padding(.bottom, 16)
                    }
                    .scrollIndicators(.automatic)
                    .onChange(of: model.layers.scrollRequest) { _, t in
                        guard let t else { return }
                        proxy.scrollTo(t.description, anchor: .center)
                        model.layers.scrollRequest = nil
                    }
                    .onChange(of: model.document?.selection) { _, selection in
                        guard let t = selection?.last, showItemsTab else { return }
                        Task { @MainActor in scroll(proxy, to: t.description) }
                    }
                }
            }
        }
        .background(Theme.sidebar)
    }

    /// The scene list's height: its rows, but at most about a third of the
    /// sidebar (and at least three rows).
    private func scenesHeight(sidebar: CGFloat) -> CGFloat {
        let objectRows = query.isEmpty || filteredObjects.isEmpty ? 0 : filteredObjects.count + 1
        let rows = CGFloat(max(filteredScenes.count, 1) + objectRows) * 29
        return min(rows, max(sidebar * 0.32, 3 * 29))
    }

    private func scroll(_ proxy: ScrollViewProxy, to id: String) {
        if reduceMotion { proxy.scrollTo(id, anchor: .center) } else {
            withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(id, anchor: .center) }
        }
    }

    /// Above the lower scroll, so it stays put: LAYERS in a scene.
    @ViewBuilder
    private var lowerHeader: some View {
        if showItemsTab, model.document != nil {
            SectionLabel(title: "Layers")
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16)
                .padding(.top, 10)
                .padding(.bottom, 4)
        }
    }

    private var showItemsTab: Bool { model.document?.ref.kind == .scene && model.mode == .scene }
    private var showPaletteTab: Bool { model.document?.ref.kind == .object && model.mode == .object }

    private var query: String { model.searchText.trimmingCharacters(in: .whitespaces).lowercased() }
    private var filteredScenes: [SceneSummary] {
        query.isEmpty ? model.scenes : model.scenes.filter { $0.name.lowercased().contains(query) }
    }
    private var filteredObjects: [ObjectSummary] {
        query.isEmpty ? model.objects : model.objects.filter { $0.name.lowercased().contains(query) }
    }

    /// A section heading; SCENES has no count (D11).
    private func sectionHeader(_ title: String, count: Int?) -> some View {
        HStack(alignment: .firstTextBaseline) {
            SectionLabel(title: title)
            Spacer()
            if let count { Text("\(count)").font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted.opacity(0.8)) }
        }
        .padding(.horizontal, 6)
        .padding(.top, 6)
        .padding(.bottom, 2)
    }

    private func emptyLine(_ text: String) -> some View {
        Text(text).font(.callout).foregroundStyle(Theme.inkMuted).padding(.horizontal, 6).padding(.vertical, 4)
    }
}

/// The sidebar's search field, on the card ground.
struct SearchField: View {
    @Binding var text: String
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(Theme.inkMuted).imageScale(.small)
            TextField("Search scenes and objects", text: $text)
                .textFieldStyle(.plain)
                .focused($focused)
                .foregroundStyle(Theme.ink)
            if !text.isEmpty {
                Button { text = "" } label: { Image(systemName: "xmark.circle.fill") }
                    .buttonStyle(.plain)
                    .foregroundStyle(Theme.inkMuted)
                    .accessibilityLabel("Clear search")
            }
        }
        .font(.callout)
        .padding(.horizontal, 8)
        .frame(height: 28)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .strokeBorder(focused ? Theme.accent : Theme.border, lineWidth: 1)
        )
    }
}

struct SceneRow: View {
    var scene: SceneSummary
    var selected: Bool
    var action: () -> Void
    @Environment(AppModel.self) private var model
    private var hovering: Bool { model.hovered == "scene:\(scene.name)" }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: scene.motion ? "film.stack" : "square.stack.3d.up")
                    .font(.system(size: 13))
                    .foregroundStyle(selected ? Theme.accent : Theme.inkMuted)
                    .frame(width: 18)
                Text(scene.name)
                    .font(Theme.nameFont(13, weight: selected ? .semibold : .medium))
                    .foregroundStyle(Theme.ink)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: 4)
            }
            .padding(.horizontal, 8)
            .frame(height: 28)
            .background(
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(selected ? Theme.selection : (hovering ? Theme.border.opacity(0.5) : .clear))
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { model.setHovered("scene:\(scene.name)", $0) }
        .accessibilityLabel("Scene \(scene.name), \(scene.items) items")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// An object a search found, in the scenes list: a row that opens it.
struct ObjectRow: View {
    var object: ObjectSummary
    var selected: Bool
    var action: () -> Void
    @Environment(AppModel.self) private var model
    private var hovering: Bool { model.hovered == "object:\(object.name)" }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: "cube")
                    .font(.system(size: 13))
                    .foregroundStyle(selected ? Theme.accent : Theme.inkMuted)
                    .frame(width: 18)
                Text(object.name)
                    .font(Theme.nameFont(13, weight: selected ? .semibold : .medium))
                    .foregroundStyle(Theme.ink)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: 4)
            }
            .padding(.horizontal, 8)
            .frame(height: 28)
            .background(
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(selected ? Theme.selection : (hovering ? Theme.border.opacity(0.5) : .clear))
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { model.setHovered("object:\(object.name)", $0) }
        .help("Edit \(object.name)")
        .accessibilityLabel("Object \(object.name), edit")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// A Library thumbnail, small and without a name: its name is its help tag and its VoiceOver label
/// (docs/intents/2026-10-floating-library/spec.md, R8, D5). Click to place, drag onto the stage to place one
/// where it lands, double-click or the menu to edit the object.
struct LibraryCell: View {
    var object: ObjectSummary
    var image: PixelImage?
    /// This is the object being placed.
    var selected: Bool
    /// The keyboard is on this cell.
    var focused = false
    var action: () -> Void
    var open: () -> Void = {}
    @Environment(AppModel.self) private var model
    private var hovering: Bool { model.hovered == "object:\(object.name)" }
    @Environment(\.displayScale) private var displayScale

    static let width: CGFloat = 44
    static let height: CGFloat = 42

    var body: some View {
        Button(action: action) {
            ZStack {
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(Theme.card)
                if let image {
                    let f = Self.factor(for: image.size, in: Self.height - 8)
                    image.view(pointsPerPixel: f, displayScale: displayScale)
                } else {
                    ProgressView().controlSize(.mini).opacity(0.5)
                }
            }
            .frame(minWidth: Self.width, maxWidth: .infinity)
            .frame(height: Self.height)
            .overlay(
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .strokeBorder(selected || focused ? Theme.accent : (hovering ? Theme.border.opacity(2) : Theme.border), lineWidth: selected || focused ? 1.5 : 1)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .simultaneousGesture(TapGesture(count: 2).onEnded { open() })
        .onDrag { NSItemProvider(object: (LibraryDrag.prefix + object.name) as NSString) }
        .contextMenu {
            Button("Place \(object.name)") { action() }
            Button("Edit Object") { open() }
        }
        .onHover { model.setHovered("object:\(object.name)", $0) }
        .help(object.name)
        .accessibilityLabel("Place \(object.name)")
        .accessibilityHint("Double-click to edit the object")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    /// Whole points per art pixel (or ½, one device pixel on Retina) so the
    /// thumbnail stays crisp; the largest that fits.
    static func factor(for size: CGSize, in side: CGFloat) -> CGFloat {
        for f: CGFloat in [4, 3, 2, 1, 0.5] where size.width * f <= side && size.height * f <= side { return f }
        return min(side / max(size.width, 1), side / max(size.height, 1))
    }
}
