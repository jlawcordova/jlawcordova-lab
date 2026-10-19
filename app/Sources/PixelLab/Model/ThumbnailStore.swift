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
    @ObservationIgnored var connection: ServerConnection?
    @ObservationIgnored private var queue: [String] = []
    @ObservationIgnored private var visible: Set<String> = []
    @ObservationIgnored private var failed: Set<String> = []
    private(set) var inFlight: String?

    var isIdle: Bool { queue.isEmpty && inFlight == nil }

    func appeared(_ name: String) {
        visible.insert(name)
        request(name)
    }

    func disappeared(_ name: String) {
        visible.remove(name)
        queue.removeAll { $0 == name }
    }

    func request(_ name: String) {
        guard images[name] == nil, !failed.contains(name), inFlight != name, !queue.contains(name) else { return }
        queue.append(name)
        pump()
    }

    func invalidate(_ names: some Sequence<String>) {
        for name in names {
            images[name] = nil
            failed.remove(name)
            if visible.contains(name) { request(name) }
        }
    }

    func invalidateAll() {
        let names = Array(images.keys) + Array(failed)
        images = [:]
        failed = []
        for name in names where visible.contains(name) { request(name) }
        for name in visible { request(name) }
    }

    func reset(connection: ServerConnection?) {
        self.connection = connection
        images = [:]
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
                if let image = PixelImage(png: r.png, renderScale: r.scale) {
                    images[name] = image
                }
            } catch {
                failed.insert(name)
            }
            if inFlight == name { inFlight = nil }
            pump()
        }
    }
}
