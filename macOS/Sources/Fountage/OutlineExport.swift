import Foundation

/// Creates a separate, editable project from the Navigator's visible entries.
/// Never copies unrelated source text, project identity, private metadata or assets.
struct OutlineExport {
    enum ExportError: LocalizedError {
        case invalidSource, empty, invalidRange
        var errorDescription: String? {
            switch self {
            case .invalidSource: "The manuscript contains a null character. Remove it before exporting the outline."
            case .empty: "There are no visible Navigator entries to export."
            case .invalidRange: "Fountage could not locate an outline entry in the manuscript. Your original project has not been changed."
            }
        }
    }
    static func make(from document: ScreenplayDocument, collapsed: Set<OutlineIndex.Key>, query: String, notesOnly: Bool = false, markersOnly: Bool = false, hiddenKinds: Set<ElementKind> = []) throws -> ScreenplayDocument {
        let source = document.text
        guard !source.contains("\0") else { throw ExportError.invalidSource }
        let analysis = ManuscriptAnalysis.parse(source, format: document.format)
        let ns = source as NSString
        let newline = source.first(where: { $0.isNewline }) == "\r\n" ? "\r\n" : "\n"
        let entries: [String]
        if notesOnly || markersOnly {
            entries = try (markersOnly ? analysis.markers : analysis.notes).filter { query.isEmpty || $0.text.localizedCaseInsensitiveContains(query) }.map { note in
                guard Range(note.range, in: source) != nil else { throw ExportError.invalidRange }
                return ns.substring(with: note.range)
            }
        } else {
            let visible = NavigatorOutline.entries(index: OutlineIndex(analysis.headings), markers: analysis.markers, notes: analysis.notes, todos: analysis.todos, collapsed: collapsed, query: query, hiddenKinds: hiddenKinds)
            let lines = source.components(separatedBy: "\n")
            var position = 0
            let ranges = lines.map { line -> NSRange in
                defer { position += line.utf16.count + 1 }
                return NSRange(location: position, length: line.utf16.count)
            }
            let hidden = (analysis.notes.map(\.range) + analysis.markers.map(\.range) + analysis.omissions.map(\.range)).sorted { $0.location < $1.location }
            var hiddenCursor = 0
            entries = try visible.map { entry in
                if let annotation = entry.annotation, entry.kind != .todo {
                    guard Range(annotation.range, in: source) != nil else { throw ExportError.invalidRange }
                    return ns.substring(with: annotation.range)
                }
                guard ranges.indices.contains(entry.line) else { throw ExportError.invalidRange }
                let range = ranges[entry.line]
                guard Range(range, in: source) != nil else { throw ExportError.invalidRange }
                let text = NSMutableString(string: ns.substring(with: range))
                // Annotations export independently; omit them from enclosing source lines.
                while hiddenCursor < hidden.count && NSMaxRange(hidden[hiddenCursor]) <= range.location { hiddenCursor += 1 }
                var cursor = hiddenCursor
                var cuts: [NSRange] = []
                while cursor < hidden.count && hidden[cursor].location < NSMaxRange(range) {
                    let cut = NSIntersectionRange(hidden[cursor], range)
                    if cut.length > 0 { cuts.append(cut) }
                    cursor += 1
                }
                for cut in cuts.reversed() { text.deleteCharacters(in: NSRange(location: cut.location - range.location, length: cut.length)) }
                if entry.kind == .todo {
                    var result = text as String
                    while result.last?.isWhitespace == true { result.removeLast() }
                    return result
                }
                return (text as String).trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }
        guard !entries.isEmpty else { throw ExportError.empty }
        var exported = ScreenplayDocument()
        exported.format = document.format
        exported.preset = document.preset
        exported.academicStyle = document.academicStyle
        exported.needsTypeSelection = false
        exported.text = entries.joined(separator: newline + newline) + newline + newline
        return exported
    }
}
