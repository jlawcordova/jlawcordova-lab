import SwiftUI

/// ITEMS: the open scene's items in paint order, as a tree of groups (with
/// their class and data-* attributes) and placed items. Selecting a row
/// selects the item on the stage and the other way round. The search field
/// filters it (object, id, class) to a flat list.
struct ItemsTree: View {
    var doc: DocumentModel
    var query: String
    @Environment(AppModel.self) private var model

    var body: some View {
        let rows = visibleRows
        HStack(alignment: .firstTextBaseline) {
            SectionLabel(title: "Items")
            Spacer()
            Text("\(doc.items.count)").font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted.opacity(0.8))
        }
        .padding(.horizontal, 6)
        .padding(.top, 4)
        .padding(.bottom, 2)
        if !doc.itemsLoaded {
            HStack(spacing: 6) {
                ProgressView().controlSize(.mini)
                Text("Loading items…").font(.caption).foregroundStyle(Theme.inkMuted)
            }
            .padding(.horizontal, 6)
        } else if rows.isEmpty {
            Text(query.isEmpty ? "No items yet: place one from the Library." : "No items match")
                .font(.callout).foregroundStyle(Theme.inkMuted).padding(.horizontal, 6).padding(.vertical, 4)
        }
        LazyVStack(alignment: .leading, spacing: 1) {
            ForEach(rows) { row in
                ItemTreeRow(row: row, flat: !query.isEmpty, selected: row.path == doc.selectedPath)
                    .id(row.path)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Items of \(doc.ref.name)")
    }

    private var visibleRows: [TreeRow] {
        if !query.isEmpty {
            return doc.treeRows.filter { row in
                guard case .item(let item) = row.kind else { return false }
                return item.object.lowercased().contains(query) || (item.id?.lowercased().contains(query) ?? false)
                    || (item.class?.lowercased().contains(query) ?? false)
            }
        }
        let collapsed = model.editor.collapsedGroups
        if collapsed.isEmpty { return doc.treeRows }
        return doc.treeRows.filter { row in !row.ancestors.contains { collapsed.contains($0) } }
    }
}

struct ItemTreeRow: View {
    var row: TreeRow
    var flat: Bool
    var selected: Bool
    @Environment(AppModel.self) private var model

    var body: some View {
        let editor = model.editor
        HStack(spacing: 5) {
            if !flat {
                Color.clear.frame(width: CGFloat(row.depth) * 12, height: 1)
            }
            switch row.kind {
            case .group(let group):
                let open = !editor.collapsedGroups.contains(row.path)
                Button {
                    if open { editor.collapsedGroups.insert(row.path) } else { editor.collapsedGroups.remove(row.path) }
                } label: {
                    HStack(spacing: 5) {
                        Image(systemName: "chevron.right")
                            .font(.system(size: 9, weight: .semibold))
                            .rotationEffect(.degrees(open ? 90 : 0))
                            .foregroundStyle(Theme.inkMuted)
                            .frame(width: 10)
                        Image(systemName: "folder")
                            .font(.system(size: 11))
                            .foregroundStyle(Theme.inkMuted)
                        Text(Self.groupTitle(group))
                            .font(.system(size: 11.5))
                            .foregroundStyle(Theme.inkMuted)
                            .lineLimit(1)
                            .truncationMode(.tail)
                        Spacer(minLength: 2)
                        Text("\(group.items)").font(Theme.monoFont(9.5)).foregroundStyle(Theme.inkMuted.opacity(0.7))
                    }
                    .frame(height: 20)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Group \(row.path), \(Self.groupTitle(group)), \(group.items) items, \(open ? "expanded" : "collapsed")")
            case .item(let item):
                Button {
                    editor.select(item.path, reveal: false)
                    editor.focusStage()
                } label: {
                    HStack(spacing: 5) {
                        Image(systemName: item.at.isTile ? "cube" : "mappin")
                            .font(.system(size: 10.5))
                            .foregroundStyle(selected ? Theme.accent : Theme.inkMuted)
                            .frame(width: 14)
                        Text(item.object)
                            .font(.system(size: 12, weight: selected ? .semibold : .regular))
                            .foregroundStyle(Theme.ink)
                            .lineLimit(1)
                            .truncationMode(.middle)
                        if let id = item.id {
                            Text("#\(id)").font(Theme.monoFont(10)).foregroundStyle(Theme.inkMuted).lineLimit(1)
                        }
                        Spacer(minLength: 2)
                        Text(item.at.short).font(Theme.monoFont(9.5)).foregroundStyle(Theme.inkMuted.opacity(0.8)).lineLimit(1)
                    }
                    .frame(height: 22)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("\(item.object)\(item.id.map { " \($0)" } ?? ""), \(item.at.spoken)")
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .padding(.horizontal, 6)
        .background(
            RoundedRectangle(cornerRadius: 5, style: .continuous)
                .fill(selected ? Theme.selection : .clear)
        )
    }

    /// ".enter · ground" from class and data-* attributes.
    static func groupTitle(_ g: SceneGroupInfo) -> String {
        var parts: [String] = []
        if let c = g.attrs["class"], !c.isEmpty { parts.append("." + c.replacingOccurrences(of: " ", with: ".")) }
        for (k, v) in g.attrs.sorted(by: { $0.key < $1.key }) where k.hasPrefix("data-") && !["data-col", "data-row", "data-x", "data-y"].contains(k) {
            parts.append(v.isEmpty ? k.dropFirst(5).description : "\(k.dropFirst(5))=\(v)")
        }
        return parts.isEmpty ? "group" : parts.joined(separator: " ")
    }
}
