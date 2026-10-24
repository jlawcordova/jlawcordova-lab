import Foundation

/// The Library's categories as pure functions (docs/intents/2026-10-object-libraries/spec.md, R1, R2, R4–R8, D7):
/// the tabs, the sections of a tab (headings with their objects), the order the keyboard walks them, and the
/// parsing and checking of a category path typed in the inspector. The views and the tests use these.
enum LibraryCategories {
    static let all = "All"
    static let others = "Others"
    /// R2: a name is 1–32 characters.
    static let maxName = 32

    /// One run of objects under a heading (nil: no heading).
    struct Section: Equatable {
        var heading: String?
        var objects: [ObjectSummary]
    }

    // MARK: Paths (R2, R4)

    /// "Nature › Trees", the way a category is read and typed.
    static func text(_ path: [String]) -> String { path.joined(separator: " › ") }

    /// A typed path ("Nature / Trees", "Nature › Trees") as names: split at › and /, each trimmed.
    /// Empty (or only blanks) is nil, which means None.
    static func parse(_ typed: String) -> [String]? {
        let names = typed.split(whereSeparator: { $0 == "›" || $0 == "/" }).map { $0.trimmingCharacters(in: .whitespaces) }
        if names.allSatisfy(\.isEmpty) { return nil }
        return names
    }

    /// Why `names` can't be a category (R2), or nil when they can: the program's own words.
    static func problem(_ names: [String]) -> String? {
        if names.isEmpty { return "A category needs at least one name" }
        for (i, name) in names.enumerated() {
            let count = name.count
            if count == 0 || count > maxName { return "“\(name)” is \(count) characters; a name is 1 to \(maxName)" }
            if let bad = name.first(where: { !($0.isLetter || $0.isNumber || $0 == " " || $0 == "&" || $0 == "'" || $0 == "-") }) {
                return "“\(name)” has “\(bad)”; use letters, digits, spaces, & ' and -"
            }
            if name.hasPrefix(" ") || name.hasSuffix(" ") || name.contains("  ") { return "“\(name)” has a leading, trailing or doubled space" }
            if i == 0, name.lowercased() == all.lowercased() || name.lowercased() == others.lowercased() {
                return "“\(name)” is a tab of the Library, so it can't be the outermost name"
            }
        }
        return nil
    }

    /// Names compare without regard to case (R2).
    static func key(_ path: [String]) -> String { path.map { $0.lowercased() }.joined(separator: "\u{1}") }

    /// Each distinct category (every path and every path above it) with the spelling of the alphabetically
    /// first object that uses it (R2), by key.
    static func spellings(_ objects: [ObjectSummary]) -> [String: [String]] {
        var spelled: [String: [String]] = [:]
        for object in objects.sorted(by: { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }) {
            guard let path = object.category, !path.isEmpty else { continue }
            for n in 1...path.count {
                let prefix = Array(path.prefix(n))
                let k = key(prefix)
                if spelled[k] == nil { spelled[k] = prefix }
            }
        }
        return spelled
    }

    /// The path as spelled by `spellings`.
    static func shown(_ path: [String], _ spelled: [String: [String]]) -> [String] {
        (1...max(path.count, 1)).compactMap { spelled[key(Array(path.prefix($0)))]?.last }
    }

    /// Every category in the document, as spelled, in alphabetical order (the inspector's menu).
    static func categories(_ objects: [ObjectSummary]) -> [[String]] {
        sortedPaths(Array(spellings(objects).values))
    }

    static func sortedPaths(_ paths: [[String]]) -> [[String]] {
        paths.sorted { a, b in
            for (x, y) in zip(a, b) {
                let c = x.localizedCaseInsensitiveCompare(y)
                if c != .orderedSame { return c == .orderedAscending }
            }
            return a.count < b.count
        }
    }

    // MARK: Tabs (R5)

    /// All, one tab per outermost category in alphabetical order, and Others when an object has none.
    /// Empty when no object has a category: the row doesn't show.
    static func tabs(_ objects: [ObjectSummary]) -> [String] {
        guard objects.contains(where: { !($0.category ?? []).isEmpty }) else { return [] }
        let spelled = spellings(objects)
        let outer = spelled.values.filter { $0.count == 1 }.map { $0[0] }
            .sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
        var tabs = [all] + outer
        if objects.contains(where: { ($0.category ?? []).isEmpty }) { tabs.append(others) }
        return tabs
    }

    /// The tab `chosen` is among `tabs` (without regard to case), else All.
    static func resolved(_ chosen: String?, in tabs: [String]) -> String {
        guard let chosen, let tab = tabs.first(where: { $0.lowercased() == chosen.lowercased() }) else { return all }
        return tab
    }

    // MARK: Sections (R6, R7)

