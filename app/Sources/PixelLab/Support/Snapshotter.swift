import AppKit

/// Self-snapshot for verification, with no screen-recording permission:
/// `PIXELLAB_SNAPSHOT=/path/out.png` (plus optional `PIXELLAB_SNAPSHOT_DOC`,
/// `PIXELLAB_SNAPSHOT_APPEARANCE`, `PIXELLAB_SNAPSHOT_SIZE=1440x900`). Waits
/// until the doc is rendered and checked and the visible thumbnails are in,
/// draws the main window (toolbar included) with `cacheDisplay`, writes the PNG
/// and quits. Exit 0 on success, 2 if it timed out (it still writes what it has).
@MainActor
enum Snapshotter {
    static func start(_ request: SnapshotRequest, model: AppModel) {
        Task {
            let started = Date()
            var steadySince: Date?
            var sized = false
            var timedOut = false
            while true {
                try? await Task.sleep(for: .milliseconds(100))
                if !sized, let window = mainWindow() {
                    window.setFrame(NSRect(origin: window.frame.origin, size: request.size), display: true)
                    window.center()
                    sized = true
                }
                if sized, isSettled(request, model: model) {
                    steadySince = steadySince ?? Date()
                    if Date().timeIntervalSince(steadySince!) > 0.8 { break }
                } else {
                    steadySince = nil
                }
                if Date().timeIntervalSince(started) > 60 {
                    timedOut = true
                    break
                }
            }
            var ok = false
            if let window = mainWindow() {
                ok = capture(window, to: request.output)
            }
            FileHandle.standardError.write(Data("snapshot: \(ok ? "wrote" : "FAILED to write") \(request.output.path)\(timedOut ? " (timed out)" : "")\n".utf8))
            await model.shutdown()
            exit(ok && !timedOut ? 0 : 2)
        }
    }

    private static func isSettled(_ request: SnapshotRequest, model: AppModel) -> Bool {
        switch model.phase {
        case .starting: return false
        case .setup: return true
        case .ready:
            guard let doc = model.document else { return false }
            if let want = request.doc, doc.ref != want { return false }
            return doc.render != nil && !doc.rendering && doc.problems != nil && !doc.checking && model.thumbnails.isIdle
                && !model.objectEditor.busy
                && model.stageViewport.width > 0
        }
    }

    static func mainWindow() -> NSWindow? {
        NSApp.windows.first { $0.identifier?.rawValue.hasPrefix(WindowID.main) == true && $0.isVisible }
            ?? NSApp.windows.filter { $0.isVisible && $0.contentView != nil && !($0 is NSPanel) }.max { $0.frame.width < $1.frame.width }
    }

    /// Draws the window's frame view (content and toolbar) into a bitmap at the
    /// backing scale and writes it as PNG.
    static func capture(_ window: NSWindow, to url: URL) -> Bool {
        guard let rep = bitmap(of: window) else { return false }
        var out = rep
        // A sheet (Build art) is its own window: draw it over the main one where it sits.
        if let sheet = window.attachedSheet, let sheetRep = bitmap(of: sheet), let composite = compose(rep, window.frame, sheetRep, sheet.frame) {
            out = composite
        }
        guard let png = out.representation(using: .png, properties: [:]) else { return false }
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try png.write(to: url)
            return true
        } catch {
            return false
        }
    }

    static func bitmap(of window: NSWindow) -> NSBitmapImageRep? {
        guard let content = window.contentView else { return nil }
        let view = content.superview ?? content
        view.layoutSubtreeIfNeeded()
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        return rep
    }

    /// The main window's bitmap with the sheet's drawn over it at its place.
    static func compose(_ base: NSBitmapImageRep, _ baseFrame: NSRect, _ top: NSBitmapImageRep, _ topFrame: NSRect) -> NSBitmapImageRep? {
        let scale = CGFloat(base.pixelsWide) / max(baseFrame.width, 1)
        guard let baseImage = base.cgImage, let topImage = top.cgImage,
              let ctx = CGContext(data: nil, width: base.pixelsWide, height: base.pixelsHigh, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        ctx.draw(baseImage, in: CGRect(x: 0, y: 0, width: base.pixelsWide, height: base.pixelsHigh))
        // Dim the window behind the sheet a little, as the screen shows it.
        ctx.setFillColor(CGColor(gray: 0, alpha: 0.18))
        ctx.fill(CGRect(x: 0, y: 0, width: base.pixelsWide, height: base.pixelsHigh))
        // Both frames are in screen points with y up, as is the context.
        let r = CGRect(x: (topFrame.minX - baseFrame.minX) * scale, y: (topFrame.minY - baseFrame.minY) * scale,
                       width: topFrame.width * scale, height: topFrame.height * scale)
        ctx.setShadow(offset: CGSize(width: 0, height: -6 * scale), blur: 24 * scale, color: CGColor(gray: 0, alpha: 0.4))
        ctx.draw(topImage, in: r)
        guard let out = ctx.makeImage() else { return nil }
        return NSBitmapImageRep(cgImage: out)
    }
}
