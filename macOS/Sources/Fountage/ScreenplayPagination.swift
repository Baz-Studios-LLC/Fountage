import AppKit
import CoreText

/// One immutable layout feeds the preview, PDF export and printing.
struct ScreenplayPagination {
    struct Fragment {
        let text: NSAttributedString
        let x: CGFloat
        let top: CGFloat
        let width: CGFloat
        let alignment: CGFloat
        let sourceLine: Int
        var generated = false
    }
    private struct FontKey: Hashable {
        let family: String
        let bold: Bool
        let italic: Bool
    }
    private struct FragmentKey: Hashable {
        // Swift String equality normalizes Unicode; byte identity is required
        // because emphasis runs and editor references use exact UTF-16 offsets.
        let bytes: Data
        let kind: ElementKind
        let runs: [ScreenplayOutput.Run]
        let x: CGFloat
        let width: CGFloat
        let alignment: CGFloat
    }
    private final class LayoutCache {
        let enabled: Bool
        private var fonts: [FontKey: NSFont] = [:]
        var fragments: [FragmentKey: [Fragment]] = [:]
        init(enabled: Bool) { self.enabled = enabled }
        func font(bold: Bool = false, italic: Bool = false, family: String) -> NSFont {
            let key = FontKey(family: family, bold: bold, italic: italic)
            if enabled, let existing = fonts[key] { return existing }
            let value = ScreenplayPagination.font(bold: bold, italic: italic, family: family)
            if enabled { fonts[key] = value }
            return value
        }
    }
    struct Page {
        var fragments: [Fragment] = []
        var number: Int?
    }
    enum LayoutError: LocalizedError {
        case titleOverflow, dialogueCueOverflow, pdfFailure
        var errorDescription: String? {
            switch self {
            case .titleOverflow: "The title page contains more text than fits on one page. Shorten the title fields before exporting."
            case .dialogueCueOverflow: "A character cue is too long to fit with its dialogue on a page. Shorten the cue before exporting."
            case .pdfFailure: "Fountage could not create the PDF. Your manuscript has not been changed."
            }
        }
    }
    static let pageSize = CGSize(width: 612, height: 792)
    static let lineHeight: CGFloat = 12
    static let rowsPerPage = 54
    let pages: [Page]
    let paperSize: CGSize
    let bodyFontFamily: String

