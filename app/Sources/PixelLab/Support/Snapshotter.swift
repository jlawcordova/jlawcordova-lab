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
        guard let content = window.contentView else { return false }
        let view = content.superview ?? content
        view.layoutSubtreeIfNeeded()
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return false }
        view.cacheDisplay(in: view.bounds, to: rep)
        guard let png = rep.representation(using: .png, properties: [:]) else { return false }
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try png.write(to: url)
            return true
        } catch {
            return false
        }
    }
}
