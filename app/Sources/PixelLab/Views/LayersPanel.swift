import AppKit
import SwiftUI

/// LAYERS (docs/intents/2026-10-scene-layers/spec.md, D11): the open scene's
/// layers, each with its layers and then its items, as a tree. The site's
/// `{ group }` wrappers aren't in it. Clicking a row selects it (⇧ adds),
/// double-clicking a layer goes inside it, right-clicking opens the layer
/// menu. With the tree focused, the keys select (↑ ↓, ⇧ to add), expand and
/// collapse (→ ←), go inside (Return) and leave (Esc). The search field
/// filters it to a flat list.
struct LayersPanel: View {
    var doc: DocumentModel
    var query: String
    @Environment(AppModel.self) private var model
    @FocusState private var focused: Bool

    var body: some View {
        let layers = model.layers
        let rows = layers.visibleRows(query: query)
        SectionLabel(title: "Layers")
            .padding(.horizontal, 6)
            .padding(.top, 4)
            .padding(.bottom, 2)
        if !doc.itemsLoaded {
            HStack(spacing: 6) {
                ProgressView().controlSize(.mini)
                Text("Loading layers…").font(.caption).foregroundStyle(Theme.inkMuted)
            }
            .padding(.horizontal, 6)
        } else if rows.isEmpty {
            Text(query.isEmpty ? "No items yet: place one from the Library." : "Nothing matches")
                .font(.callout).foregroundStyle(Theme.inkMuted).padding(.horizontal, 6).padding(.vertical, 4)
        }
        LazyVStack(alignment: .leading, spacing: 1) {
            ForEach(rows) { row in
                LayerRow(row: row, doc: doc, flat: !query.isEmpty) { focused = true }
                    .id(row.id)
            }
        }
        .padding(2)
        // Visible focus: an accent ring while the tree takes the keys.
        .overlay(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .strokeBorder(Theme.accent.opacity(focused ? 0.7 : 0), lineWidth: 2)
                .allowsHitTesting(false)
        )
        .focusable()
        .focused($focused)
        .focusEffectDisabled()
        .onKeyPress(phases: .down) { press in
            guard let k = Self.keyInput(press) else { return .ignored }
            return model.layers.treeKey(k, query: query) ? .handled : .ignored
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Layers of \(doc.ref.name)")
        .accessibilityHint("Arrow keys select, Shift adds, Right and Left expand and collapse, Return goes inside a layer, Escape leaves it")
    }

    static func keyInput(_ press: KeyPress) -> KeyInput? {
        let key: KeyInput.Key
        switch press.key {
        case .upArrow: key = .up
        case .downArrow: key = .down
        case .leftArrow: key = .left
        case .rightArrow: key = .right
        case .return: key = .returnKey
        case .escape: key = .escape
        default: return nil
        }
        let m = press.modifiers
        return KeyInput(key: key, option: m.contains(.option), shift: m.contains(.shift), command: m.contains(.command), control: m.contains(.control))
    }
}

/// A row of the Layers tree: a layer (chevron, dashed square, its name in
/// Sora, the lock and the eye) or an item (cube, object, position).
struct LayerRow: View {
    var row: LayerTree.Row
    var doc: DocumentModel
    var flat: Bool
    /// Gives the tree keyboard focus (a click on a row).
    var focusTree: () -> Void
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let layers = model.layers
        let tree = doc.layerTree
        let selected = doc.selection.contains(row.target)
        let hovering = model.hovered == "row:\(row.id)" || layers.hover == row.target
        HStack(spacing: 0) {
            if !flat { Color.clear.frame(width: CGFloat(row.depth) * 14, height: 1) }
            switch row.target {
            case .layer(let id): layerRow(id, tree: tree, selected: selected, hovering: hovering)
            case .item(let path): itemRow(path, tree: tree, selected: selected)
            }
        }
        .padding(.trailing, 2)
        .frame(height: 28)
        .background(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(selected ? Theme.selection : (hovering ? Theme.card : .clear))
        )
        .opacity(dimmed(tree) ? 0.4 : 1)
        .overlay(RightClickCatcher(tag: row.id) { point in layers.openMenu(atWindow: point, target: row.target) })
        .onHover { model.setHovered("row:\(row.id)", $0) }
        .background(RowFrameReporter(id: row.id))
    }

    /// Inside a layer, rows outside it dim (the layers that hold it don't).
    private func dimmed(_ tree: LayerTree) -> Bool {
        guard let inside = model.layers.inside else { return false }
        if tree.isWithin(row.target, inside) { return false }
        if case .layer(let id) = row.target, tree.chain(ofLayer: inside).contains(id) { return false }
        return true
    }

    private func choose() {
        model.layers.selectRow(row.target, shift: NSEvent.modifierFlags.contains(.shift))
        focusTree()
    }

