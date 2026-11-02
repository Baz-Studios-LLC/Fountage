import AppKit

struct ManuscriptEdit {
    let id = UUID()
    let source: String
    let range: NSRange
    let replacement: String
    let selection: NSRange
    let actionName: String

    @MainActor
    @discardableResult
    func apply(to editor: NSTextView) -> Bool {
        guard !editor.hasMarkedText(), editor.string.utf8.elementsEqual(source.utf8) else { return false }
        editor.breakUndoCoalescing()
        editor.insertText(replacement, replacementRange: range)
        editor.breakUndoCoalescing()
        editor.undoManager?.setActionName(actionName)
        editor.setSelectedRange(selection)
        editor.scrollRangeToVisible(selection)
        return true
    }
}

extension OutlineIndex {
    func neighbor(of line: Int, down: Bool) -> Row? {
        guard let index = rows.firstIndex(where: { $0.heading.line == line }), !([ElementKind.synopsis, .pageBreak].contains(rows[index].heading.kind)) else { return nil }
        let siblings = rows.indices.filter { rows[$0].ancestors == rows[index].ancestors }
        guard let position = siblings.firstIndex(of: index) else { return nil }
        let next = position + (down ? 1 : -1)
        guard siblings.indices.contains(next), rows[siblings[next]].heading.kind == rows[index].heading.kind else { return nil }
        return rows[siblings[next]]
    }

    func move(line: Int, down: Bool, source: String) -> ManuscriptEdit? {
        guard let neighbor = neighbor(of: line, down: down) else { return nil }
        return move(line: line, to: neighbor.heading.line, after: down, source: source)
    }

    func canMove(line: Int, to targetLine: Int, after: Bool) -> Bool {
        guard let from = rows.firstIndex(where: { $0.heading.line == line }),
              let target = rows.firstIndex(where: { $0.heading.line == targetLine }),
              ![ElementKind.synopsis, .pageBreak].contains(rows[from].heading.kind) else { return false }
        let end = rows.indices.dropFirst(from + 1).first { !rows[$0].ancestors.contains(from) } ?? rows.count
        guard !(from..<end).contains(target) else { return false }
        let insertion = after ? rows.indices.dropFirst(target + 1).first { !rows[$0].ancestors.contains(target) } ?? rows.count : target
        return insertion != from && insertion != end
    }

    func move(line: Int, to targetLine: Int, after: Bool, source: String) -> ManuscriptEdit? {
        guard !source.contains("\0"), canMove(line: line, to: targetLine, after: after),
              let from = rows.firstIndex(where: { $0.heading.line == line }),
              let target = rows.firstIndex(where: { $0.heading.line == targetLine }) else { return nil }
        func subtreeEnd(_ index: Int) -> Int {
            rows.indices.dropFirst(index + 1).first { !rows[$0].ancestors.contains(index) } ?? rows.count
        }
        let end = subtreeEnd(from)
        guard !(from..<end).contains(target) else { return nil }
        var insertion = after ? subtreeEnd(target) : target
        let moving = Array(from..<end)
        var order = Array(rows.indices)
        order.removeSubrange(from..<end)
        if insertion >= end { insertion -= moving.count }
        order.insert(contentsOf: moving, at: insertion)
        guard let first = order.indices.first(where: { order[$0] != $0 }),
              let last = order.indices.last(where: { order[$0] != $0 }) else { return nil }
        let ns = source as NSString
        let positions = rows.map { EditorJump(line: $0.heading.line).selection(in: source).location } + [ns.length]
        guard zip(positions, positions.dropFirst()).allSatisfy({ $0 < $1 }) else { return nil }
        func split(_ index: Int) -> (body: String, ending: String) {
            let value = ns.substring(with: NSRange(location: positions[index], length: positions[index + 1] - positions[index])) as NSString
            var cut = value.length
            while cut > 0 && [10, 13].contains(Int(value.character(at: cut - 1))) { cut -= 1 }
            return (value.substring(to: cut), value.substring(from: cut))
        }
        let pieces = rows.indices.map(split)
        // Move original bodies verbatim, keeping boundary separators in their slots.
        // The final slot keeps its absent newline without fusing two headings.
        let replacement = (first...last).map { pieces[order[$0]].body + pieces[$0].ending }.joined()
        let destinationSlot = order.firstIndex(of: from)!
        let prefix = (first..<destinationSlot).map { pieces[order[$0]].body + pieces[$0].ending }.joined()
        let start = positions[first]
        return ManuscriptEdit(source: source, range: NSRange(location: start, length: positions[last + 1] - start), replacement: replacement,
                              selection: NSRange(location: start + prefix.utf16.count, length: 0), actionName: "Move Outline Item")
    }
}