    init(output: ScreenplayOutput, dialogueContinuations: Bool) throws {
        var options = ScreenplayPrintOptions(); options.dialogueContinuations = dialogueContinuations
        try self.init(output: output, options: options)
    }
    init(output: ScreenplayOutput, options: ScreenplayPrintOptions = .init(), cacheEnabled: Bool = true) throws {
        let cache = LayoutCache(enabled: cacheEnabled)
        bodyFontFamily = options.bodyFontFamily
        paperSize = options.paper.size
        let pageSize = options.paper.size
        let rowLimit = Int(floor((pageSize.height - 72 - 82) / Self.lineHeight)) + 1
        let dialogueContinuations = options.dialogueContinuations
        var pages: [Page] = []
        let centered = output.title_fields.filter { $0.placement == .center }.flatMap(\.values)
        let contact = output.title_fields.filter { $0.placement == .lowerLeft }.flatMap(\.values)
        if options.includeTitlePage && (!centered.isEmpty || !contact.isEmpty) {
            var title = Page(number: nil)
            let middle = centered.flatMap { Self.fragments($0, x: 72, width: pageSize.width - 144, alignment: 0.5, options: options, cache: cache) }
            let lower = contact.flatMap { Self.fragments($0, x: 108, width: pageSize.width - 180, options: options, cache: cache) }
            let centerTop: CGFloat = 252
            let lowerTop = pageSize.height - 72 - CGFloat(lower.count) * Self.lineHeight
            guard centerTop + CGFloat(middle.count) * Self.lineHeight + 36 <= lowerTop,
                  lowerTop >= 72 else { throw LayoutError.titleOverflow }
            for (index, item) in middle.enumerated() { title.fragments.append(Self.position(item, top: centerTop + CGFloat(index) * Self.lineHeight)) }
            for (index, item) in lower.enumerated() { title.fragments.append(Self.position(item, top: lowerTop + CGFloat(index) * Self.lineHeight)) }
            pages.append(title)
        }
        var page = Page(number: 1)
        var row = 0
        var trailingBlankRows = 0
        func nextPage() {
            pages.append(page)
            page = Page(number: (page.number ?? 0) + 1)
            row = 0; trailingBlankRows = 0
        }
        func appendRows(_ rows: [[Fragment]], keep: Int = 1) {
            if row > 0 && rowLimit - row < min(keep, rows.count) { nextPage() }
            for items in rows {
                if row == rowLimit { nextPage() }
                page.fragments += items.map { Self.position($0, top: 82 + CGFloat(row) * Self.lineHeight) }
                row += 1
                trailingBlankRows = items.allSatisfy { $0.text.string.isEmpty } ? trailingBlankRows + 1 : 0
            }
        }
        // Keep at least two rows of a split action paragraph on each page.
        // Short one-row paragraphs remain valid; explicit source breaks are untouched.
        func appendParagraph(_ rows: [[Fragment]]) {
            var offset = 0
            while offset < rows.count {
                let remaining = rows.count - offset
                let room = rowLimit - row
                var take = min(remaining, room)
                if remaining > take && remaining - take == 1 { take -= 1 }
                if take <= 0 || (row > 0 && remaining > take && take < 2) {
                    nextPage(); continue
                }
                appendRows(Array(rows[offset..<(offset + take)]))
                offset += take
                if offset < rows.count { nextPage() }
            }
        }
        func appendDialogue(_ speeches: [Speech]) throws {
            let total = speeches.map { $0.cue.count + $0.content.count }.max() ?? 0
            if row > 0 && total <= rowLimit && total > rowLimit - row { nextPage() }
            var offsets = Array(repeating: 0, count: speeches.count)
            var firstSegment = true
            repeat {
                let headers = speeches.enumerated().map { index, speech in
                    firstSegment ? speech.cue : offsets[index] < speech.content.count && dialogueContinuations ? speech.continuedCue : []
                }
                let headerHeight = headers.map(\.count).max() ?? 0
                guard headerHeight < rowLimit - 2 else { throw LayoutError.dialogueCueOverflow }
                let remaining = speeches.enumerated().map { $0.element.content.count - offsets[$0.offset] }
                let longest = remaining.max() ?? 0
                let room = rowLimit - row
                let needsMore = dialogueContinuations && longest > room - headerHeight
                let capacity = room - headerHeight - (needsMore ? 1 : 0)
                // A cue must share the page with at least two content rows when available.
                if row > 0 && capacity < min(2, longest) { nextPage(); continue }
                var takes = remaining.map { min($0, max(0, capacity)) }
                for i in speeches.indices where takes[i] < remaining[i] {
                    // Avoid a single final line on the following page.
                    if remaining[i] - takes[i] == 1 && takes[i] > 2 { takes[i] -= 1 }
                    // Protect individual wrapped dialogue paragraphs as well as the whole speech.
                    let content = speeches[i].content
                    let split = offsets[i] + takes[i]
                    if takes[i] > 0, split < content.count,
                       content[split - 1].kind == .dialogue, content[split].kind == .dialogue,
                       content[split - 1].fragment.sourceLine == content[split].fragment.sourceLine {
                        let line = content[split].fragment.sourceLine
                        var beginning = split - 1, end = split + 1
                        while beginning > offsets[i] && content[beginning - 1].fragment.sourceLine == line { beginning -= 1 }
                        while end < content.count && content[end].fragment.sourceLine == line { end += 1 }
                        if end - split == 1 { takes[i] -= 1 }
                        if offsets[i] + takes[i] - beginning == 1 { takes[i] -= 1 }
                    }
                    // Keep a parenthetical with the dialogue that follows it.
                    while takes[i] > 0 && content[offsets[i] + takes[i] - 1].kind == .parenthetical { takes[i] -= 1 }
                }
                if row > 0 && zip(takes, remaining).contains(where: { $0 == 0 && $1 > 0 }) { nextPage(); continue }
                // A parenthetical longer than a whole page must itself be allowed to split.
                for i in speeches.indices where takes[i] == 0 && remaining[i] > 0 { takes[i] = min(remaining[i], capacity) }
                let more = speeches.indices.map { takes[$0] < remaining[$0] }
                let count = max(headerHeight, speeches.indices.map { headerHeight + takes[$0] + (more[$0] && dialogueContinuations ? 1 : 0) }.max() ?? 0)
                var rows = Array(repeating: [Fragment](), count: count)
                for i in speeches.indices {
                    for (j, header) in headers[i].enumerated() { rows[j].append(header) }
                    for j in 0..<takes[i] { rows[headerHeight + j].append(speeches[i].content[offsets[i] + j].fragment) }
                    if more[i] && dialogueContinuations { rows[headerHeight + takes[i]].append(speeches[i].more) }
                    offsets[i] += takes[i]
                }
                appendRows(rows)
                firstSegment = false
                if more.contains(true) { nextPage() }
            } while speeches.indices.contains(where: { offsets[$0] < speeches[$0].content.count })
        }
        // Character cues and their following dialogue form one unit; the caret joins
        // the second unit to the preceding unit as two parallel columns.
        var units: [[ScreenplayOutput.Line]] = []
        let includedOutline = output.outline.filter { line in
            line.kind == .synopsis ? options.includeSynopses : line.section_level.map { options.visibleSectionLevels.contains($0) } ?? false
        }
        let printable = output.body.filter { $0.kind != .todo || (options.includeTodos && (options.includeCompletedTodos || $0.todo_completed != true)) }
        var body = (printable + includedOutline).sorted { $0.source_line < $1.source_line }[...]
        if !output.title.isEmpty || printable.count != output.body.count {
            while body.first?.text.isEmpty == true && body.first?.kind == .body { body = body.dropFirst() }
        }
        if printable.count != output.body.count {
            while body.last?.text.isEmpty == true && body.last?.kind == .body { body = body.dropLast() }
        }
        for line in body {
            if [.dialogue, .parenthetical].contains(line.kind),
               units.last?.first?.kind == .character {
                units[units.count - 1].append(line)
            } else if [.body, .lyric].contains(line.kind), !line.text.isEmpty,
                      let last = units.last?.last, last.kind == line.kind, !last.text.isEmpty {
                // Consecutive physical action lines are one paragraph until a blank line.
                units[units.count - 1].append(line)
            } else { units.append([line]) }
        }
        var index = 0
        while index < units.count {
            let unit = units[index]
            let first = unit[0]
            if first.kind == .pageBreak {
                // Explicit breaks create a new page even when the preceding page is blank.
                nextPage(); index += 1; continue
            }
            if first.kind == .character {
                var next = index + 1
                while next < units.count && units[next].allSatisfy({ $0.text.isEmpty && $0.kind == .body }) { next += 1 }
                if next < units.count, units[next][0].kind == .character, units[next][0].dual_dialogue {
                    try appendDialogue([Self.speech(unit, columnX: 108, columnWidth: (pageSize.width - 216) / 2, options: options, cache: cache), Self.speech(units[next], columnX: 108 + (pageSize.width - 216) / 2 + 36, columnWidth: (pageSize.width - 216) / 2, options: options, cache: cache)])
                    index = next + 1; continue
                }
            }
            if first.kind == .character {
                try appendDialogue([Self.speech(unit, options: options, cache: cache)])
                index += 1; continue
            }
            var rows = unit.flatMap { Self.fragments($0, options: options, cache: cache) }.map { [$0] }
            if let number = first.scene_number, !rows.isEmpty {
                let ordinary = NSAttributedString(string: number, attributes: [.font: cache.font(family: options.headingFontFamily)])
                let width = CTLineGetTypographicBounds(CTLineCreateWithAttributedString(ordinary), nil, nil, nil)
                let size = min(12, 360 / max(1, width))
                let font = NSFontManager.shared.convert(cache.font(family: options.headingFontFamily), toSize: size)
                let label = NSAttributedString(string: number, attributes: [.font: font, .foregroundColor: NSColor.black])
                rows[0].append(Fragment(text: label, x: 72, top: 0, width: 30, alignment: 0, sourceLine: first.source_line))
                rows[0].append(Fragment(text: label, x: pageSize.width - 66, top: 0, width: 54, alignment: 0, sourceLine: first.source_line))
            }
            if first.kind == .section && row > 0 && row + rows.count + 2 > rowLimit { nextPage() }
            if first.kind == .scene {
                let extra = options.doubleSpaceSceneHeadings && row > 0 ? max(0, 2 - trailingBlankRows) : 0
                // Include source blank rows and the following paragraph in the keep.
                // Otherwise moving a three-row paragraph could strand its heading.
                var keep = rows.count
                var following = index + 1
                while following < units.count && units[following].allSatisfy({ $0.kind == .body && $0.text.isEmpty }) {
                    keep += units[following].count; following += 1
                }
                if following < units.count, units[following][0].kind != .pageBreak {
                    let nextRows = units[following].flatMap { Self.fragments($0, options: options, cache: cache) }.count
                    keep += min(3, nextRows)
                }
                if row > 0 && row + extra + max(4, keep) > rowLimit { nextPage() }
                else if extra > 0 { appendRows(Array(repeating: [], count: extra)) }
            }
            if [.body, .lyric].contains(first.kind), !first.text.isEmpty {
                appendParagraph(rows)
            } else {
                appendRows(rows, keep: first.kind == .character ? (rows.count <= rowLimit ? rows.count : 3) : 1)
            }
            index += 1
        }
        // Suppress an accidental trailing empty page, but retain a blank manuscript page.
        if !page.fragments.isEmpty || pages.isEmpty || page.number == 1 { pages.append(page) }
        self.pages = pages
    }