    @ViewBuilder
    private func layerRow(_ id: String, tree: LayerTree, selected: Bool, hovering: Bool) -> some View {
        let layers = model.layers
        let name = tree.name(of: id)
        let open = layers.expanded.contains(id)
        let hidden = layers.hidden.contains(id)
        let locked = layers.locked.contains(id)
        let count = tree.byID[id]?.items ?? 0
        Button { layers.toggleExpanded(id) } label: {
            Image(systemName: "chevron.right")
                .font(.system(size: 9, weight: .semibold))
                .rotationEffect(.degrees(open ? 90 : 0))
                .animation(reduceMotion ? nil : .easeOut(duration: 0.12), value: open)
                .foregroundStyle(Theme.inkMuted)
                .frame(width: 18, height: 28)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(open ? "Collapse \(name)" : "Expand \(name)")
        Button(action: choose) {
            HStack(spacing: 6) {
                Image(systemName: "square.dashed")
                    .font(.system(size: 12.5))
                    .foregroundStyle(selected ? Theme.accent : Theme.inkMuted)
                    .frame(width: 15)
                Text(name)
                    .font(Theme.nameFont(12.5, weight: hidden ? .regular : .semibold))
                    .foregroundStyle(hidden ? Theme.inkMuted : Theme.ink)
                    .lineLimit(1)
                    .truncationMode(.tail)
                if layers.inside == id {
                    Text("IN")
                        .font(Theme.labelFont(9.5))
                        .foregroundStyle(Theme.page)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 1)
                        .background(Theme.gold, in: RoundedRectangle(cornerRadius: 3, style: .continuous))
                        .accessibilityHidden(true)
                }
                Spacer(minLength: 2)
            }
            .padding(.leading, 2)
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .simultaneousGesture(TapGesture(count: 2).onEnded { layers.goInside(id) })
        .accessibilityLabel("Layer \(name), \(count) item\(count == 1 ? "" : "s")\(hidden ? ", hidden" : "")\(locked ? ", locked" : "")\(layers.inside == id ? ", you're inside it" : "")")
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityAction(named: "Go Inside") { layers.goInside(id) }
        .accessibilityAction(named: hidden ? "Show" : "Hide") { layers.toggleHidden(id) }
        .accessibilityAction(named: locked ? "Unlock" : "Lock") { layers.toggleLocked(id) }
        .accessibilityAction(named: open ? "Collapse" : "Expand") { layers.toggleExpanded(id) }
        let lockShown = locked || hovering || selected
        RowIconButton(symbol: locked ? "lock.fill" : "lock.open", on: locked, label: locked ? "Unlock \(name)" : "Lock \(name)") { layers.toggleLocked(id) }
            .opacity(lockShown ? (locked ? 1 : 0.75) : 0)
            .allowsHitTesting(lockShown)
        RowIconButton(symbol: hidden ? "eye.slash" : "eye", on: hidden, label: hidden ? "Show \(name)" : "Hide \(name)") { layers.toggleHidden(id) }
    }

    @ViewBuilder
    private func itemRow(_ path: String, tree: LayerTree, selected: Bool) -> some View {
        let item = doc.itemsByPath[path]
        let object = item?.object ?? path
        Color.clear.frame(width: 18, height: 1)
        Button(action: choose) {
            HStack(spacing: 6) {
                Image(systemName: "cube")
                    .font(.system(size: 11))
                    .foregroundStyle(selected ? Theme.accent : Theme.inkMuted)
                    .frame(width: 15)
                Text(object)
                    .font(.system(size: 12, weight: selected ? .semibold : .regular))
                    .foregroundStyle(Theme.ink)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: 2)
                Text(item?.at.short ?? "")
                    .font(Theme.monoFont(10))
                    .foregroundStyle(Theme.inkMuted)
                    .lineLimit(1)
                    .padding(.trailing, 4)
            }
            .padding(.leading, 2)
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(Self.spokenName(item) ?? path), \(item?.at.spoken ?? "")\(tree.layerOfItem[path].map { ", in \(tree.name(of: $0))" } ?? "")")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

extension LayerRow {
    /// "truck-open truck-tip": the object, and its id when it has one.
    static func spokenName(_ item: SceneItemInfo?) -> String? {
        guard let item else { return nil }
        return item.id.map { "\(item.object) \($0)" } ?? item.object
    }
}

/// The eye or the lock: 24 pt, muted until it's on.
struct RowIconButton: View {
    var symbol: String
    var on: Bool
    var label: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 11.5))
                .foregroundStyle(on ? Theme.ink : Theme.inkMuted)
                .frame(width: 24, height: 24)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
    }
}

/// Over a row: takes right-clicks (and ⌃-clicks) only, so every other click
/// goes to the row's own buttons, and hands the window point to `action`.
struct RightClickCatcher: NSViewRepresentable {
    /// What it's over ("layer:train", "menu"), as its identifier.
    var tag = ""
    var action: (CGPoint) -> Void

    func makeNSView(context: Context) -> CatcherView {
        let v = CatcherView()
        v.action = action
        v.identifier = NSUserInterfaceItemIdentifier(tag)
        return v
    }

    func updateNSView(_ v: CatcherView, context: Context) {
        v.action = action
        v.identifier = NSUserInterfaceItemIdentifier(tag)
    }

    final class CatcherView: NSView {
        var action: ((CGPoint) -> Void)?

        override func hitTest(_ point: NSPoint) -> NSView? {
            guard let e = NSApp.currentEvent else { return nil }
            switch e.type {
            case .rightMouseDown, .rightMouseUp: return super.hitTest(point)
            case .leftMouseDown where e.modifierFlags.contains(.control): return super.hitTest(point)
            default: return nil
            }
        }

        override func rightMouseDown(with event: NSEvent) { action?(event.locationInWindow) }
        override func rightMouseUp(with event: NSEvent) {}
        override func mouseDown(with event: NSEvent) {
            if event.modifierFlags.contains(.control) { action?(event.locationInWindow) }
        }
        override func menu(for event: NSEvent) -> NSMenu? { nil }
    }
}

/// Keeps each row's frame in window points, for the script hook's real right-clicks.
private struct RowFrameReporter: View {
    var id: String
    @Environment(AppModel.self) private var model

    var body: some View {
        Color.clear
            .onGeometryChange(for: CGRect.self, of: { $0.frame(in: .global) }) { model.layers.rowFrames[id] = $0 }
            .onDisappear { model.layers.rowFrames[id] = nil }
    }
}
