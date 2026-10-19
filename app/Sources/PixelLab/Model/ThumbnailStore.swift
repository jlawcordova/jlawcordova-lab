import AppKit
import Observation

/// Library thumbnails: each object rendered with `object.render` at scale 2,
/// cached in memory, fetched one at a time (so a scene render never waits
/// behind more than one), in the order the grid asks for them. Dropped when
/// `files.changed` names the object (or the palette).
@MainActor @Observable
final class ThumbnailStore {
    static let renderScale = 2

    private(set) var images: [String: PixelImage] = [:]
    /// The same renders with their geometry and alpha, for ghosts and hit-testing.
    private(set) var arts: [String: ObjectArt] = [:]
    @ObservationIgnored var connection: ServerConnection?
    @ObservationIgnored private var queue: [String] = []
    @ObservationIgnored private var visible: Set<String> = []
    @ObservationIgnored private var failed: Set<String> = []
    /// Objects the open scene uses: kept in the queue when the grid scrolls.
    @ObservationIgnored private var pinned: Set<String> = []
    private(set) var inFlight: String?

    var isIdle: Bool { queue.isEmpty && inFlight == nil }

    func appeared(_ name: String) {
        visible.insert(name)
        request(name)
    }

    func disappeared(_ name: String) {
        visible.remove(name)
        if !pinned.contains(name) { queue.removeAll { $0 == name } }
    }

    func request(_ name: String) {
        guard images[name] == nil, !failed.contains(name), inFlight != name, !queue.contains(name) else { return }
        queue.append(name)
        pump()
    }

    func invalidate(_ names: some Sequence<String>) {
        for name in names {
            images[name] = nil
            arts[name] = nil
            failed.remove(name)
            if visible.contains(name) { request(name) }
        }
    }

    func invalidateAll() {
        let names = Array(images.keys) + Array(failed)
        images = [:]
        arts = [:]
        failed = []
        for name in names where visible.contains(name) { request(name) }
        for name in visible { request(name) }
    }

    func reset(connection: ServerConnection?) {
        self.connection = connection
        images = [:]
        arts = [:]
        failed = []
        queue = []
        inFlight = nil
        for name in visible { request(name) }
    }

    private func pump() {
        guard inFlight == nil, let connection, !queue.isEmpty else { return }
        let name = queue.removeFirst()
        inFlight = name
        Task {
            do {
                let r: RenderResult = try await connection.call(
                    "object.render", ["name": .string(name), "scale": JSONValue(Self.renderScale)])
                store(name, r)
            } catch {
                failed.insert(name)
            }
            if inFlight == name { inFlight = nil }
            pump()
        }
    }

    private func store(_ name: String, _ r: RenderResult) {
        guard let image = PixelImage(png: r.png, renderScale: r.scale) else { return }
        images[name] = image
        let viewBox = CGRect(x: r.viewBox[0], y: r.viewBox[1], width: r.viewBox[2], height: r.viewBox[3])
        var box: CGRect?
        if let b = r.items.first?.bounds, b.count == 4 {
            let s = CGFloat(max(r.scale, 1))
            box = CGRect(x: viewBox.minX + b[0] / s, y: viewBox.minY + b[1] / s, width: b[2] / s, height: b[3] / s)
        }
        arts[name] = ObjectArt(image: image, viewBox: viewBox, box: box, scale: r.scale, mask: AlphaMask(cgImage: image.cgImage))
    }

    /// Renders objects the open scene uses (for hit-testing and drag ghosts),
    /// behind whatever the library grid asked for.
    func prefetch(_ names: some Sequence<String>) {
        pinned = Set(names)
        for name in Set(names).sorted() where images[name] == nil && !failed.contains(name) && inFlight != name && !queue.contains(name) {
            queue.append(name)
        }
        pump()
    }

    /// An object's art now, rendering it first if need be.
    func art(_ name: String) async -> ObjectArt? {
        if let a = arts[name] { return a }
        guard let connection, !failed.contains(name) else { return nil }
        do {
            let r: RenderResult = try await connection.call(
                "object.render", ["name": .string(name), "scale": JSONValue(Self.renderScale)])
            store(name, r)
        } catch {
            failed.insert(name)
        }
        return arts[name]
    }
}
