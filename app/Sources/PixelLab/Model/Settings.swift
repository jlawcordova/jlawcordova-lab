import AppKit
import Foundation

/// UserDefaults keys (also used by @AppStorage in SettingsView).
enum SettingsKey {
    /// The `.pixellab` document the app opens (empty: the default site checkout's).
    static let document = "documentPath"
    /// The site checkout, before documents (read once to seed `document`, never written).
    static let site = "sitePath"
    static let appearance = "appearance"
    static let lastDoc = "lastDoc"
    static let showGrid = "showGrid"
    /// The claude command the owner chose in the Agent tab's setup (empty: look for it).
    static let claude = "claudePath"
    /// The right column's tab: inspector or agent.
    static let rightTab = "rightTab"
}

enum AppearanceSetting: String, CaseIterable, Identifiable, Sendable {
    case auto, light, dark
    var id: String { rawValue }
    var title: String {
        switch self {
        case .auto: "Auto"
        case .light: "Light"
        case .dark: "Dark"
        }
    }
    var appearance: NSAppearance? {
        switch self {
        case .auto: nil
        case .light: NSAppearance(named: .aqua)
        case .dark: NSAppearance(named: .darkAqua)
        }
    }
}

/// What a snapshot run asks for: `PIXELLAB_SNAPSHOT=/path/out.png`, with
/// `PIXELLAB_SNAPSHOT_DOC=scene:hero-island` and
/// `PIXELLAB_SNAPSHOT_APPEARANCE=dark|light`. The app opens the doc, waits until
/// it and the visible library thumbnails are rendered, captures the main
/// window to PNG and quits.
struct SnapshotRequest: Sendable {
    var output: URL
    var doc: DocRef?
    var appearance: AppearanceSetting?
    var size: CGSize
    /// `PIXELLAB_SNAPSHOT_ZOOM=4`: draw at this zoom instead of Fit.
    var zoom: Int?

    static func fromEnvironment() -> SnapshotRequest? {
        let env = ProcessInfo.processInfo.environment
        guard let out = env["PIXELLAB_SNAPSHOT"], !out.isEmpty else { return nil }
        var size = CGSize(width: 1440, height: 900)
        if let s = env["PIXELLAB_SNAPSHOT_SIZE"], case let parts = s.split(separator: "x").compactMap({ Double($0) }), parts.count == 2 {
            size = CGSize(width: parts[0], height: parts[1])
        }
        return SnapshotRequest(
            output: URL(fileURLWithPath: NSString(string: out).expandingTildeInPath),
            doc: env["PIXELLAB_SNAPSHOT_DOC"].flatMap { DocRef(id: $0) },
            appearance: env["PIXELLAB_SNAPSHOT_APPEARANCE"].flatMap { AppearanceSetting(rawValue: $0) },
            size: size,
            zoom: env["PIXELLAB_SNAPSHOT_ZOOM"].flatMap { Int($0) }
        )
    }
}
