import SwiftUI

/// The inspector column (docs/intents/2026-10-agent-chat/spec.md, D12): a tab
/// bar, Inspector | ✦ Agent, over the Inspector or the Agent tab.
struct RightColumn: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 2) {
                tab(.inspector, "Inspector")
                tab(.agent, "Agent", sparkle: true)
            }
            .padding(2)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .padding(.horizontal, 12).padding(.vertical, 8)
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Right column")
            Rectangle().fill(Theme.border).frame(height: 1)
            if model.rightTab == .agent {
                AgentTab()
            } else {
                InspectorView()
            }
        }
        .background(Theme.sidebar)
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
            .frame(maxWidth: .infinity, minHeight: 26)
            .background(on ? Theme.page : Color.clear, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityAddTraits(on ? [.isSelected, .isButton] : .isButton)
        .help(t == .agent ? "Chat with Claude about the scene (⌥⌘A)" : "The selection, its position and the checks")
    }
}
