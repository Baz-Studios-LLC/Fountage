import Foundation

/// Edits title fields only; untouched blocks and the body retain their exact bytes.
struct TitlePageEdit {
    static let keys = ["Title", "Credit", "Author", "Source", "Draft date", "Contact"]
    struct Block {
        let key: String
        let label: String
        var range: NSRange
        var value: String
    }
    enum EditError: LocalizedError {
        case complexHeader, invalidValue, staleSource
        var errorDescription: String? {
            switch self {
            case .complexHeader: "This title page uses syntax the form cannot safely update. Edit its fields in the manuscript."
            case .invalidValue: "Use consecutive lines within each field. Blank lines end a Fountain title page; null characters are not supported."
            case .staleSource: "The manuscript changed while the title page was open. Reopen Title Page to edit the current version."
            }
        }
    }
    let source: String
    let blocks: [Block]
    let headerLength: Int
    let newline: String
    let original: [String: String]
    static func canonical(_ label: String) -> String {
        let value = label.lowercased().trimmingCharacters(in: .whitespaces)
        return value == "authors" ? "author" : value
    }
    init(source: String) throws {
        guard !source.contains("\0") else { throw EditError.invalidValue }
        self.source = source
        newline = source.contains("\r\n") ? "\r\n" : "\n"
        let titleLines = Set(ManuscriptAnalysis.parse(source, format: .screenplay).elements.filter { $0.kind == .title }.map(\.line))
        var blocks: [Block] = []
        var offset = 0
        var end = 0
        let lines = source.components(separatedBy: "\n")
        for (index, raw) in lines.enumerated() {
            let text = raw.hasSuffix("\r") ? String(raw.dropLast()) : raw
            let length = raw.utf16.count + (index < lines.count - 1 ? 1 : 0)
            defer { offset += length }
            guard titleLines.contains(index) else {
                // Hidden notes/omissions cannot be interpreted as editable field values.
                if !blocks.isEmpty, index < (titleLines.max() ?? 0) { throw EditError.complexHeader }
                continue
            }
            guard offset == end else { throw EditError.complexHeader }
            let indented = text.hasPrefix("   ") || text.hasPrefix("\t")
            if !indented, let colon = text.firstIndex(of: ":") {
                let label = String(text[..<colon]).trimmingCharacters(in: .whitespaces)
                guard !label.isEmpty, label.allSatisfy({ $0.isLetter || $0 == " " }) else { throw EditError.complexHeader }
                blocks.append(Block(key: Self.canonical(label), label: label, range: NSRange(location: offset, length: length),
                                    value: String(text[text.index(after: colon)...]).trimmingCharacters(in: .whitespaces)))
            } else if indented, !blocks.isEmpty {
                let value = text.trimmingCharacters(in: .whitespaces)
                if !blocks[blocks.count - 1].value.isEmpty { blocks[blocks.count - 1].value += "\n" }
                blocks[blocks.count - 1].value += value
                blocks[blocks.count - 1].range.length += length
            } else { throw EditError.complexHeader }
            end = offset + length
        }
        self.blocks = blocks; headerLength = end
        original = Dictionary(grouping: blocks, by: \.key).mapValues { $0.map(\.value).filter { !$0.isEmpty }.joined(separator: "\n") }
    }
    func edit(values: [String: String]) throws -> ManuscriptEdit? {
        let known = Self.keys.map(Self.canonical)
        let changed = Set(known.filter { !(values[$0] ?? "").utf8.elementsEqual((original[$0] ?? "").utf8) })
        guard !changed.isEmpty else { return nil }
        func normalized(_ value: String) throws -> [String] {
            let value = value.replacingOccurrences(of: "\r\n", with: "\n").trimmingCharacters(in: .newlines)
            guard !value.contains("\0"), !value.contains("\r") else { throw EditError.invalidValue }
            if value.isEmpty { return [] }
            let lines = value.components(separatedBy: "\n")
            guard lines.allSatisfy({ !$0.trimmingCharacters(in: .whitespaces).isEmpty }) else { throw EditError.invalidValue }
            return lines
        }
        func render(key: String, label: String) throws -> String {
            let lines = try normalized(values[key] ?? "")
            guard !lines.isEmpty else { return "" }
            if lines.count == 1 { return label + ": " + lines[0] + newline }
            return label + ":" + newline + lines.map { "    " + $0 + newline }.joined()
        }
        let ns = source as NSString
        var replacement = ""
        var emitted: Set<String> = []
        for block in blocks {
            if changed.contains(block.key) {
                if emitted.insert(block.key).inserted { replacement += try render(key: block.key, label: block.label) }
            } else { replacement += ns.substring(with: block.range) }
        }
        for label in Self.keys {
            let key = Self.canonical(label)
            if changed.contains(key), !emitted.contains(key) {
                let rendered = try render(key: key, label: label)
                if !rendered.isEmpty, !replacement.isEmpty, replacement.utf8.last != 10 { replacement += newline }
                replacement += rendered
            }
        }
        if !replacement.isEmpty {
            let firstKey = Self.canonical(String(replacement.prefix { $0 != ":" }))
            if !["title", "credit", "author", "source", "draft date", "contact", "copyright", "notes"].contains(firstKey) {
                // An empty recognized field anchors preserved custom metadata after
                // the writer removes the previously first recognized field.
                replacement = "Title:" + newline + replacement
            }
        }
        if headerLength == 0, !replacement.isEmpty { replacement += newline }
        let range = NSRange(location: 0, length: headerLength)
        return ManuscriptEdit(source: source, range: range, replacement: replacement,
                              selection: NSRange(location: 0, length: 0), actionName: "Edit Title Page")
    }
}