    /// The sections of a tab, for a search `query` (R7): the objects whose name holds the query, with their headings,
    /// and no heading that has no objects.
    static func sections(_ objects: [ObjectSummary], tab: String, query: String = "") -> [Section] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        let kept = (q.isEmpty ? objects : objects.filter { $0.name.lowercased().contains(q) })
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
        let spelled = spellings(objects)
        let categorized = kept.filter { !($0.category ?? []).isEmpty }
        let uncategorized = kept.filter { ($0.category ?? []).isEmpty }
        let anyCategory = objects.contains { !($0.category ?? []).isEmpty }
        guard anyCategory else { return kept.isEmpty ? [] : [Section(heading: nil, objects: kept)] }

        func grouped(_ list: [ObjectSummary], path: (ObjectSummary) -> [String]) -> [(path: [String], objects: [ObjectSummary])] {
            var groups: [String: (path: [String], objects: [ObjectSummary])] = [:]
            for object in list {
                let p = path(object)
                let k = key(p)
                if groups[k] == nil { groups[k] = (p, []) }
                groups[k]?.objects.append(object)
            }
            let order = sortedPaths(groups.values.map(\.path))
            return order.compactMap { groups[key($0)] }
        }

        if tab.lowercased() == others.lowercased() {
            return uncategorized.isEmpty ? [] : [Section(heading: nil, objects: uncategorized)]
        }
        if tab.lowercased() == all.lowercased() {
            var out = grouped(categorized) { shown($0.category ?? [], spelled) }.map { Section(heading: text($0.path), objects: $0.objects) }
            if !uncategorized.isEmpty { out.append(Section(heading: others, objects: uncategorized)) }
            return out
        }
        // A category's tab: the objects in it first, then each subcategory under its path below the tab.
        let inTab = categorized.filter { $0.category?.first?.lowercased() == tab.lowercased() }
        let groups = grouped(inTab) { Array(shown($0.category ?? [], spelled).dropFirst()) }
        return groups.map { Section(heading: $0.path.isEmpty ? nil : text($0.path), objects: $0.objects) }
    }

    /// Every object of the sections, in the order the keyboard walks them (R8).
    static func flattened(_ sections: [Section]) -> [ObjectSummary] { sections.flatMap(\.objects) }

    // MARK: The cursor (R8)

    enum Direction { case left, right, up, down }

    /// Where an arrow key moves the cursor through sections laid out in rows of `columns` cells, each section
    /// starting a new row. `counts` is the number of objects in each section; `index` is the cursor in the
    /// flattened list. ←/→ step through reading order across headings. ↓ from a group's last row goes to the next
    /// group's first row, in the same column or its last cell. Nil: there is no cell that way (↑ from the first row).
    static func move(_ direction: Direction, from index: Int, counts: [Int], columns: Int) -> Int? {
        let total = counts.reduce(0, +)
        guard total > 0, columns > 0 else { return nil }
        let i = min(max(index, 0), total - 1)
        switch direction {
        case .left: return i > 0 ? i - 1 : 0
        case .right: return min(i + 1, total - 1)
        case .up, .down:
            // Which section the cursor is in, where it starts, and its row and column in it.
            var start = 0
            var s = 0
            while s < counts.count, i >= start + counts[s] { start += counts[s]; s += 1 }
            let p = i - start
            let row = p / columns, col = p % columns
            let rows = (counts[s] + columns - 1) / columns
            func cell(section: Int, sectionStart: Int, row: Int) -> Int {
                let first = row * columns
                let last = min(first + columns, counts[section]) - 1
                return sectionStart + min(first + col, last)
            }
            if direction == .down {
                if row + 1 < rows { return cell(section: s, sectionStart: start, row: row + 1) }
                var next = s + 1, nextStart = start + counts[s]
                while next < counts.count, counts[next] == 0 { nextStart += counts[next]; next += 1 }
                guard next < counts.count else { return nil }
                return cell(section: next, sectionStart: nextStart, row: 0)
            }
            if row > 0 { return cell(section: s, sectionStart: start, row: row - 1) }
            var prev = s - 1
            while prev >= 0, counts[prev] == 0 { prev -= 1 }
            guard prev >= 0 else { return nil }
            let prevStart = counts[..<prev].reduce(0, +)
            let prevRows = (counts[prev] + columns - 1) / columns
            return cell(section: prev, sectionStart: prevStart, row: prevRows - 1)
        }
    }

    /// The next (+1) or previous (-1) tab, wrapping nowhere (⌃Tab, ⌃⇧Tab, ← →).
    static func step(_ tab: String, by delta: Int, in tabs: [String]) -> String {
        guard let i = tabs.firstIndex(where: { $0.lowercased() == tab.lowercased() }) else { return tabs.first ?? all }
        return tabs[min(max(i + delta, 0), tabs.count - 1)]
    }
}
