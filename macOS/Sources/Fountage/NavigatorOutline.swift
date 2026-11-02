import Foundation

/// Combine point annotations with structural rows without making annotations
/// movable source blocks or conflating two markers on the same source line.
struct NavigatorOutline {
    struct Entry: Identifiable {
        let row: OutlineIndex.Row?
        let annotation: ManuscriptNote?
        let kind: ElementKind
        var marker: ManuscriptNote? { kind == .marker ? annotation : nil }
        var note: ManuscriptNote? { kind == .note ? annotation : nil }
        var todo: ManuscriptTodo? = nil
        let depth: Int
        var id: Int { row?.heading.line ?? -(annotation!.start + 1) }
        var line: Int { row?.heading.line ?? annotation!.line }
    }
    static func entries(index: OutlineIndex, markers: [ManuscriptNote], notes: [ManuscriptNote] = [], todos: [ManuscriptTodo] = [], collapsed: Set<OutlineIndex.Key>, query: String, hiddenKinds: Set<ElementKind>) -> [Entry] {
        struct Annotation {
            let value: ManuscriptNote
            let kind: ElementKind
            var todo: ManuscriptTodo? = nil
            var title: String { value.text.isEmpty ? (kind == .note ? "Empty note" : kind == .todo ? "Untitled to-do" : "Marker") : value.text }
        }
        let annotations = markers.map { Annotation(value: $0, kind: .marker) } + notes.map { Annotation(value: $0, kind: .note) } + todos.map { Annotation(value: ManuscriptNote(line: $0.line, start: $0.start, length: $0.length, text: $0.text), kind: .todo, todo: $0) }
        func parents(of marker: ManuscriptNote) -> [Int] {
            guard let owner = index.rows.indices.last(where: {
                index.rows[$0].heading.line <= marker.line && ![ElementKind.synopsis, .pageBreak].contains(index.rows[$0].heading.kind)
            }) else { return [] }
            return index.rows[owner].ancestors + [owner]
        }
        let matches = annotations.filter { !hiddenKinds.contains($0.kind) && (query.isEmpty || $0.title.localizedCaseInsensitiveContains(query)) }
        let matchedLines = Set(matches.flatMap { parents(of: $0.value).map { index.rows[$0].heading.line } })
        let rows = index.visibleRows(collapsed: collapsed, query: query, hiddenKinds: hiddenKinds, additionalMatches: query.isEmpty ? [] : matchedLines)
        let visibleLines = Set(rows.map(\.heading.line))
        let annotated = matches.compactMap { item -> Entry? in
            let ancestors = parents(of: item.value).filter { !hiddenKinds.contains(index.rows[$0].heading.kind) }
            guard !query.isEmpty || !ancestors.contains(where: { collapsed.contains(index.rows[$0].key) }) else { return nil }
            return Entry(row: nil, annotation: item.value, kind: item.kind, todo: item.todo, depth: ancestors.filter { visibleLines.contains(index.rows[$0].heading.line) }.count)
        }
        // A marker can be a heading's only child; it must still offer folding.
        let parentsWithMarkers = Set(annotated.flatMap { parents(of: $0.annotation!).map { index.rows[$0].heading.line } })
        // Include collapsed parents too, whose markers are currently hidden.
        let allParents = parentsWithMarkers.union(matches.flatMap { parents(of: $0.value).map { index.rows[$0].heading.line } })
        let entries = rows.map { row -> Entry in
            var row = row
            row.hasChildren = row.hasChildren || allParents.contains(row.heading.line)
            return Entry(row: row, annotation: nil, kind: row.heading.kind, depth: row.depth)
        } + annotated
        return entries.sorted {
            if $0.line != $1.line { return $0.line < $1.line }
            if $0.row != nil { return $1.row == nil }
            if $1.row != nil { return false }
            return $0.annotation!.start < $1.annotation!.start
        }
    }
}
