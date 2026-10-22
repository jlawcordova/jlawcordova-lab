import SwiftUI

/// The Library, floating over the stage above the bar (docs/intents/2026-10-floating-library/spec.md, R8, D5): a
/// search field and a grid of small thumbnails with no names, no close button and no instructions.
/// It is an in-window overlay, not an `NSPopover`, so a drag out of it onto the stage keeps going.
/// It stays open while placing and dragging; L, the bar's button and Esc close it.
struct LibraryPopover: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @FocusState private var searchFocused: Bool
    @FocusState private var gridFocused: Bool

    static let width: CGFloat = 452
    static let gridHeight: CGFloat = 236
    static let columns = 8

    var body: some View {
        @Bindable var model = model
        let objects = filtered
        VStack(spacing: 8) {
            search
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(minimum: LibraryCell.width), spacing: 6), count: Self.columns), spacing: 6) {
                        ForEach(Array(objects.enumerated()), id: \.element.id) { i, object in
                            LibraryCell(
                                object: object,
                                image: model.thumbnails.images[object.name],
                                selected: model.editor.placeObject == object.name,
                                focused: gridFocused && model.libraryCursor == i,
                                action: { pick(object.name) },
                                open: { Task { await model.editObject(object.name) } }
                            )
                            .id(object.name)
                            .onAppear { model.thumbnails.appeared(object.name) }
                            .onDisappear { model.thumbnails.disappeared(object.name) }
                        }
                    }
                    .padding(.vertical, 2)
                    .background(QuietScrollers())
                }
                .scrollIndicators(.automatic)
                .frame(height: Self.gridHeight)
                .overlay {
                    if objects.isEmpty {
                        Text("No objects match").font(.callout).foregroundStyle(Theme.inkMuted)
                    }
                }
                .focusable()
                .focused($gridFocused)
                .focusEffectDisabled()
                .onKeyPress(phases: [.down, .repeat]) { press in key(press, objects: objects, proxy: proxy) }
                .accessibilityLabel("Library objects")
            }
        }
        .padding(12)
        .frame(width: Self.width)
        .background(Theme.sidebar, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .shadow(color: .black.opacity(0.28), radius: 14, y: 4)
        .transition(reduceMotion ? .identity : .opacity.combined(with: .offset(y: 6)))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Library")
        .onAppear { searchFocused = true }
        .onChange(of: model.libraryQuery) { _, _ in model.libraryCursor = 0 }
    }

    private var search: some View {
        @Bindable var model = model
        return HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(Theme.inkMuted).imageScale(.small)
            TextField("Search objects", text: $model.libraryQuery)
                .textFieldStyle(.plain)
                .focused($searchFocused)
                .foregroundStyle(Theme.ink)
                .onKeyPress(.escape) {
                    if model.libraryQuery.isEmpty { model.closeLibrary() } else { model.libraryQuery = "" }
                    return .handled
                }
                .onKeyPress(.downArrow) {
                    gridFocused = true
                    return .handled
                }
                .onSubmit { gridFocused = true }
            if !model.libraryQuery.isEmpty {
                Button { model.libraryQuery = "" } label: { Image(systemName: "xmark.circle.fill") }
                    .buttonStyle(.plain)
                    .foregroundStyle(Theme.inkMuted)
                    .accessibilityLabel("Clear search")
            }
        }
        .font(.callout)
        .padding(.horizontal, 8)
        .frame(height: 28)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(searchFocused ? Theme.accent : Theme.border, lineWidth: 1))
    }

    /// The objects the search keeps.
    private var filtered: [ObjectSummary] {
        let q = model.libraryQuery.trimmingCharacters(in: .whitespaces).lowercased()
        return q.isEmpty ? model.objects : model.objects.filter { $0.name.lowercased().contains(q) }
    }

    /// A click, Return: start placing that object, and hand the keys back to the stage.
    private func pick(_ name: String) {
        model.editor.choosePlaceObject(name)
        Task { _ = await model.thumbnails.art(name) }
        model.editor.focusStage()
    }

    /// The grid's keys: arrows between cells, Return picks, Esc closes, ⇧-Tab goes back to the search.
    private func key(_ press: KeyPress, objects: [ObjectSummary], proxy: ScrollViewProxy) -> KeyPress.Result {
        guard !objects.isEmpty else { return press.key == .escape ? escape() : .ignored }
        let cursor = min(max(model.libraryCursor, 0), objects.count - 1)
        var next = cursor
        switch press.key {
        case .leftArrow: next = max(cursor - 1, 0)
        case .rightArrow: next = min(cursor + 1, objects.count - 1)
        case .upArrow:
            if cursor < Self.columns { searchFocused = true; return .handled }
            next = cursor - Self.columns
        case .downArrow: next = min(cursor + Self.columns, objects.count - 1)
        case .return:
            pick(objects[cursor].name)
            return .handled
        case .escape: return escape()
        default: return .ignored
        }
        model.libraryCursor = next
        proxy.scrollTo(objects[next].name)
        model.editor.announce(objects[next].name)
        return .handled
    }

    private func escape() -> KeyPress.Result {
        model.closeLibrary()
        return .handled
    }
}
