import AppKit

// Formatting is a source edit, never transient rich-text state.
enum FountainFormatting {
    enum Style: String, CaseIterable {
        case bold, italic, underline
        var marker: String { switch self { case .bold: "**"; case .italic: "*"; case .underline: "_" } }
        var title: String { rawValue.capitalized }
        var selector: Selector {
            switch self {
            case .bold: #selector(ManuscriptTextView.fountainBold(_:))
            case .italic: #selector(ManuscriptTextView.fountainItalic(_:))
            case .underline: #selector(ManuscriptTextView.fountainUnderline(_:))
            }
        }
    }
    static func edit(source: String, selection originalSelection: NSRange, style: Style) -> ManuscriptEdit? {
        var selection = originalSelection
        guard Range(selection, in: source) != nil else { return nil }
        let ns = source as NSString
        for boundary in [selection.location, NSMaxRange(selection)] where boundary > 0 && boundary < ns.length {
            if (0xD800...0xDBFF).contains(ns.character(at: boundary - 1)) && (0xDC00...0xDFFF).contains(ns.character(at: boundary)) { return nil }
        }
        let width = style.marker.utf16.count
        if selection.length > 0, selection.location >= width, NSMaxRange(selection) <= ns.length - width {
            let before = ns.substring(to: selection.location)
            let after = ns.substring(from: NSMaxRange(selection))
            let marker = style.marker.first!
            let leftCount = before.reversed().prefix { $0 == marker }.count
            let rightCount = after.prefix { $0 == marker }.count
            let escaped = before.dropLast(leftCount).reversed().prefix { $0 == "\\" }.count % 2 == 1
            if !escaped && leftCount >= width && rightCount >= width && (style != .italic || (leftCount % 2 == 1 && rightCount % 2 == 1)) {
                selection = NSRange(location: selection.location - width, length: selection.length + width * 2)
            }
        }
        let text = (source as NSString).substring(with: selection)
        let marker = style.marker
        if selection.length == 0 {
            return ManuscriptEdit(source: source, range: selection, replacement: marker + marker,
                                  selection: NSRange(location: selection.location + marker.utf16.count, length: 0), actionName: style.title)
        }
        // Preserve all line endings and edge whitespace; Fountain emphasis is line-local.
        let lines = text.components(separatedBy: "\n")
        let bodies = lines.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        let nonempty = bodies.filter { !$0.isEmpty }
        guard !nonempty.isEmpty else { return nil }
        let removing = nonempty.allSatisfy { body in
            let wrapped = body.hasPrefix(marker) && body.hasSuffix(marker) && body.count > marker.count * 2
            if style != .italic { return wrapped }
            return wrapped && body.prefix { $0 == "*" }.count % 2 == 1 && body.reversed().prefix { $0 == "*" }.count % 2 == 1
        }
        let replacement = zip(lines, bodies).map { line, body -> String in
            guard !body.isEmpty, let range = line.range(of: body) else { return line }
            let changed = removing ? String(body.dropFirst(marker.count).dropLast(marker.count)) : marker + body + marker
            return String(line[..<range.lowerBound]) + changed + String(line[range.upperBound...])
        }.joined(separator: "\n")
        return ManuscriptEdit(source: source, range: selection, replacement: replacement,
                              selection: NSRange(location: selection.location, length: replacement.utf16.count), actionName: style.title)
    }


}

extension ManuscriptTextView {
    @objc func addSceneNumbers(_ sender: Any?) { numberScenes(remove: false) }
    @objc func removeSceneNumbers(_ sender: Any?) { numberScenes(remove: true) }
    @objc func removeSelectedSceneNumbers(_ sender: Any?) { numberScenes(remove: true, selectedOnly: true) }
    @objc func configureSceneNumbers(_ sender: Any?) {
        guard supportsFountainFormatting, isEditable, !hasMarkedText(), selectedRanges.count == 1, let window else { return }
        let source = string, selection = selectedRange()
        let alert = NSAlert()
        alert.messageText = "Number Scenes"
        alert.informativeText = "Replace existing scene numbers in the chosen range. Headings touched by the selection are included. This can be undone."
        alert.addButton(withTitle: "Number Scenes"); alert.addButton(withTitle: "Cancel")
        let start = NSTextField(string: "1")
        start.setAccessibilityLabel("Starting scene number")
        let scope = NSPopUpButton(frame: .zero, pullsDown: false)
        scope.addItems(withTitles: ["Entire Screenplay", "Selected Text"])
        scope.item(at: 1)?.isEnabled = selection.length > 0
        scope.autoenablesItems = false
        if selection.length > 0 { scope.selectItem(at: 1) }
        scope.setAccessibilityLabel("Numbering range")
        let firstLabel = NSTextField(labelWithString: "Start at")
        let rangeLabel = NSTextField(labelWithString: "Range")
        let grid = NSGridView(views: [[firstLabel, start], [rangeLabel, scope]])
        grid.rowSpacing = 12; grid.columnSpacing = 14
        grid.frame = NSRect(x: 0, y: 0, width: 330, height: 72)
        alert.accessoryView = grid
        alert.window.initialFirstResponder = start
        alert.beginSheetModal(for: window) { [weak self] response in
            guard response == .alertFirstButtonReturn, let self else { return }
            do {
                guard self.string == source else { throw SceneNumbering.NumberingError(message: "The manuscript changed while numbering was open. Try again. No text was changed.") }
                guard let first = UInt32(start.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)), first > 0 else {
                    throw SceneNumbering.NumberingError(message: "Enter a starting number between 1 and 4294967295. No text was changed.")
                }
                try SceneNumbering.edit(source: source, selection: selection, remove: false, firstNumber: first, selectedOnly: scope.indexOfSelectedItem == 1)?.apply(to: self)
            } catch { self.showNumberingError(error) }
        }
    }
    private func numberScenes(remove: Bool, selectedOnly: Bool = false) {
        guard supportsFountainFormatting, isEditable, !hasMarkedText(), selectedRanges.count == 1 else { return }
        do { try SceneNumbering.edit(source: string, selection: selectedRange(), remove: remove, selectedOnly: selectedOnly)?.apply(to: self) }
        catch { showNumberingError(error) }
    }
    private func showNumberingError(_ error: Error) {
        let alert = NSAlert(error: error)
        if let window { alert.beginSheetModal(for: window) }
        else { alert.runModal() }
    }

    @objc func insertFountainPageBreak(_ sender: Any?) {
        guard supportsFountainFormatting, isEditable, !hasMarkedText(), selectedRanges.count == 1 else { return }
        let newline = string.contains("\r\n") ? "\r\n" : "\n"
        let insertion = newline + newline + "===" + newline + newline
        let selected = selectedRange()
        ManuscriptEdit(source: string, range: selected, replacement: insertion,
                       selection: NSRange(location: selected.location + insertion.utf16.count, length: 0),
                       actionName: "Insert Page Break").apply(to: self)
    }

    @objc func fountainBold(_ sender: Any?) { applyFormatting(.bold) }
    @objc func fountainItalic(_ sender: Any?) { applyFormatting(.italic) }
    @objc func fountainUnderline(_ sender: Any?) { applyFormatting(.underline) }
    private func applyFormatting(_ style: FountainFormatting.Style) {
        guard supportsFountainFormatting, isEditable, !hasMarkedText(), selectedRanges.count == 1 else { return }
        FountainFormatting.edit(source: string, selection: selectedRange(), style: style)?.apply(to: self)
    }
}
