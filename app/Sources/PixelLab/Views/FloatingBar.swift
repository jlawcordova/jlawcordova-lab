import SwiftUI

/// What the floating bar holds, by id: in a scene Select, Hand and the Library; in an object the paint
/// tools and Hand (docs/intents/2026-10-floating-library/spec.md, R6).
enum FloatingBarItems {
    static func ids(for kind: DocRef.Kind) -> [String] {
        kind == .scene ? ["select", "pan", "library"] : PaintTool.allCases.map(\.rawValue) + ["pan"]
    }
}

/// The bar at the stage's bottom center (R6, D4): icons only, 36 pt targets, over the art, outside the
/// scroll view so it moves with neither scrolling nor zoom. A click focuses the stage so the keys keep working.
struct FloatingBar: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let editor = model.editor
        let objects = model.objectEditor
        HStack(spacing: 2) {
            if model.document?.ref.kind == .object {
                ForEach(PaintTool.allCases) { tool in
                    FloatingBarButton(symbol: tool.symbol, title: tool.title, key: String(tool.key).uppercased(),
                                      on: objects.tool == tool && !objects.handHeld) {
                        objects.setTool(tool)
                        editor.focusStage()
                    }
                }
                FloatingBarDivider()
                FloatingBarButton(symbol: StageTool.pan.symbol, title: StageTool.pan.title, key: "H", on: objects.handHeld) {
                    objects.setHand(!objects.handHeld)
                    editor.focusStage()
                }
            } else {
                ForEach(StageTool.allCases) { tool in
                    FloatingBarButton(symbol: tool.symbol, title: tool.title, key: String(tool.key).uppercased(), on: editor.tool == tool) {
                        editor.setTool(tool)
                        editor.focusStage()
                    }
                }
                FloatingBarDivider()
                FloatingBarButton(symbol: "square.grid.2x2", title: "Library", key: "L", on: model.libraryOpen) {
                    model.toggleLibrary()
                }
            }
        }
        .padding(4)
        .background(Theme.sidebar, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.border, lineWidth: 1))
        .shadow(color: .black.opacity(0.22), radius: 10, y: 3)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Tools")
    }
}

/// One button of the bar: a symbol, the selected one marked. Its help tag and its VoiceOver label name it and its key.
struct FloatingBarButton: View {
    var symbol: String
    var title: String
    var key: String
    var on: Bool
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 15))
                .foregroundStyle(on ? Theme.accent : Theme.ink)
                .frame(width: 36, height: 36)
                .background(on ? Theme.selection : Color.clear, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("\(title) (\(key))")
        .accessibilityLabel("\(title), \(key)")
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

struct FloatingBarDivider: View {
    var body: some View {
        Rectangle()
            .fill(Theme.border)
            .frame(width: 1, height: 20)
            .padding(.horizontal, 3)
            .accessibilityHidden(true)
    }
}
