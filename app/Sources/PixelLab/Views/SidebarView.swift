import SwiftUI

/// The left sidebar: a search field, the SCENES list, and below it the
/// LIBRARY grid of object thumbnails or (scene mode) the open scene's ITEMS
/// tree. In scene mode a library click picks the object to place (drag it onto
/// the stage, or double-click to open it in object mode).
struct SidebarView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            SearchField(text: $model.searchText)
                .padding(.horizontal, 12)
                .padding(.top, 10)
                .padding(.bottom, 8)
            ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    sectionHeader("Scenes", count: filteredScenes.count)
                    VStack(spacing: 1) {
                        ForEach(filteredScenes) { scene in
                            SceneRow(scene: scene, selected: model.document?.ref == DocRef(.scene, scene.name)) {
                                Task { await model.open(DocRef(.scene, scene.name)) }
                            }
                        }
                    }
                    if filteredScenes.isEmpty { emptyLine("No scenes match") }

                    if showItemsTab {
                        Picker("Show", selection: Binding(get: { model.editor.sidebarTab }, set: { model.editor.sidebarTab = $0 })) {
                            Text("Library").tag(SceneEditor.SidebarTab.library)
                            Text("Items").tag(SceneEditor.SidebarTab.items)
                        }
                        .pickerStyle(.segmented)
                        .labelsHidden()
                        .padding(.top, 12)
                        .padding(.horizontal, 4)
                    } else if showPaletteTab {
                        Picker("Show", selection: Binding(get: { model.objectEditor.sidebarTab }, set: { model.objectEditor.sidebarTab = $0 })) {
                            Text("Palette").tag(ObjectEditor.SidebarTab.palette)
                            Text("Library").tag(ObjectEditor.SidebarTab.library)
                        }
                        .pickerStyle(.segmented)
                        .labelsHidden()
                        .padding(.top, 12)
                        .padding(.horizontal, 4)
                    }
                    if showItemsTab, model.editor.sidebarTab == .items, let doc = model.document {
                        ItemsTree(doc: doc, query: query)
                    } else if showPaletteTab, model.objectEditor.sidebarTab == .palette {
                        PalettePanel()
                    } else {
                        library
                    }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 16)
            }
            .scrollIndicators(.automatic)
            .onChange(of: model.editor.placeObject) { _, name in
                guard let name, model.editor.sidebarTab == .library || !showItemsTab else { return }
                withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(name, anchor: .center) }
            }
            .onChange(of: model.editor.sidebarTab) { _, tab in
                guard tab == .items, let path = model.document?.selectedPath else { return }
                Task { @MainActor in proxy.scrollTo(path, anchor: .center) }
            }
            .onChange(of: model.document?.selectedPath) { _, path in
                guard let path, model.editor.sidebarTab == .items else { return }
                withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(path, anchor: .center) }
            }
            }
        }
        .background(Theme.sidebar)
    }

    private var showItemsTab: Bool { model.document?.ref.kind == .scene && model.mode == .scene }
    private var showPaletteTab: Bool { model.document?.ref.kind == .object && model.mode == .object }

    @ViewBuilder
    private var library: some View {
        sectionHeader("Library", count: filteredObjects.count)
            .padding(.top, showItemsTab || showPaletteTab ? 4 : 14)
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 6), count: 3), spacing: 6) {
            ForEach(filteredObjects) { object in
                LibraryCell(
                    object: object,
                    image: model.thumbnails.images[object.name],
                    selected: showItemsTab
                        ? model.editor.placeObject == object.name
                        : model.document?.ref == DocRef(.object, object.name),
                    placing: showItemsTab
                ) {
                    if showItemsTab {
                        model.editor.choosePlaceObject(object.name)
                    } else {
                        Task { await model.open(DocRef(.object, object.name)) }
                    }
                } open: {
                    Task { await model.open(DocRef(.object, object.name)) }
                }
                .onAppear { model.thumbnails.appeared(object.name) }
                .onDisappear { model.thumbnails.disappeared(object.name) }
            }
        }
        if filteredObjects.isEmpty { emptyLine("No objects match") }
    }

    private var query: String { model.searchText.trimmingCharacters(in: .whitespaces).lowercased() }
    private var filteredScenes: [SceneSummary] {
        query.isEmpty ? model.scenes : model.scenes.filter { $0.name.lowercased().contains(query) }
    }
    private var filteredObjects: [ObjectSummary] {
        query.isEmpty ? model.objects : model.objects.filter { $0.name.lowercased().contains(query) }
    }

    private func sectionHeader(_ title: String, count: Int) -> some View {
        HStack(alignment: .firstTextBaseline) {
            SectionLabel(title: title)
            Spacer()
            Text("\(count)").font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted.opacity(0.8))
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
                Text("\(scene.items)")
                    .font(Theme.monoFont(10))
                    .foregroundStyle(Theme.inkMuted.opacity(0.8))
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

struct LibraryCell: View {
    var object: ObjectSummary
    var image: PixelImage?
    var selected: Bool
    /// Scene mode: a click picks the object to place, and it drags onto the stage.
    var placing = false
    var action: () -> Void
    var open: () -> Void = {}
    @Environment(AppModel.self) private var model
    private var hovering: Bool { model.hovered == "object:\(object.name)" }
    @Environment(\.displayScale) private var displayScale

    static let side: CGFloat = 66

    var body: some View {
        Button(action: action) {
            VStack(spacing: 3) {
                ZStack {
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .fill(Theme.card)
                    if let image {
                        let f = Self.factor(for: image.size, in: Self.side - 12)
                        image.view(pointsPerPixel: f, displayScale: displayScale)
                    } else {
                        ProgressView().controlSize(.mini).opacity(0.5)
                    }
                }
                .frame(height: Self.side)
                .overlay(
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .strokeBorder(selected ? Theme.accent : (hovering ? Theme.border.opacity(2) : Theme.border), lineWidth: selected ? 1.5 : 1)
                )
                Text(object.name)
                    .font(.system(size: 9.5))
                    .foregroundStyle(selected ? Theme.ink : Theme.inkMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .simultaneousGesture(TapGesture(count: 2).onEnded { if placing { open() } })
        .onDrag {
            if placing { model.editor.placeObject = object.name }
            return NSItemProvider(object: (LibraryDrag.prefix + object.name) as NSString)
        }
        .contextMenu {
            if placing {
                Button("Place \(object.name)") { action() }
            }
            Button("Open in Object Mode") { open() }
        }
        .onHover { model.setHovered("object:\(object.name)", $0) }
        .help(placing ? "\(object.name): click to place it, drag it onto the stage, double-click to open it" : (object.description ?? object.name))
        .accessibilityLabel(placing ? "Place \(object.name)" : "Object \(object.name)")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    /// Whole points per art pixel (or ½, one device pixel on Retina) so the
    /// thumbnail stays crisp; the largest that fits.
    static func factor(for size: CGSize, in side: CGFloat) -> CGFloat {
        for f: CGFloat in [4, 3, 2, 1, 0.5] where size.width * f <= side && size.height * f <= side { return f }
        return min(side / max(size.width, 1), side / max(size.height, 1))
    }
}
