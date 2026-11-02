import Foundation

// Derived navigation state only. Manuscript text remains the source of truth.
struct OutlineIndex {
    struct Key: Hashable {
        let kind: String
        let title: String
        let level: Int
        let occurrence: Int
    }
    struct Row: Identifiable {
        let heading: ManuscriptHeading
        let key: Key
        let ancestors: [Int]
        var hasChildren = false
        var id: Int { heading.line }
        var depth: Int { ancestors.count }
    }
    private struct HeadingName: Hashable { let kind: String; let title: String }
    var rows: [Row] = []
    var keys: [Key] { rows.map(\.key) }
    var expandableKeys: Set<Key> { Set(rows.filter(\.hasChildren).map(\.key)) }

    init(_ headings: [ManuscriptHeading] = []) {
        var ancestors: [Int] = []
        var occurrences: [HeadingName: Int] = [:]
        for heading in headings {
            while let last = ancestors.last, rows[last].heading.level >= heading.level {
                ancestors.removeLast()
            }
            let name = HeadingName(kind: heading.kind.rawValue, title: heading.heading)
            let occurrence = occurrences[name, default: 0]
            occurrences[name] = occurrence + 1
            let key = Key(kind: name.kind, title: name.title, level: heading.level, occurrence: occurrence)
            if let parent = ancestors.last { rows[parent].hasChildren = true }
            rows.append(Row(heading: heading, key: key, ancestors: ancestors))
            ancestors.append(rows.count - 1)
        }
    }

    func visibleRows(collapsed: Set<Key>, query: String, hiddenKinds: Set<ElementKind> = [], additionalMatches: Set<Int> = []) -> [Row] {
        let allowed = Set(rows.indices.filter { !hiddenKinds.contains(rows[$0].heading.kind) })
        var included = allowed
        if !query.isEmpty {
            included = []
            for index in allowed where additionalMatches.contains(rows[index].heading.line) || rows[index].heading.heading.localizedCaseInsensitiveContains(query) {
                included.insert(index)
                included.formUnion(rows[index].ancestors.filter { allowed.contains($0) })
            }
        }
        let parents = Set(included.flatMap { rows[$0].ancestors.filter { included.contains($0) } })
        return rows.indices.compactMap { index in
            guard included.contains(index) else { return nil }
            let row = rows[index]
            let ancestors = row.ancestors.filter { included.contains($0) }
            guard !query.isEmpty || !ancestors.contains(where: { collapsed.contains(rows[$0].key) }) else { return nil }
            return Row(heading: row.heading, key: row.key, ancestors: ancestors, hasChildren: parents.contains(index))
        }
    }
}
