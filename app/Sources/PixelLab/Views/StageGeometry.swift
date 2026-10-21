import CoreGraphics

/// How the stage maps between its three spaces:
///
/// - **view points**: the stage's scrollable content, origin at its top-left.
/// - **scene pixels** (art pixels): the engine's coordinates; the render's
///   PNG covers `viewBox`, and `zoom` points draw one art pixel.
/// - **tiles** `[col, row, level]`: the iso grid (32×16 tiles, 16-pixel levels),
///   with tile [0, 0, 0]'s top-face center at `origin` in scene pixels
///   (the site's `iso.mjs` tileToPx and pxToTile).
///
/// Slice 4 uses it for hit-testing (view → scene, against item bounds) and
/// placing (view → tile).
struct StageGeometry: Equatable {
    static let tileW: CGFloat = 32
    static let tileH: CGFloat = 16
    static let levelH: CGFloat = 16

    /// Scene pixels the art covers.
    var viewBox: CGRect
    /// Points per art pixel (a whole number).
    var zoom: CGFloat
    /// Where the art's top-left (viewBox.origin) sits, in view points.
    var artOrigin: CGPoint
    /// The scrollable content's size, in view points.
    var contentSize: CGSize
    /// Tile [0, 0, 0]'s top-face center in scene pixels (scenes only).
    var tileOrigin: CGPoint?

    /// Centers the art in the content, which is at least the viewport.
    /// When the art fits the viewport the content is the viewport (no scrolling);
    /// when it doesn't, the content is the art plus `margin` on each side.
    init(viewBox: CGRect, zoom: Int, viewport: CGSize, margin: CGFloat, tileOrigin: CGPoint?) {
        self.viewBox = viewBox
        self.zoom = CGFloat(max(zoom, 1))
        self.tileOrigin = tileOrigin
        let art = CGSize(width: viewBox.width * self.zoom, height: viewBox.height * self.zoom)
        contentSize = CGSize(
            width: art.width <= viewport.width ? viewport.width : art.width + 2 * margin,
            height: art.height <= viewport.height ? viewport.height : art.height + 2 * margin
        )
        // Whole points, so every art pixel lands on whole device pixels.
        artOrigin = CGPoint(x: ((contentSize.width - art.width) / 2).rounded(.down), y: ((contentSize.height - art.height) / 2).rounded(.down))
    }

    /// A scene's stage (docs/intents/2026-10-scene-canvas/spec.md, R9): the
    /// render covers a workspace `window` of scene pixels, wider than the art on
    /// every side, so the content has no edge to stop at. It is at least the
    /// viewport, and centered in it when smaller.
    init(window: CGRect, zoom: Int, viewport: CGSize, tileOrigin: CGPoint?) {
        viewBox = window
        self.zoom = CGFloat(max(zoom, 1))
        self.tileOrigin = tileOrigin
        let art = CGSize(width: window.width * self.zoom, height: window.height * self.zoom)
        contentSize = CGSize(width: max(art.width, viewport.width), height: max(art.height, viewport.height))
        artOrigin = CGPoint(x: ((contentSize.width - art.width) / 2).rounded(.down), y: ((contentSize.height - art.height) / 2).rounded(.down))
    }

    var artRect: CGRect {
        CGRect(origin: artOrigin, size: CGSize(width: viewBox.width * zoom, height: viewBox.height * zoom))
    }

    // MARK: view ↔ scene

    /// A view point → scene pixels (fractional).
    func scenePoint(fromView p: CGPoint) -> CGPoint {
        CGPoint(x: viewBox.minX + (p.x - artOrigin.x) / zoom, y: viewBox.minY + (p.y - artOrigin.y) / zoom)
    }

    /// Scene pixels → a view point.
    func viewPoint(fromScene s: CGPoint) -> CGPoint {
        CGPoint(x: artOrigin.x + (s.x - viewBox.minX) * zoom, y: artOrigin.y + (s.y - viewBox.minY) * zoom)
    }

    /// The whole scene pixel under a view point.
    func scenePixel(fromView p: CGPoint) -> (x: Int, y: Int) {
        let s = scenePoint(fromView: p)
        return (Int(floor(s.x)), Int(floor(s.y)))
    }

    /// A rect in scene pixels → view points.
    func viewRect(fromScene r: CGRect) -> CGRect {
        CGRect(origin: viewPoint(fromScene: r.origin), size: CGSize(width: r.width * zoom, height: r.height * zoom))
    }

