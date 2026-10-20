import AppKit
import SwiftUI

/// The layer menu (docs/intents/2026-10-scene-layers/spec.md, D9): drawn in
/// the window, over everything, where the stage or a Layers row was
/// right-clicked. Group ⌘G and Ungroup ⇧⌘G, a separator, then Go Inside,
/// Hide and Lock. It's drawn rather than an `NSMenu` so the script hook can
/// drive it and snapshots can show it; the same actions are in the Edit menu
/// and the inspector.
struct LayerMenuLayer: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        ZStack(alignment: .topLeading) {
            MenuAnchor { model.layers.menuAnchor = $0 }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
            if let menu = model.layers.menu {
                // A click (or a right-click) anywhere else closes it.
                Color.clear
                    .contentShape(Rectangle())
                    .onTapGesture { model.layers.closeMenu() }
                    .overlay(RightClickCatcher(tag: "menu") { _ in model.layers.closeMenu() })
                    .accessibilityHidden(true)
                LayerMenuView(state: menu)
                    .offset(x: menu.origin.x, y: menu.origin.y)
            }
        }
    }
}

/// The menu itself: a board 03 card, the hovered or keyboard item in the accent.
struct LayerMenuView: View {
    var state: LayerMenuState
    @Environment(AppModel.self) private var model
    @FocusState private var focused: Bool

    var body: some View {
        let layers = model.layers
        let items = layers.menuItems
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.element.id) { i, item in
                if item.separatorBefore {
                    Rectangle().fill(Theme.border).frame(height: 1).padding(.vertical, 4).padding(.horizontal, 6)
                        .accessibilityHidden(true)
                }
                LayerMenuRow(item: item, highlighted: state.highlighted == i) {
                    Task { await layers.perform(item.action) }
                }
                .onHover { inside in
                    if inside, item.enabled { layers.menu?.highlighted = i }
                }
            }
        }
        .padding(5)
        .frame(width: LayersModel.menuSize.width, alignment: .leading)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .shadow(color: .black.opacity(0.3), radius: 14, y: 8)
        .focusable()
        .focused($focused)
        .focusEffectDisabled()
        .onAppear { focused = true }
        .onKeyPress(.upArrow) { layers.moveMenuHighlight(-1); return .handled }
        .onKeyPress(.downArrow) { layers.moveMenuHighlight(1); return .handled }
        .onKeyPress(.escape) { layers.closeMenu(); return .handled }
        .onKeyPress(.return) {
            if let h = state.highlighted, items.indices.contains(h), items[h].enabled { Task { await layers.perform(items[h].action) } }
            return .handled
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Layer actions")
        .accessibilityHint("Up and down arrows move, Return chooses, Escape closes")
    }
}

struct LayerMenuRow: View {
    var item: LayerMenuItem
    var highlighted: Bool
    var action: () -> Void

    var body: some View {
        let lit = highlighted && item.enabled
        Button(action: action) {
            HStack(spacing: 12) {
                Text(item.title).font(.system(size: 13))
                Spacer(minLength: 8)
                if !item.shortcut.isEmpty {
                    Text(item.shortcut).font(.system(size: 12)).foregroundStyle(lit ? Theme.onAccent : Theme.inkMuted)
                }
            }
            .foregroundStyle(lit ? Theme.onAccent : Theme.ink)
            .padding(.horizontal, 9)
            .frame(height: 26)
            .background(lit ? Theme.accent : .clear, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!item.enabled)
        .opacity(item.enabled ? 1 : 0.4)
        .accessibilityLabel(item.shortcut.isEmpty ? item.title : "\(item.title), \(Self.spoken(item.shortcut))")
    }

    /// "⇧⌘G" → "Shift Command G".
    static func spoken(_ shortcut: String) -> String {
        shortcut.map { c -> String in
            switch c {
            case "⌘": "Command"
            case "⇧": "Shift"
            case "⌥": "Option"
            case "⌃": "Control"
            default: String(c)
            }
        }.joined(separator: " ")
    }
}

/// A flipped AppKit view under the menu layer: window points convert into
/// it, so the menu opens where the pointer was.
struct MenuAnchor: NSViewRepresentable {
    var register: (NSView) -> Void

    func makeNSView(context: Context) -> AnchorView {
        let v = AnchorView()
        register(v)
        return v
    }

    func updateNSView(_ v: AnchorView, context: Context) { register(v) }

    final class AnchorView: NSView {
        override var isFlipped: Bool { true }
        override func hitTest(_ point: NSPoint) -> NSView? { nil }
    }
}
