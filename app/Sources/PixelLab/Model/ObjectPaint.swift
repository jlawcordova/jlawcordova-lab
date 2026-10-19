import CoreGraphics
import Foundation

/// A pixel of an object, in `object.render` coordinates at scale 1: a
/// sprite's map column and row; a block's render pixel (its viewBox starts at
/// the block's top-left pixel). These are the coordinates `object.paint`,
/// `object.erase`, `object.fill` and `object.pick` take.
struct PixelPoint: Hashable, Sendable, CustomStringConvertible {
    var x: Int
    var y: Int
    init(_ x: Int, _ y: Int) {
        self.x = x
        self.y = y
    }
    var description: String { "\(x),\(y)" }
    var json: JSONValue { [JSONValue(x), JSONValue(y)] }

    /// "3,4" → (3, 4).
    init?(spec: String) {
        let parts = spec.split(separator: ",").compactMap { Int($0) }
        guard parts.count == 2 else { return nil }
        self.init(parts[0], parts[1])
    }
}

/// Stroke geometry: the pixels between two mouse samples, so a fast drag
/// leaves no gaps (the lab's `line`, Bresenham's algorithm).
enum StrokeMath {
    /// Every pixel on the line from `a` to `b`, both included, in order.
    static func line(_ a: PixelPoint, _ b: PixelPoint) -> [PixelPoint] {
        var out: [PixelPoint] = []
        let dx = abs(b.x - a.x), dy = -abs(b.y - a.y)
        let sx = a.x < b.x ? 1 : -1, sy = a.y < b.y ? 1 : -1
        var err = dx + dy
        var x = a.x, y = a.y
        while true {
            out.append(PixelPoint(x, y))
            if x == b.x && y == b.y { return out }
            let e2 = 2 * err
            if e2 >= dy { err += dy; x += sx }
            if e2 <= dx { err += dx; y += sy }
        }
    }

    /// The object pixel under a view point (nil outside the object's box).
    static func pixel(atView p: CGPoint, geometry: StageGeometry) -> PixelPoint? {
        let q = unclampedPixel(atView: p, geometry: geometry)
        return contains(q, size: geometry.viewBox.size) ? q : nil
    }

    /// The object pixel under a view point, even outside the box (a stroke
    /// that leaves the art keeps its line).
    static func unclampedPixel(atView p: CGPoint, geometry: StageGeometry) -> PixelPoint {
        let s = geometry.scenePixel(fromView: p)
        return PixelPoint(s.x - Int(geometry.viewBox.minX.rounded(.down)), s.y - Int(geometry.viewBox.minY.rounded(.down)))
    }

    /// A pixel's square in view points.
    static func viewRect(of p: PixelPoint, geometry: StageGeometry) -> CGRect {
        CGRect(
            x: geometry.artOrigin.x + CGFloat(p.x) * geometry.zoom,
            y: geometry.artOrigin.y + CGFloat(p.y) * geometry.zoom,
            width: geometry.zoom, height: geometry.zoom)
    }

    static func contains(_ p: PixelPoint, size: CGSize) -> Bool {
        p.x >= 0 && p.y >= 0 && CGFloat(p.x) < size.width && CGFloat(p.y) < size.height
    }

    /// The pixels of `points` inside the box, in order, each once.
    static func clip(_ points: [PixelPoint], size: CGSize) -> [PixelPoint] {
        var seen = Set<PixelPoint>()
        return points.filter { contains($0, size: size) && seen.insert($0).inserted }
    }
}

/// A stroke's pixels waiting to be sent: they gather while one `object.paint`
/// call is in flight and go out together in the next, so a drag is a few
/// calls, not one per mouse sample.
struct StrokeBatch: Sendable {
    private(set) var pending: [PixelPoint] = []
    private var queued = Set<PixelPoint>()

    var isEmpty: Bool { pending.isEmpty }

    mutating func add(_ points: [PixelPoint]) {
        for p in points where queued.insert(p).inserted { pending.append(p) }
    }

    /// Everything gathered so far, in order; the batch is empty after.
    mutating func take() -> [PixelPoint] {
        defer {
            pending = []
            queued = []
        }
        return pending
    }
}

/// The RGBA of a render, to read the color under the pointer.
struct PixelColors: Sendable, Equatable {
    var width: Int
    var height: Int
    var rgba: [UInt8]

    init(width: Int, height: Int, rgba: [UInt8]) {
        self.width = width
        self.height = height
        self.rgba = rgba
    }