    /// Render bounds `[x, y, w, h]` in output pixels at `scale` → view points.
    func viewRect(fromOutputBounds b: [Double], scale: Int) -> CGRect? {
        guard b.count == 4 else { return nil }
        let s = CGFloat(scale)
        return viewRect(fromScene: CGRect(x: viewBox.minX + b[0] / s, y: viewBox.minY + b[1] / s, width: b[2] / s, height: b[3] / s))
    }

    // MARK: tiles

    /// The top-face center of a tile, in scene pixels (iso.mjs tileToPx).
    static func tileToScene(col: Int, row: Int, level: Int, origin: CGPoint) -> CGPoint {
        CGPoint(
            x: CGFloat(col - row) * (tileW / 2) + origin.x,
            y: CGFloat(col + row) * (tileH / 2) - CGFloat(level) * levelH + origin.y
        )
    }

    /// The tile at `level` whose top face holds the scene pixel (iso.mjs pxToTile).
    static func sceneToTile(x: Int, y: Int, level: Int, origin: CGPoint) -> (col: Int, row: Int, level: Int) {
        let u = (CGFloat(x) + 0.5 - origin.x) / tileW
        let v = (CGFloat(y) + 0.5 - origin.y + CGFloat(level) * levelH) / tileH
        // JS Math.round rounds .5 up; floor(x + 0.5) matches it.
        return (Int(floor(v + u + 0.5)), Int(floor(v - u + 0.5)), level)
    }

    /// The tile under a view point at `level` (nil without a tile origin).
    func tile(atView p: CGPoint, level: Int = 0) -> (col: Int, row: Int, level: Int)? {
        guard let tileOrigin else { return nil }
        let px = scenePixel(fromView: p)
        return Self.sceneToTile(x: px.x, y: px.y, level: level, origin: tileOrigin)
    }

    /// A tile's top-face center in view points.
    func viewPoint(ofTile col: Int, _ row: Int, _ level: Int) -> CGPoint? {
        guard let tileOrigin else { return nil }
        return viewPoint(fromScene: Self.tileToScene(col: col, row: row, level: level, origin: tileOrigin))
    }

    /// The four corners (top, right, bottom, left) of a tile's top face, in view points.
    func tileDiamond(_ col: Int, _ row: Int, _ level: Int) -> [CGPoint]? {
        guard let c = viewPoint(ofTile: col, row, level) else { return nil }
        let hw = Self.tileW / 2 * zoom, hh = Self.tileH / 2 * zoom
        return [CGPoint(x: c.x, y: c.y - hh), CGPoint(x: c.x + hw, y: c.y), CGPoint(x: c.x, y: c.y + hh), CGPoint(x: c.x - hw, y: c.y)]
    }

    /// The level-0 lattice lines crossing `rect` (view points), as segments.
    /// Tile edges lie on col = k + ½ (slope −½ in scene pixels) and
    /// row = k + ½ (slope +½), through the corners of each top face.
    func gridSegments(in rect: CGRect) -> [(CGPoint, CGPoint)] {
        guard let o = tileOrigin else { return [] }
        let a = scenePoint(fromView: rect.origin)
        let b = scenePoint(fromView: CGPoint(x: rect.maxX, y: rect.maxY))
        var out: [(CGPoint, CGPoint)] = []
        // y = o.y + 8 + 16k ± (x − o.x)/2  ⇔  k = (y − o.y − 8 ∓ (x − o.x)/2) / 16
        for sign in [CGFloat(1), -1] {
            let corners = [CGPoint(x: a.x, y: a.y), CGPoint(x: b.x, y: a.y), CGPoint(x: a.x, y: b.y), CGPoint(x: b.x, y: b.y)]
            let ks = corners.map { ($0.y - o.y - Self.tileH / 2 - sign * ($0.x - o.x) / 2) / Self.tileH }
            guard let kmin = ks.min(), let kmax = ks.max() else { continue }
            for k in Int(floor(kmin))...Int(ceil(kmax)) {
                let y0 = o.y + Self.tileH / 2 + Self.tileH * CGFloat(k) + sign * (a.x - o.x) / 2
                let y1 = o.y + Self.tileH / 2 + Self.tileH * CGFloat(k) + sign * (b.x - o.x) / 2
                out.append((viewPoint(fromScene: CGPoint(x: a.x, y: y0)), viewPoint(fromScene: CGPoint(x: b.x, y: y1))))
            }
        }
        return out
    }
}