    private struct SpeechRow { let fragment: Fragment; let kind: ElementKind }
    private struct Speech {
        let cue: [Fragment]
        let continuedCue: [Fragment]
        let content: [SpeechRow]
        let more: Fragment
    }
    private static func speech(_ lines: [ScreenplayOutput.Line], columnX: CGFloat? = nil, columnWidth: CGFloat? = nil, options: ScreenplayPrintOptions, cache: LayoutCache) -> Speech {
        func render(_ line: ScreenplayOutput.Line) -> [Fragment] {
            guard let x = columnX else { return fragments(line, options: options, cache: cache) }
            return fragments(line, x: x + (line.kind == .character ? 36 : line.kind == .parenthetical ? 18 : 0),
                             width: (columnWidth ?? 198) - (line.kind == .character ? 48 : line.kind == .parenthetical ? 42 : 0), options: options, cache: cache)
        }
        let cue = lines[0]
        let hasContinuation = cue.text.uppercased().contains("(CONT'D)") || cue.text.uppercased().contains("(CONT’D)")
        let repeated = ScreenplayOutput.Line(source_line: cue.source_line, kind: .character,
            text: cue.text + (hasContinuation ? "" : " (CONT'D)"), runs: cue.runs, scene_number: nil, section_level: nil, dual_dialogue: false)
        let continued = render(repeated).map { item in
            var copy = item; copy.generated = true; return copy
        }
        let more = Fragment(text: NSAttributedString(string: "(MORE)", attributes: [.font: cache.font(family: options.bodyFontFamily), .foregroundColor: NSColor.black]),
                            x: columnX.map { $0 + 36 } ?? 252, top: 0, width: columnX == nil ? options.paper.size.width - 324 : (columnWidth ?? 198) - 48,
                            alignment: 0, sourceLine: cue.source_line, generated: true)
        return Speech(cue: render(cue), continuedCue: continued,
                      content: lines.dropFirst().flatMap { line in render(line).map { SpeechRow(fragment: $0, kind: line.kind) } }, more: more)
    }

