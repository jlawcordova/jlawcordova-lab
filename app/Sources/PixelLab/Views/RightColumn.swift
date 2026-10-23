import SwiftUI

/// The inspector column (docs/intents/2026-10-agent-chat/spec.md, D12): a tab row, **Inspector | ✦ Agent …
/// Publish**, over the Inspector or the Agent tab. Publish is here, in either tab, in scenes and objects
/// (docs/intents/2026-10-floating-library/spec.md, R14, D8). There is no Save: edits save themselves
/// (docs/intents/2026-10-autosave/spec.md, R5), and ⌘S saves a version.
struct RightColumn: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                HStack(spacing: 2) {
                    tab(.inspector, "Inspector")
                    tab(.agent, "Agent", sparkle: true)
                }
                .padding(2)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .accessibilityElement(children: .contain)
                .accessibilityLabel("Right column tabs")
                Spacer(minLength: 4)
                publishButton
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
            Rectangle().fill(Theme.border).frame(height: 1)
            if model.rightTab == .agent {
                AgentTab()
            } else {
                InspectorView()
            }
        }
        .background(Theme.sidebar)
        .onGeometryChange(for: CGFloat.self, of: { $0.size.width }) { model.rightColumnWidth = $0 }
    }

    private func tab(_ t: RightTab, _ title: String, sparkle: Bool = false) -> some View {
        let on = model.rightTab == t
        return Button {
            if t == .agent { model.showAgentTab() } else { model.rightTab = .inspector }
        } label: {
            HStack(spacing: 5) {
                if sparkle { Image(systemName: "sparkle").font(.system(size: 11)).foregroundStyle(Theme.accent) }
                Text(title).font(.system(size: 12.5, weight: on ? .semibold : .regular)).foregroundStyle(on ? Theme.ink : Theme.inkMuted)
            }
            .padding(.horizontal, 10)
            .frame(minHeight: 26)
            .background(on ? Theme.page : Color.clear, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityAddTraits(on ? [.isSelected, .isButton] : .isButton)
        .help(t == .agent ? "Chat with Claude about the scene (⌥⌘A)" : "The selection, its position and the checks")
    }

    private var publishButton: some View {
        Button {
            Task { await model.publish() }
        } label: {
            Image(systemName: "square.and.arrow.up")
                .font(.system(size: 13))
                .foregroundStyle(Theme.ink)
                .frame(width: 28, height: 28)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(model.build?.running == true)
        .help("Publish (⌘B)")
        .accessibilityLabel("Publish")
    }
}
