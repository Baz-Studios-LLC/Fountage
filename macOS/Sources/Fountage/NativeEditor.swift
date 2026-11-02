import SwiftUI
import AppKit

struct EditorJump: Equatable {
    var line: Int
    var range: NSRange? = nil
    var token = UUID()
    var focusEditor = true
    func selection(in text: String) -> NSRange {
        let length = (text as NSString).length
        if let range {
            let start = max(0, min(range.location, length))
            return NSRange(location: start, length: max(0, min(range.length, length - start)))
        }
        let lines = text.components(separatedBy: "\n")
        let location = lines.prefix(max(0, line)).reduce(0) { $0 + ($1 as NSString).length + 1 }
        return NSRange(location: min(location, length), length: 0)
    }
}

enum EditorPosition {
    static func selectedWords(in text: String, ranges: [NSRange], manuscriptWords: [ManuscriptSourceRange]? = nil) -> Int? {
        let source = text as NSString
        let valid = ranges.filter { $0.location != NSNotFound && $0.location >= 0 && $0.location < source.length && $0.length > 0 }
            .map { NSRange(location: $0.location, length: min($0.length, source.length - $0.location)) }
            .sorted { $0.location < $1.location }
        var merged: [NSRange] = []
        for range in valid {
            if let last = merged.last, NSMaxRange(last) >= range.location {
                merged[merged.count - 1] = NSUnionRange(last, range)
            } else { merged.append(range) }
        }
        guard !merged.isEmpty else { return nil }
        if let manuscriptWords {
            return manuscriptWords.filter { word in merged.contains { NSIntersectionRange(word.range, $0).length > 0 } }.count
        }
        return merged.reduce(0) { $0 + source.substring(with: $1).split(whereSeparator: \.isWhitespace).count }
    }

    static func lineRanges(in text: String) -> [NSRange] {
        var ranges: [NSRange] = []
        var start = 0
        var offset = 0
        for unit in text.utf16 {
            offset += 1
            if unit == 10 {
                ranges.append(NSRange(location: start, length: offset - start))
                start = offset
            }
        }
        if start < offset { ranges.append(NSRange(location: start, length: offset - start)) }
        return ranges
    }

    // AppKit positions are UTF-16 offsets. Count LF code units rather than Swift
    // Characters: CRLF is one Character and does not compare equal to "\n".
    static func line(in text: String, at location: Int) -> Int {
        text.utf16.prefix(max(0, location)).reduce(0) { $1 == 10 ? $0 + 1 : $0 }
    }
}

