import AppKit
import CoreText
import SwiftUI

/// Board 03, "Field notebook" (docs/plan.md): the site's warm palette in a
/// real Mac window. Every token has a night (dark) and a day (light) value and
/// follows the effective appearance, which the Appearance setting drives.
enum Theme {
    /// Window ground.
    static let page = dynamic(night: 0x1C1712, day: 0xF4EDE0)
    /// Toolbar and stage.
    static let hero = dynamic(night: 0x262019, day: 0xEADFC8)
    /// Sidebar and inspector.
    static let sidebar = dynamic(night: 0x211B15, day: 0xEFE6D6)
    /// Thumbnails and fields.
    static let card = dynamic(night: 0x2D251C, day: 0xE9DCC6)
    static let ink = dynamic(night: 0xF1E8D8, day: 0x2E2418)
    static let inkMuted = dynamic(night: 0xC8B9A1, day: 0x5C4B39)
    /// Ink at 16% (night), rgba(90,62,43,.16) (day).
    static let border = dynamic(night: 0xF1E8D8, nightAlpha: 0.16, day: 0x5A3E2B, dayAlpha: 0.16)
    /// Selection, the primary button, the app tint.
    static let accent = dynamic(night: 0x8CBC86, day: 0x3F6B45)
    /// Text on the accent.
    static let onAccent = dynamic(night: 0x1C1712, day: 0xF4EDE0)
    /// Playhead and warnings; a fill only, never text on light.
    static let gold = dynamic(night: 0xD8B66A, day: 0xD8B66A)
    /// The iso grid: ink at 6% (night), rgba(90,62,43,.10) (day).
    static let grid = dynamic(night: 0xF1E8D8, nightAlpha: 0.07, day: 0x5A3E2B, dayAlpha: 0.12)
    /// The lattice's origin lines, a little stronger.
    static let gridStrong = dynamic(night: 0xF1E8D8, nightAlpha: 0.13, day: 0x5A3E2B, dayAlpha: 0.2)
    /// Under the stage's dashed outlines, so they read over any art.
    static let halo = dynamic(night: 0x1C1712, nightAlpha: 0.7, day: 0xF4EDE0, dayAlpha: 0.8)
    /// A selected row: the accent, faint.
    static let selection = dynamic(night: 0x8CBC86, nightAlpha: 0.18, day: 0x3F6B45, dayAlpha: 0.16)

    /// A palette color, "#RRGGBB".
    static func hexColor(_ hex: String) -> Color {
        let v = UInt32(hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) ?? 0
        return Color(nsColor: NSColor(hex: v))
    }

    static func dynamic(night: UInt32, nightAlpha: CGFloat = 1, day: UInt32, dayAlpha: CGFloat = 1) -> Color {
        Color(nsColor: dynamicNS(night: night, nightAlpha: nightAlpha, day: day, dayAlpha: dayAlpha))
    }

    static func dynamicNS(night: UInt32, nightAlpha: CGFloat = 1, day: UInt32, dayAlpha: CGFloat = 1) -> NSColor {
        NSColor(name: nil) { appearance in
            isDark(appearance) ? NSColor(hex: night, alpha: nightAlpha) : NSColor(hex: day, alpha: dayAlpha)
        }
    }

    static func isDark(_ appearance: NSAppearance) -> Bool {
        appearance.bestMatch(from: [.darkAqua, .aqua, .vibrantDark, .vibrantLight, .accessibilityHighContrastDarkAqua, .accessibilityHighContrastAqua])
            .map { $0 == .darkAqua || $0 == .vibrantDark || $0 == .accessibilityHighContrastDarkAqua } ?? false
    }

    // MARK: Type

    /// Sora for document and item names and headings, else SF Pro semibold.
    static func nameFont(_ size: CGFloat, weight: Font.Weight = .semibold) -> Font {
        if Fonts.sora { return .custom("Sora", size: size).weight(weight) }
        return .system(size: size, weight: weight, design: .default)
    }

    /// Silkscreen for short uppercase section labels, else SF Mono 11.
    static func labelFont(_ size: CGFloat = 11) -> Font {
        if Fonts.silkscreen { return .custom("Silkscreen", size: size - 1) }
        return .system(size: size, weight: .medium, design: .monospaced)
    }

    /// SF Mono for coordinates and code.
    static func monoFont(_ size: CGFloat = 11) -> Font { .system(size: size, design: .monospaced) }
}

extension NSColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1) {
        self.init(
            srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha
        )
    }
}

/// A section label: SCENES, LIBRARY, SELECTED, CHECKS, TIMELINE.
struct SectionLabel: View {
    var title: String
    var body: some View {
        Text(title.uppercased())
            .font(Theme.labelFont())
            .tracking(1.2)
            .foregroundStyle(Theme.inkMuted)
            .accessibilityAddTraits(.isHeader)
    }
}

/// Registers Sora and Silkscreen from Resources/Fonts when the files are there.
enum Fonts {
    nonisolated(unsafe) private(set) static var sora = false
    nonisolated(unsafe) private(set) static var silkscreen = false

    @MainActor static func register() {
        // `.process` may flatten Resources/Fonts into the bundle root: look everywhere in it.
        guard let dir = AppResources.bundle?.resourceURL,
              let files = FileManager.default.enumerator(at: dir, includingPropertiesForKeys: nil)
        else { return }
        for case let url as URL in files where ["ttf", "otf"].contains(url.pathExtension.lowercased()) {
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        }
        let families = Set(NSFontManager.shared.availableFontFamilies)
        sora = families.contains("Sora")
        silkscreen = families.contains("Silkscreen")
    }
}

/// The SwiftPM resource bundle, looked up where `build-app.sh` puts it
/// (Contents/Resources) and next to the executable (`swift run`). Unlike
/// `Bundle.module` it never traps when the bundle is missing.
enum AppResources {
    static let bundle: Bundle? = {
        let name = "PixelLab_PixelLab.bundle"
        var dirs: [URL] = []
        if let r = Bundle.main.resourceURL { dirs.append(r) }
        dirs.append(Bundle.main.bundleURL)
        if let exe = Bundle.main.executableURL { dirs.append(exe.deletingLastPathComponent()) }
        for dir in dirs {
            let url = dir.appendingPathComponent(name)
            if let b = Bundle(url: url) { return b }
        }
        return nil
    }()

    static func url(for path: String) -> URL? {
        guard let base = bundle?.resourceURL else { return nil }
        let url = base.appendingPathComponent(path)
        return FileManager.default.fileExists(atPath: url.path) ? url : nil
    }

    static var appIcon: NSImage? {
        url(for: "AppIcon.png").flatMap { NSImage(contentsOf: $0) }
    }
}
