import CoreGraphics
import Foundation

/// The alpha channel of an image, row-major from the top-left, one byte per pixel.
struct AlphaMask: Sendable, Equatable {
    var width: Int
    var height: Int
    var alpha: [UInt8]

    init(width: Int, height: Int, alpha: [UInt8]) {
        self.width = width
        self.height = height
        self.alpha = alpha
    }

    /// Reads the alpha of a CGImage by drawing it into an RGBA buffer.
    init?(cgImage: CGImage) {
        let w = cgImage.width, h = cgImage.height
        guard w > 0, h > 0, w * h <= 16_000_000 else { return nil }
        var rgba = [UInt8](repeating: 0, count: w * h * 4)
        let ok = rgba.withUnsafeMutableBytes { buf -> Bool in
            guard let ctx = CGContext(
                data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return false }
            ctx.interpolationQuality = .none
            ctx.draw(cgImage, in: CGRect(x: 0, y: 0, width: w, height: h))
            return true
        }
        guard ok else { return nil }
        width = w
        height = h
        alpha = stride(from: 3, to: rgba.count, by: 4).map { rgba[$0] }
    }

    /// Whether the pixel is painted (outside the image: false).
    func isOpaque(x: Int, y: Int) -> Bool {
        guard x >= 0, y >= 0, x < width, y < height else { return false }
        return alpha[y * width + x] > 0
    }
}

/// An object's art as `object.render` returns it, for thumbnails, ghosts and
/// pixel-accurate hit-testing.
struct ObjectArt: @unchecked Sendable {
    var image: PixelImage
    /// The object pixels the PNG covers (anchor at 0, 0).
    var viewBox: CGRect
    /// The box around the painted pixels, in object pixels (nil if blank).
    var box: CGRect?
    /// The PNG's render scale.
    var scale: Int
    var mask: AlphaMask?

    /// Whether object pixel (x, y) (anchor at 0, 0) is painted.
    func isOpaque(objectX x: Int, objectY y: Int) -> Bool {
        guard let mask else { return true }
        let s = max(scale, 1)
        let px = (x - Int(viewBox.minX)) * s + s / 2
        let py = (y - Int(viewBox.minY)) * s + s / 2
        return mask.isOpaque(x: px, y: py)
    }

    /// Where the PNG's top-left goes for an item whose painted box starts at
    /// `itemOrigin` in scene pixels.
    func imageOrigin(forItemBoxOrigin itemOrigin: CGPoint) -> CGPoint {
        let b = box ?? viewBox
        return CGPoint(x: itemOrigin.x - (b.minX - viewBox.minX), y: itemOrigin.y - (b.minY - viewBox.minY))
    }

    /// Where the PNG's top-left goes for an item placed at `point` (its anchor).
    func imageOrigin(forPlacementPoint point: CGPoint) -> CGPoint {
        CGPoint(x: point.x + viewBox.minX, y: point.y + viewBox.minY)
    }
}

/// Hit-testing a click against the scene's items: the render's bounds pick
/// the candidates, topmost (latest painted) first; each candidate's own
/// pixels decide, so a click on a transparent corner of a tree's box goes to
/// the block behind it.
enum HitTester {
    struct Candidate: Equatable {
        var path: String
        /// The item's painted box in scene pixels.
        var rect: CGRect
        /// The object drawn (its pose at this time, if any).
        var object: String
    }

    /// The items whose box holds scene pixel (x, y), topmost first.
    static func candidates(atPixel x: Int, _ y: Int, in items: [Candidate]) -> [Candidate] {
        let p = CGPoint(x: CGFloat(x) + 0.5, y: CGFloat(y) + 0.5)
        return items.reversed().filter { $0.rect.contains(p) }
    }

    /// The topmost item painted at scene pixel (x, y). `opaque` says whether a
    /// candidate is painted at object pixel (dx, dy) from its box's origin, or
    /// nil if its art isn't known (then its box counts).
    static func pick(
        atPixel x: Int, _ y: Int, in items: [Candidate],
        opaque: (Candidate, _ dx: Int, _ dy: Int) -> Bool?
    ) -> String? {
        for c in candidates(atPixel: x, y, in: items) {
            let dx = x - Int(c.rect.minX.rounded(.down)), dy = y - Int(c.rect.minY.rounded(.down))
            if opaque(c, dx, dy) ?? true { return c.path }
        }
        return nil
    }
}

/// Finds the item a check problem names: `scenes/x.mjs: items[5].items[2]: …`
/// is path "5.2"; a quoted id (a motion track's item) is matched against the
/// scene's ids.
enum ProblemLocator {
    static func path(in text: String) -> String? {
        guard let start = text.range(of: "items[") else { return nil }
        var indices: [String] = []
        var rest = text[start.lowerBound...]
        while rest.hasPrefix("items[") {
            rest = rest.dropFirst("items[".count)
            guard let close = rest.firstIndex(of: "]") else { break }
            let n = rest[..<close]
            guard !n.isEmpty, n.allSatisfy(\.isNumber) else { break }
            indices.append(String(n))
            rest = rest[rest.index(after: close)...]
            if rest.hasPrefix(".items[") { rest = rest.dropFirst() } else { break }
        }
        return indices.isEmpty ? nil : indices.joined(separator: ".")
    }

    /// The quoted words in a problem ("…track "jib"…" → ["jib"]).
    static func quoted(in text: String) -> [String] {
        var out: [String] = []
        for quote in ["\"", "'"] {
            let parts = text.components(separatedBy: quote)
            guard parts.count >= 3 else { continue }
            for i in stride(from: 1, to: parts.count - 1, by: 2) { out.append(parts[i]) }
        }
        return out
    }

    /// The path of the item a problem names, if the scene has it.
    static func locate(_ text: String, items: [SceneItemInfo]) -> String? {
        if let p = path(in: text), items.contains(where: { $0.path == p }) { return p }
        let ids = quoted(in: text)
        return items.first { item in item.id.map { ids.contains($0) } ?? false }?.path
    }
}
