import CoreGraphics
import Foundation
import ImageIO
import SwiftUI

/// A decoded render: the PNG's CGImage and its size in art pixels.
///
/// To stay crisp everywhere (on screen and in `cacheDisplay` snapshots, which
/// ignore a layer's nearest-neighbor filter) the stage never asks the GPU to
/// magnify: `view` draws a copy already scaled, nearest-neighbor, to the exact
/// device pixels it covers, cached per factor.
struct PixelImage: @unchecked Sendable {
    let cgImage: CGImage
    /// Art pixels (the PNG's pixels divided by its render scale).
    let size: CGSize
    private let cache = ScaledCache()

    init?(png: Data, renderScale: Int) {
        guard let source = CGImageSourceCreateWithData(png as CFData, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
        else { return nil }
        cgImage = image
        let s = CGFloat(max(renderScale, 1))
        size = CGSize(width: CGFloat(image.width) / s, height: CGFloat(image.height) / s)
    }

    /// The art at `pointsPerPixel` points per art pixel, nearest-neighbor, on a
    /// display with `displayScale` device pixels per point.
    func view(pointsPerPixel: CGFloat, displayScale: CGFloat) -> some View {
        let width = size.width * pointsPerPixel, height = size.height * pointsPerPixel
        let device = Int((pointsPerPixel * displayScale).rounded())
        let image: Image
        if device >= 1, abs(CGFloat(device) - pointsPerPixel * displayScale) < 0.01, let scaled = scaled(devicePixelsPerPixel: device) {
            image = Image(decorative: scaled, scale: displayScale)
        } else {
            image = Image(decorative: cgImage, scale: 1)
        }
        return image
            .interpolation(.none)
            .antialiased(false)
            .resizable()
            .frame(width: width, height: height)
    }

    /// A copy with `n` device pixels per art pixel (nil if it would be huge).
    func scaled(devicePixelsPerPixel n: Int) -> CGImage? {
        if let hit = cache.get(n) { return hit }
        let w = Int(size.width.rounded()) * n, h = Int(size.height.rounded()) * n
        guard w > 0, h > 0, w <= 12_000, h <= 12_000, w * h <= 60_000_000 else { return nil }
        if w == cgImage.width, h == cgImage.height { return cgImage }
        guard let ctx = CGContext(
            data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }
        ctx.interpolationQuality = .none
        ctx.setShouldAntialias(false)
        ctx.draw(cgImage, in: CGRect(x: 0, y: 0, width: w, height: h))
        let out = ctx.makeImage()
        if let out { cache.set(n, out) }
        return out
    }
}

/// Scaled copies of one image, a few factors at most.
private final class ScaledCache: @unchecked Sendable {
    private var images: [Int: CGImage] = [:]
    private var order: [Int] = []
    private let lock = NSLock()

    func get(_ n: Int) -> CGImage? {
        lock.lock(); defer { lock.unlock() }
        return images[n]
    }

    func set(_ n: Int, _ image: CGImage) {
        lock.lock(); defer { lock.unlock() }
        images[n] = image
        order.removeAll { $0 == n }
        order.append(n)
        while order.count > 3 { images[order.removeFirst()] = nil }
    }
}