struct NativeEditor: NSViewRepresentable {
    @Binding var text: String
    var jump: EditorJump?
    var edit: ManuscriptEdit? = nil
    var fontSize: CGFloat
    var format: DocumentFormat
    var preset: LayoutPreset
    var paper = EditorPaperAppearance.matchApp
    var typewriterScrolling = false
    var analysisCache = ManuscriptAnalysisCache()
    var onTitlePage: (() -> Void)? = nil
    var onSelectedWords: (Int?) -> Void = { _ in }
    var onShelfCapture: ((String, NSRange, Bool) throws -> Void)? = nil
    var onSourceSelection: (String, NSRange) -> Void = { _, _ in }
    var onCursor: (Int) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> NSScrollView {
        let scroll = ManuscriptScrollView()
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false
        let editor = ManuscriptTextView(frame: .zero)
        editor.supportsFountainFormatting = format == .screenplay
        editor.manuscriptFormat = format
        editor.onTitlePage = onTitlePage
        editor.onShelfCapture = onShelfCapture
        editor.isRichText = false
        editor.isEditable = true
        editor.isSelectable = true
        editor.allowsUndo = true
        editor.usesFindBar = true
        editor.isIncrementalSearchingEnabled = true
        editor.isAutomaticQuoteSubstitutionEnabled = false
        editor.isAutomaticDashSubstitutionEnabled = false
        editor.isAutomaticSpellingCorrectionEnabled = false
        editor.isContinuousSpellCheckingEnabled = true
        editor.isGrammarCheckingEnabled = false
        editor.isVerticallyResizable = true
        editor.isHorizontallyResizable = false
        editor.autoresizingMask = [.width]
        editor.textContainer?.widthTracksTextView = true
        editor.textContainer?.containerSize = NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude)
        editor.minSize = NSSize(width: 0, height: 0)
        editor.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        editor.textContainerInset = NSSize(width: 54, height: 42)
        editor.backgroundColor = .textBackgroundColor
        editor.textColor = .textColor
        editor.insertionPointColor = StudioTheme.accent
        editor.font = NSFont(name: "Courier", size: fontSize) ?? .monospacedSystemFont(ofSize: fontSize, weight: .regular)
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineSpacing = 7
        editor.defaultParagraphStyle = paragraph
        editor.typingAttributes = [.font: editor.font!, .paragraphStyle: paragraph, .foregroundColor: NSColor.textColor]
        editor.string = text
        editor.setAccessibilityLabel("Manuscript editor")
        editor.setAccessibilityHelp("Write your manuscript. Formatting follows the selected document type. Use Command F to find text.")
        editor.delegate = context.coordinator
        scroll.documentView = editor
        scroll.onWidthChange = { [weak editor, weak coordinator = context.coordinator] in
            guard let editor else { return }
            coordinator?.refreshDisplay(in: editor)
        }
        context.coordinator.refreshDisplay(in: editor)
        scroll.typewriterScrolling = typewriterScrolling
        return scroll
    }
    func updateNSView(_ scroll: NSScrollView, context: Context) {
        context.coordinator.parent = self
        guard let editor = scroll.documentView as? NSTextView else { return }
        (editor as? ManuscriptTextView)?.supportsFountainFormatting = format == .screenplay
        (editor as? ManuscriptTextView)?.manuscriptFormat = format
        (editor as? ManuscriptTextView)?.onTitlePage = onTitlePage
        (editor as? ManuscriptTextView)?.onShelfCapture = onShelfCapture
        if let edit, context.coordinator.lastEdit != edit.id {
            context.coordinator.lastEdit = edit.id
            DispatchQueue.main.async { [weak editor] in
                guard let editor else { return }
                edit.apply(to: editor)
            }
        }
        if !editor.string.utf8.elementsEqual(text.utf8) {
            let selection = editor.selectedRange()
            editor.string = text
            let location = min(selection.location, (text as NSString).length)
            editor.setSelectedRange(NSRange(location: location, length: 0))
        }
        context.coordinator.refreshDisplay(in: editor)
        (scroll as? ManuscriptScrollView)?.typewriterScrolling = typewriterScrolling
        if let jump, context.coordinator.lastJump != jump.token {
            context.coordinator.lastJump = jump.token
            let range = jump.selection(in: text)
            editor.setSelectedRange(range)
            editor.scrollRangeToVisible(range)
            if jump.focusEditor { editor.window?.makeFirstResponder(editor) }
            (scroll as? ManuscriptScrollView)?.scheduleCentering(requiresEditorFocus: false)
        }
    }
    class Coordinator: NSObject, NSTextViewDelegate {
        var parent: NativeEditor
        var lastJump: UUID?
        var lastEdit: UUID?
        var lastSource = ""
        var styleSignature = ""
        init(_ parent: NativeEditor) { self.parent = parent }
        func refreshDisplay(in editor: NSTextView) {
            guard let scroll = editor.enclosingScrollView, !editor.hasMarkedText() else { return }
            let signature = "\(parent.format.rawValue):\(parent.preset.rawValue):\(parent.fontSize):\(scroll.contentSize.width):\(parent.paper.rawValue)"
            let styleChanged = styleSignature != signature
            guard styleChanged || !lastSource.utf8.elementsEqual(editor.string.utf8) else { return }
            if styleChanged { parent.paper.apply(to: scroll, editor: editor) }
            ManuscriptStyle.apply(to: editor, format: parent.format, preset: parent.preset, size: parent.fontSize, analysisCache: parent.analysisCache)
            lastSource = editor.string
            styleSignature = signature
            (scroll as? ManuscriptScrollView)?.scheduleCentering()
        }
        func textDidChange(_ notification: Notification) {
            guard let editor = notification.object as? NSTextView else { return }
            parent.text = editor.string
            refreshDisplay(in: editor)
        }
        func textViewDidChangeSelection(_ notification: Notification) {
            guard let editor = notification.object as? NSTextView else { return }
            let line = EditorPosition.line(in: editor.string, at: editor.selectedRange().location)
            let ranges = editor.selectedRanges.map(\.rangeValue)
            let words = ranges.contains(where: { $0.length > 0 })
                ? parent.analysisCache.analysis(for: editor.string, format: parent.format).word_ranges : nil
            let selectedWords = EditorPosition.selectedWords(in: editor.string, ranges: ranges, manuscriptWords: words)
            let source = editor.string
            let singleRange = ranges.count == 1 ? ranges[0] : NSRange(location: NSNotFound, length: 0)
            DispatchQueue.main.async { [weak self] in
                self?.parent.onSourceSelection(source, singleRange)
                self?.parent.onCursor(line)
                self?.parent.onSelectedWords(selectedWords)
            }
            (editor.enclosingScrollView as? ManuscriptScrollView)?.scheduleCentering()
        }
    }
}

