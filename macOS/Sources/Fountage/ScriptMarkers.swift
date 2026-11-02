import AppKit

/// Source directives stay in the manuscript; derived ranges come from Rust.
enum ScriptMarkers {
    private static func valid(_ selection: NSRange, in source: String) -> Bool {
        let length = source.utf16.count
        guard selection.location >= 0, selection.location <= length,
              selection.length >= 0, selection.length <= length - selection.location else { return false }
        let ns = source as NSString
        for boundary in [selection.location, NSMaxRange(selection)] where boundary > 0 && boundary < length {
            if (0xD800...0xDBFF).contains(ns.character(at: boundary - 1)) && (0xDC00...0xDFFF).contains(ns.character(at: boundary)) { return false }
        }
        return Range(selection, in: source) != nil
    }
    static func insertion(source: String, selection: NSRange, named: Bool) -> ManuscriptEdit? {
        guard !source.utf8.contains(0), valid(selection, in: source) else { return nil }
        // Insert at the selection start without consuming manuscript text.
        let at = selection.location
        let text = named ? "{{%m: }}" : "{{%m}}"
        return ManuscriptEdit(source: source, range: NSRange(location: at, length: 0), replacement: text,
            selection: NSRange(location: at + (named ? 6 : text.utf16.count), length: 0),
            actionName: named ? "Insert Named Marker" : "Insert Marker")
    }
    static func next(in source: String, selection: NSRange, format: DocumentFormat = .screenplay) -> NSRange? {
        guard !source.utf8.contains(0), valid(selection, in: source) else { return nil }
        let markers = ManuscriptAnalysis.parse(source, format: format).markers
        // Selected marker is skipped; reaching the end wraps to the first.
        return (markers.first { $0.start >= NSMaxRange(selection) && $0.range != selection } ?? markers.first)?.range
    }
    static func removal(source: String, selection: NSRange, format: DocumentFormat = .screenplay) -> ManuscriptEdit? {
        guard !source.utf8.contains(0), valid(selection, in: source) else { return nil }
        let markers = ManuscriptAnalysis.parse(source, format: format).markers
        guard !markers.isEmpty else { return nil }
        let result = NSMutableString(string: source)
        for marker in markers.reversed() { result.deleteCharacters(in: marker.range) }
        func mapped(_ position: Int) -> Int {
            position - markers.reduce(0) { $0 + max(0, min($1.length, position - $1.start)) }
        }
        let start = mapped(selection.location), end = mapped(NSMaxRange(selection))
        return ManuscriptEdit(source: source, range: NSRange(location: 0, length: source.utf16.count),
            replacement: result as String, selection: NSRange(location: start, length: end - start), actionName: "Remove Markers")
    }
    @MainActor static func addMenu(to tools: NSMenu) {
        tools.addItem(.separator())
        let menu = NSMenu(title: "Markers")
        for (title, selector, key, modifiers): (String, Selector, String, NSEvent.ModifierFlags) in [
            ("Insert Marker", #selector(ManuscriptTextView.insertScriptMarker(_:)), "m", [.control]),
            ("Insert Named Marker", #selector(ManuscriptTextView.insertNamedScriptMarker(_:)), "", []),
            ("Go to Next Marker", #selector(ManuscriptTextView.goToNextScriptMarker(_:)), "m", [.control, .option]),
            ("Remove Markers", #selector(ManuscriptTextView.removeScriptMarkers(_:)), "", [])
        ] {
            let item = NSMenuItem(title: title, action: selector, keyEquivalent: key)
            item.keyEquivalentModifierMask = modifiers; menu.addItem(item)
        }
        let item = NSMenuItem(title: "Markers", action: nil, keyEquivalent: ""); item.submenu = menu; tools.addItem(item)
    }
}

extension ManuscriptTextView {
    @objc func insertScriptMarker(_ sender: Any?) { insertMarker(named: false) }
    @objc func insertNamedScriptMarker(_ sender: Any?) { insertMarker(named: true) }
    private func insertMarker(named: Bool) {
        guard isEditable, !hasMarkedText(), selectedRanges.count == 1 else { return }
        ScriptMarkers.insertion(source: string, selection: selectedRange(), named: named)?.apply(to: self)
    }
    @objc func goToNextScriptMarker(_ sender: Any?) {
        guard !hasMarkedText(), selectedRanges.count == 1,
              let range = ScriptMarkers.next(in: string, selection: selectedRange(), format: manuscriptFormat) else { return }
        setSelectedRange(range); scrollRangeToVisible(range)
    }
    @objc func removeScriptMarkers(_ sender: Any?) {
        guard isEditable, !hasMarkedText(), selectedRanges.count == 1 else { return }
        ScriptMarkers.removal(source: string, selection: selectedRange(), format: manuscriptFormat)?.apply(to: self)
    }
}