    private static func position(_ item: Fragment, top: CGFloat) -> Fragment {
        Fragment(text: item.text, x: item.x, top: top, width: item.width, alignment: item.alignment, sourceLine: item.sourceLine, generated: item.generated)
    }
    private static func fragments(_ line: ScreenplayOutput.Line, x: CGFloat? = nil, width: CGFloat? = nil, alignment: CGFloat? = nil, options: ScreenplayPrintOptions, cache: LayoutCache) -> [Fragment] {
        let geometry: (CGFloat, CGFloat, CGFloat)
        switch line.kind {
        case .character: geometry = (252, options.paper.size.width - 324, 0)
        case .dialogue: geometry = (180, 252, 0)
        case .parenthetical: geometry = (216, 180, 0)
        case .transition: geometry = (108, options.paper.size.width - 180, 1)
        case .centered: geometry = (108, options.paper.size.width - 180, 0.5)
        default: geometry = (108, options.paper.size.width - 180, 0)
        }
        let available = width ?? geometry.1
        let key = FragmentKey(bytes: Data(line.text.utf8), kind: line.kind, runs: line.runs,
                              x: x ?? geometry.0, width: available, alignment: alignment ?? geometry.2)
        if cache.enabled, let existing = cache.fragments[key] {
            return existing.map { Fragment(text: $0.text, x: $0.x, top: $0.top, width: $0.width,
                                           alignment: $0.alignment, sourceLine: line.source_line, generated: $0.generated) }
        }
        let text = styled(line, options: options, cache: cache)
        let typesetter = CTTypesetterCreateWithAttributedString(text)
        var result: [Fragment] = []
        var start = 0
        repeat {
            let length = text.length == 0 ? 0 : CTTypesetterSuggestLineBreak(typesetter, start, Double(available))
            // Core Text returns a complete cluster even when it exceeds the width.
            guard text.length == 0 || length > 0 else { break }
            let part = text.attributedSubstring(from: NSRange(location: start, length: length))
            result.append(Fragment(text: part, x: x ?? geometry.0, top: 0, width: available,
                                   alignment: alignment ?? geometry.2, sourceLine: line.source_line))
            start += length
        } while start < text.length
        // Bound derived state to this layout. Never retain a manuscript history.
        if cache.enabled, cache.fragments.count < 256 { cache.fragments[key] = result }
        return result
    }
    static func font(bold: Bool = false, italic: Bool = false, family: String = "Courier") -> NSFont {
        var font = NSFontManager.shared.font(withFamily: ScreenplayPrintOptions.availableFamily(family), traits: [], weight: 5, size: 12)
            ?? NSFont(name: "Courier", size: 12) ?? .monospacedSystemFont(ofSize: 12, weight: .regular)
        if bold { font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask) }
        if italic { font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask) }
        return font
    }
    private static func styled(_ line: ScreenplayOutput.Line, options: ScreenplayPrintOptions, cache: LayoutCache) -> NSAttributedString {
        let family = [.scene, .section].contains(line.kind) ? options.headingFontFamily : options.bodyFontFamily
        let semanticBold = line.kind == .section || (line.kind == .scene && options.boldSceneHeadings)
        let semanticItalic = line.kind == .lyric || line.kind == .synopsis
        let text = NSMutableAttributedString(string: line.text, attributes: [.font: cache.font(bold: semanticBold, italic: semanticItalic, family: family), .foregroundColor: NSColor.black])
        for run in line.runs {
            let range = NSRange(location: run.start, length: run.length)
            text.addAttribute(.font, value: cache.font(bold: run.style.bold || semanticBold, italic: run.style.italic || semanticItalic, family: family), range: range)
            if run.style.underline || (line.kind == .scene && options.underlineSceneHeadings) { text.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: range) }
        }
        if line.kind == .todo && text.length > 0 {
            text.addAttribute(.font, value: NSFont.systemFont(ofSize: 12), range: NSRange(location: 0, length: 1))
            if line.todo_completed == true && text.length > 2 {
                text.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: NSRange(location: 2, length: text.length - 2))
            }
        }
        return text
    }
    func pdfData() throws -> Data {
        let data = NSMutableData()
        var box = CGRect(origin: .zero, size: paperSize)
        guard let consumer = CGDataConsumer(data: data), let context = CGContext(consumer: consumer, mediaBox: &box, nil) else { throw LayoutError.pdfFailure }
        for page in pages {
            context.beginPDFPage(nil)
            context.setFillColor(NSColor.white.cgColor); context.fill(box)
            func draw(_ text: NSAttributedString, x: CGFloat, top: CGFloat, width: CGFloat, alignment: CGFloat) {
                let line = CTLineCreateWithAttributedString(text)
                let offset = CTLineGetPenOffsetForFlush(line, alignment, Double(width))
                context.textMatrix = .identity
                context.textPosition = CGPoint(x: x + offset, y: paperSize.height - top)
                CTLineDraw(line, context)

            }
            if let number = page.number, number > 1 {
                draw(NSAttributedString(string: "\(number).", attributes: [.font: Self.font(family: bodyFontFamily), .foregroundColor: NSColor.black]), x: paperSize.width - 144, top: 48, width: 72, alignment: 1)
            }
            for fragment in page.fragments { draw(fragment.text, x: fragment.x, top: fragment.top, width: fragment.width, alignment: fragment.alignment) }
            context.endPDFPage()
        }
        context.closePDF()
        guard data.length > 0 else { throw LayoutError.pdfFailure }
        return data as Data
    }
}