    /// Un-premultiplied sRGB, from the image's own pixels (render scale `scale`).
    init?(cgImage: CGImage, scale: Int) {
        let s = max(scale, 1)
        let w = cgImage.width / s, h = cgImage.height / s
        guard w > 0, h > 0, w * h <= 4_000_000 else { return nil }
        var buf = [UInt8](repeating: 0, count: w * h * 4)
        let ok = buf.withUnsafeMutableBytes { raw -> Bool in
            guard let ctx = CGContext(
                data: raw.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else { return false }
            ctx.interpolationQuality = .none
            ctx.draw(cgImage, in: CGRect(x: 0, y: 0, width: w, height: h))
            return true
        }
        guard ok else { return nil }
        width = w
        height = h
        rgba = buf
    }

    /// "#RRGGBB" of an opaque pixel, nil for a transparent one or outside.
    func hex(_ p: PixelPoint) -> String? {
        guard p.x >= 0, p.y >= 0, p.x < width, p.y < height else { return nil }
        let i = (p.y * width + p.x) * 4
        guard rgba[i + 3] > 0 else { return nil }
        return String(format: "#%02X%02X%02X", rgba[i], rgba[i + 1], rgba[i + 2])
    }
}

/// The site's frame loops (motion.mjs SITE_LOOPS and LOOP_CYCLES).
enum FrameLoop: String, CaseIterable, Identifiable, Sendable {
    case ff, wf, hf
    var id: String { rawValue }
    var frames: Int {
        switch self {
        case .ff: 4
        case .wf: 5
        case .hf: 6
        }
    }
    /// One cycle, in ms.
    var cycle: Int {
        switch self {
        case .ff: 600
        case .wf: 1000
        case .hf: 3000
        }
    }
    var title: String { "\(rawValue) · \(frames) frames in \(cycle >= 1000 ? "\(cycle / 1000) s" : "0.\(cycle / 100) s")" }

    /// How long each frame shows when the lab plays a loop of `count` frames.
    static func step(loop: String?, count: Int) -> Int {
        let n = max(count, 1)
        let cycle = loop.flatMap(FrameLoop.init(rawValue:))?.cycle ?? 200 * n
        return max(cycle / n, 16)
    }
}

/// The New Scene / New Object dialog's rules (the lab's dialogs.ts).
enum NewDocRules {
    static let maxName = 64

    /// engine.mjs isName: lowercase words of letters and digits joined by single hyphens.
    static func isName(_ name: String) -> Bool {
        name.range(of: "^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$", options: .regularExpression) != nil
    }

    /// Why a name can't be used, or nil.
    static func nameProblem(_ name: String, taken: Set<String>) -> String? {
        if name.isEmpty { return "Give it a name, like small-rock." }
        if name.count > maxName { return "That's \(name.count) characters; the most is \(maxName)." }
        if !isName(name) { return "Use lowercase letters and digits, with single hyphens between words, like small-rock." }
        if taken.contains(name) { return "There's already an object or scene called \(name)." }
        return nil
    }

    /// A sprite's width and height are whole numbers from 1 to 64.
    static func sizeProblem(width: String, height: String) -> String? {
        func ok(_ v: String) -> Bool { Int(v.trimmingCharacters(in: .whitespaces)).map { (1...64).contains($0) } ?? false }
        return ok(width) && ok(height) ? nil : "Width and height are whole numbers from 1 to 64."
    }
}

/// The pixels of a tile's top face centered on (0, 0) (iso.mjs tilePixels),
/// and its outline, for the Preview.
enum TileShape {
    static let pixels: [PixelPoint] = {
        var out: [PixelPoint] = []
        let w = Int(StageGeometry.tileW), h = Int(StageGeometry.tileH)
        for y in -h / 2..<h / 2 {
            let half = CGFloat(w / 2) - 2 * abs(CGFloat(y) + 0.5)
            for x in -w / 2..<w / 2 where abs(CGFloat(x) + 0.5) < half { out.append(PixelPoint(x, y)) }
        }
        return out
    }()

    static let outline: [PixelPoint] = {
        let inside = Set(pixels)
        return pixels.filter { p in
            [PixelPoint(p.x - 1, p.y), PixelPoint(p.x + 1, p.y), PixelPoint(p.x, p.y - 1), PixelPoint(p.x, p.y + 1)].contains { !inside.contains($0) }
        }
    }()
}
