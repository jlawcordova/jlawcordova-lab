import SwiftUI

/// The Library, floating over the stage above the bar (docs/intents/2026-10-floating-library/spec.md, R8, D5): a
/// search field and a grid of small thumbnails with no names, no close button and no instructions.
/// It is an in-window overlay, not an `NSPopover`, so a drag out of it onto the stage keeps going.
/// It stays open while placing and dragging; L, the bar's button and Esc close it.
struct LibraryPopover: View {
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private enum Field: Hashable { case search, tabs, grid }
    @FocusState private var focus: Field?

    static let width: CGFloat = 452
    /// The grid's height with no tab row. With the row (24 pt and the 8 pt gap above it), the grid gives that room up,
    /// so the popover keeps its height (docs/intents/2026-10-object-libraries/spec.md, D7).
    static let gridHeight: CGFloat = 236
    static let tabRowHeight: CGFloat = 24
    static let columns = 8

    var body: some View {
        @Bindable var model = model
        let sections = model.librarySections
        let objects = LibraryCategories.flattened(sections)
        let tabs = model.libraryTabNames
        VStack(spacing: 8) {
            search
            if !tabs.isEmpty { tabRow(tabs) }
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(minimum: LibraryCell.width), spacing: 6), count: Self.columns), spacing: 6, pinnedViews: [.sectionHeaders]) {
                        ForEach(Array(sections.enumerated()), id: \.offset) { s, section in
                            let before = sections[..<s].reduce(0) { $0 + $1.objects.count }
                            Section {
                                ForEach(Array(section.objects.enumerated()), id: \.element.id) { i, object in
                                    LibraryCell(
                                        object: object,
                                        image: model.thumbnails.images[object.name],
                                        selected: model.editor.placeObject == object.name,
                                        focused: focus == .grid && model.libraryCursor == before + i,
                                        action: { pick(object.name) },
                                        open: { Task { await model.editObject(object.name) } }
                                    )
                                    .id(object.name)
                                    .onAppear { model.thumbnails.appeared(object.name) }
                                    .onDisappear { model.thumbnails.disappeared(object.name) }
                                }
                            } header: {
                                if let heading = section.heading { headingView(heading) }
                            }
                        }
                    }
                    .padding(.vertical, 2)
                    .background(QuietScrollers())
                }
                .scrollIndicators(.automatic)
                .frame(height: tabs.isEmpty ? Self.gridHeight : Self.gridHeight - Self.tabRowHeight - 8)
                .overlay {
                    if objects.isEmpty {
                        Text("No objects match").font(.callout).foregroundStyle(Theme.inkMuted)
                    }
                }
                .focusable()
                .focused($focus, equals: .grid)
                .focusEffectDisabled()
                .onKeyPress(phases: [.down, .repeat]) { press in key(press, sections: sections, proxy: proxy) }
                .accessibilityLabel("Library objects")
            }
        }
        .padding(12)
        .frame(width: Self.width)
        // The shadow is the panel's only: on the whole view it would shadow every cell and pinned heading too.
        .background {
            RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.sidebar).shadow(color: .black.opacity(0.28), radius: 14, y: 4)
        }
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .transition(reduceMotion ? .identity : .opacity.combined(with: .offset(y: 6)))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Library")
        // ⌃Tab and ⌃⇧Tab choose the next and previous tab from anywhere in the popover (R8).
        .onKeyPress(phases: [.down, .repeat]) { press in
            guard press.key == .tab, press.modifiers.contains(.control), !tabs.isEmpty else { return .ignored }
            chooseTab(LibraryCategories.step(model.libraryTab, by: press.modifiers.contains(.shift) ? -1 : 1, in: tabs))
            return .handled
        }
        .onAppear { focus = .search }
        .onChange(of: model.libraryQuery) { _, _ in model.libraryCursor = 0 }
    }

    // MARK: Tabs (R5, R8)

    /// The tab row: text tabs, the chosen one in ink with an accent underline, scrolling sideways when it doesn't fit.
    /// No animation, so Reduce Motion has nothing to turn off.
    private func tabRow(_ tabs: [String]) -> some View {
        let current = model.libraryTab
        return ScrollViewReader { proxy in
            ScrollView(.horizontal) {
                HStack(spacing: 6) {
                    ForEach(tabs, id: \.self) { tab in tabButton(tab, chosen: tab.lowercased() == current.lowercased()) }
                }
                .padding(.horizontal, 2)
                .background(QuietScrollers())
                .accessibilityElement(children: .contain)
                .accessibilityLabel("Categories")
                .accessibilityAddTraits(.isTabBar)
            }
            .scrollIndicators(.automatic)
            .frame(height: Self.tabRowHeight)
            .focusable()
            .focused($focus, equals: .tabs)
            .focusEffectDisabled()
            .onKeyPress(phases: [.down, .repeat]) { press in tabKey(press, tabs: tabs) }
            .onChange(of: current) { _, tab in proxy.scrollTo(tab) }
            .onAppear { proxy.scrollTo(current) }
        }
    }

    private func tabButton(_ tab: String, chosen: Bool) -> some View {
        Button { chooseTab(tab) } label: {
            VStack(spacing: 0) {
                Text(tab)
                    .font(Theme.nameFont(12, weight: chosen ? .semibold : .medium))
                    .foregroundStyle(chosen ? Theme.ink : Theme.inkMuted)
                    .lineLimit(1)
                    .fixedSize()
                    .frame(height: 20)
                Rectangle().fill(chosen ? Theme.accent : .clear).frame(height: 2)
            }
            .padding(.horizontal, 6)
            .overlay(RoundedRectangle(cornerRadius: 4, style: .continuous).strokeBorder(chosen && focus == .tabs ? Theme.accent : .clear, lineWidth: 1.5))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .id(tab)
        .help(tab == LibraryCategories.all ? "Every object" : tab)
        .accessibilityLabel(tab)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }

    private func chooseTab(_ tab: String) {
        guard tab.lowercased() != model.libraryTab.lowercased() else { return }
        model.libraryTab = tab
        model.libraryCursor = 0
        model.editor.announce("\(tab) tab")
    }

    /// ← and → between tabs; Tab to the grid, ⇧Tab back to the search; ↓ to the grid.
    private func tabKey(_ press: KeyPress, tabs: [String]) -> KeyPress.Result {
        if press.modifiers.contains(.control) { return .ignored }
        switch press.key {
        case .leftArrow: chooseTab(LibraryCategories.step(model.libraryTab, by: -1, in: tabs))
        case .rightArrow: chooseTab(LibraryCategories.step(model.libraryTab, by: 1, in: tabs))
        case .downArrow: focus = .grid
        case .upArrow: focus = .search
        case .tab: focus = press.modifiers.contains(.shift) ? .search : .grid
        case .escape: model.closeLibrary()
        default: return .ignored
        }
        return .handled
    }

    /// A subcategory's heading, full width, pinned at the top of the grid while its objects scroll under it.
    private func headingView(_ heading: String) -> some View {
        HStack {
            SectionLabel(title: heading)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 2)
        .padding(.vertical, 3)
        .background(Theme.sidebar)
    }

    private var search: some View {
        @Bindable var model = model
        return HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(Theme.inkMuted).imageScale(.small)
            TextField("Search objects", text: $model.libraryQuery)
                .textFieldStyle(.plain)
                .focused($focus, equals: .search)
                .foregroundStyle(Theme.ink)
                .onKeyPress(.escape) {
                    if model.libraryQuery.isEmpty { model.closeLibrary() } else { model.libraryQuery = "" }
                    return .handled
                }
                .onKeyPress(.downArrow) {
                    focus = .grid
                    return .handled
                }
                // Tab goes to the tab row, then the grid (R8).
                .onKeyPress(phases: [.down, .repeat]) { press in
                    guard press.key == .tab, !press.modifiers.contains(.control), !press.modifiers.contains(.shift) else { return .ignored }
                    focus = model.libraryTabNames.isEmpty ? .grid : .tabs
                    return .handled
                }
                .onSubmit { focus = .grid }
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
        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(focus == .search ? Theme.accent : Theme.border, lineWidth: 1))
    }

    /// A click, Return: start placing that object, and hand the keys back to the stage.
    private func pick(_ name: String) {
        model.editor.choosePlaceObject(name)
        Task { _ = await model.thumbnails.art(name) }
        model.editor.focusStage()
    }

    /// The grid's keys: arrows through the cells in reading order across headings, Return picks, Esc closes,
    /// ⇧Tab goes back to the tabs (or the search).
    private func key(_ press: KeyPress, sections: [LibraryCategories.Section], proxy: ScrollViewProxy) -> KeyPress.Result {
        let objects = LibraryCategories.flattened(sections)
        if press.key == .tab, press.modifiers.contains(.shift), !press.modifiers.contains(.control) {
            focus = model.libraryTabNames.isEmpty ? .search : .tabs
            return .handled
        }
        guard !objects.isEmpty else { return press.key == .escape ? escape() : .ignored }
        let cursor = min(max(model.libraryCursor, 0), objects.count - 1)
        let counts = sections.map(\.objects.count)
        var next = cursor
        switch press.key {
        case .leftArrow: next = LibraryCategories.move(.left, from: cursor, counts: counts, columns: Self.columns) ?? cursor
        case .rightArrow: next = LibraryCategories.move(.right, from: cursor, counts: counts, columns: Self.columns) ?? cursor
        case .upArrow:
            guard let n = LibraryCategories.move(.up, from: cursor, counts: counts, columns: Self.columns) else {
                focus = model.libraryTabNames.isEmpty ? .search : .tabs
                return .handled
            }
            next = n
        case .downArrow: next = LibraryCategories.move(.down, from: cursor, counts: counts, columns: Self.columns) ?? cursor
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
