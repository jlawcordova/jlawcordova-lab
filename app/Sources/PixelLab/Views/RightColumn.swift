import SwiftUI

/// The inspector column (docs/intents/2026-10-agent-chat/spec.md, D12): a tab row, **Inspector | ✦ Agent ⟨n⟩ …
/// Publish · Save**, over the Inspector or the Agent tab. Publish and Save are here, in either tab, in scenes
/// and objects (docs/intents/2026-10-floating-library/spec.md, R14, D8).
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
                saveButton
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
    }

    private func tab(_ t: RightTab, _ title: String, sparkle: Bool = false) -> some View {
        let on = model.rightTab == t
        let n = t == .agent ? model.agent.pendingCount : 0
        return Button {
            if t == .agent { model.showAgentTab() } else { model.rightTab = .inspector }
        } label: {
            HStack(spacing: 5) {
                if sparkle { Image(systemName: "sparkle").font(.system(size: 11)).foregroundStyle(Theme.accent) }
                Text(title).font(.system(size: 12.5, weight: on ? .semibold : .regular)).foregroundStyle(on ? Theme.ink : Theme.inkMuted)
                if n > 0 {
                    // The waiting proposals, as the toolbar's Agent button showed them.
                    Text("\(n)")
                        .font(.system(size: 10, weight: .bold).monospacedDigit())
                        .foregroundStyle(Theme.onAccent)
                        .padding(.horizontal, 5)
                        .frame(minWidth: 16, minHeight: 16)
                        .background(Theme.gold, in: Capsule())
                }
            }
            .padding(.horizontal, 10)
            .frame(minHeight: 26)
            .background(on ? Theme.page : Color.clear, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Self.tabLabel(title, pending: n))
        .accessibilityAddTraits(on ? [.isSelected, .isButton] : .isButton)
        .help(t == .agent ? (n == 0 ? "Chat with Claude about the scene (⌥⌘A)" : "\(n) proposal\(n == 1 ? "" : "s") to review (⌥⌘A)") : "The selection, its position and the checks")
    }

    /// "Agent", or "Agent, 2 proposals to review".
    static func tabLabel(_ title: String, pending n: Int) -> String {
        n == 0 ? title : "\(title), \(n) proposal\(n == 1 ? "" : "s") to review"
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

    private var saveButton: some View {
        Button("Save") { Task { await model.save() } }
            .buttonStyle(AccentButtonStyle(height: 26))
            .disabled(!model.canSave)
            .help("Save the source (⌘S)")
            .accessibilityLabel(Self.saveLabel(unsaved: model.canSave))
    }

    /// Save's VoiceOver label says whether anything is unsaved.
    static func saveLabel(unsaved: Bool) -> String { unsaved ? "Save, unsaved changes" : "Save, all saved" }
}
