import XCTest
import AppKit
@testable import Fountage

@MainActor
final class TypewriterScrollingTests: XCTestCase {
    private func workspace(_ source: String, format: DocumentFormat = .screenplay) -> (NSWindow, ManuscriptScrollView, ManuscriptTextView) {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 720, height: 400), styleMask: [.titled], backing: .buffered, defer: true)
        window.isReleasedWhenClosed = false
        let scroll = ManuscriptScrollView(frame: NSRect(x: 0, y: 0, width: 720, height: 400))
        let editor = ManuscriptTextView(frame: scroll.bounds)
        editor.isRichText = false
        editor.isVerticallyResizable = true
        editor.isHorizontallyResizable = false
        editor.autoresizingMask = [.width]
        editor.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        editor.textContainer?.widthTracksTextView = true
        editor.textContainer?.containerSize = NSSize(width: 612, height: CGFloat.greatestFiniteMagnitude)
        editor.textContainerInset = NSSize(width: 54, height: 42)
        editor.allowsUndo = true
        editor.string = source
        scroll.documentView = editor
        window.contentView = scroll
        scroll.onWidthChange = { [weak editor] in
            if let editor { ManuscriptStyle.apply(to: editor, format: format, preset: format.presets[0], size: 16) }
        }
        scroll.layoutSubtreeIfNeeded()
        ManuscriptStyle.apply(to: editor, format: format, preset: format.presets[0], size: 16)
        window.makeFirstResponder(editor)
        return (window, scroll, editor)
    }

    private func assertCentered(_ editor: NSTextView, _ scroll: NSScrollView, file: StaticString = #filePath, line: UInt = #line) {
        let screenRect = editor.firstRect(forCharacterRange: editor.selectedRange(), actualRange: nil)
        let windowRect = editor.window!.convertFromScreen(screenRect)
        let localRect = editor.convert(windowRect, from: nil)
        // AppKit's caret rectangle excludes some paragraph leading in its line fragment.
        XCTAssertEqual(localRect.midY, scroll.contentView.bounds.midY, accuracy: 8, file: file, line: line)
    }

    func testCentersFirstMiddleAndFinalLinesAcrossEveryWritingTypeWithoutChangingSource() {
        for format in DocumentFormat.allCases {
            let source = format.starter + "\n" + (1...60).map { "Line \($0): Zoë sees 🌙 above the café." }.joined(separator: "\n") + "\n"
            let (window, scroll, editor) = workspace(source, format: format)
            defer { window.close() }
            scroll.typewriterScrolling = true
            for position in [0, (source as NSString).range(of: "Line 30").location, (source as NSString).length] {
                let selection = NSRange(location: position, length: 0)
                editor.setSelectedRange(selection)
                scroll.centerInsertionPoint()
                assertCentered(editor, scroll)
                XCTAssertEqual(editor.selectedRange(), selection)
                XCTAssertEqual(editor.string, source)
            }
        }
    }

    func testEmptyAndWrappedLinesResizeAndRestoreOrdinaryInsets() {
        for source in ["", "Zoë 🌙 " + String(repeating: "A long line of prose wraps across the page. ", count: 40), "Short line\r\n"] {
            let (window, scroll, editor) = workspace(source)
            defer { window.close() }
            editor.setSelectedRange(NSRange(location: (source as NSString).length, length: 0))
            scroll.typewriterScrolling = true
            scroll.centerInsertionPoint()
            assertCentered(editor, scroll)
            window.setContentSize(NSSize(width: 530, height: 650))
            scroll.layoutSubtreeIfNeeded()
            scroll.centerInsertionPoint()
            assertCentered(editor, scroll)
            let selection = editor.selectedRange()
            scroll.typewriterScrolling = false
            XCTAssertEqual(editor.textContainerInset.height, 42)
            XCTAssertEqual(editor.selectedRange(), selection)
            XCTAssertEqual(editor.string, source)
        }
    }

    func testReviewScrollingSelectionsAndMarkedTextAreNotOverridden() {
        let source = String(repeating: "A quiet signal arrives.\n", count: 80)
        let (window, scroll, editor) = workspace(source)
        defer { window.close() }
        scroll.typewriterScrolling = true
        editor.setSelectedRange(NSRange(location: (source as NSString).length, length: 0))
        scroll.centerInsertionPoint()
        scroll.contentView.scroll(to: .zero)
        scroll.reflectScrolledClipView(scroll.contentView)
        scroll.layoutSubtreeIfNeeded()
        XCTAssertEqual(scroll.contentView.bounds.minY, 0, accuracy: 1)
        editor.setSelectedRange(NSRange(location: 700, length: 50))
        scroll.centerInsertionPoint()
        XCTAssertEqual(scroll.contentView.bounds.minY, 0, accuracy: 1)
        editor.setSelectedRanges([NSValue(range: NSRange(location: 300, length: 5)), NSValue(range: NSRange(location: 700, length: 5))], affinity: .downstream, stillSelecting: false)
        scroll.centerInsertionPoint()
        XCTAssertEqual(scroll.contentView.bounds.minY, 0, accuracy: 1)
        editor.setSelectedRange(NSRange(location: 700, length: 0))
        editor.setMarkedText("あ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        scroll.contentView.scroll(to: .zero)
        scroll.centerInsertionPoint()
        XCTAssertEqual(scroll.contentView.bounds.minY, 0, accuracy: 1)
    }

    func testTypingAndUndoPreserveTextWithTypewriterScrolling() {
        let source = String(repeating: "The antenna turns.\n", count: 30)
        let (window, scroll, editor) = workspace(source)
        defer { window.close() }
        scroll.typewriterScrolling = true
        editor.setSelectedRange(NSRange(location: (source as NSString).length, length: 0))
        editor.undoManager?.beginUndoGrouping()
        editor.insertText("Zoë follows the signal. 🌙\n", replacementRange: editor.selectedRange())
        editor.undoManager?.endUndoGrouping()
        scroll.centerInsertionPoint()
        assertCentered(editor, scroll)
        XCTAssertEqual(editor.string, source + "Zoë follows the signal. 🌙\n")
        XCTAssertTrue(editor.undoManager?.canUndo == true)
        editor.undoManager?.undo()
        scroll.centerInsertionPoint()
        assertCentered(editor, scroll)
        XCTAssertEqual(editor.string, source)
        editor.undoManager?.redo()
        XCTAssertEqual(editor.string, source + "Zoë follows the signal. 🌙\n")
    }

    func testDisplayStylingKeepsNativeTypingUndoCoalesced() {
        let (window, scroll, editor) = workspace("A signal.\n\n")
        defer { window.close() }
        scroll.typewriterScrolling = true
        editor.setSelectedRange(NSRange(location: (editor.string as NSString).length, length: 0))
        for character in "A quiet room" {
            editor.undoManager?.beginUndoGrouping()
            editor.insertText(String(character), replacementRange: NSRange(location: NSNotFound, length: 0))
            XCTAssertTrue(editor.isCoalescingUndo)
            EditorPaperAppearance.matchApp.apply(to: scroll, editor: editor)
            ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
            scroll.centerInsertionPoint()
            XCTAssertTrue(editor.isCoalescingUndo, "Display-only styling must not end a typing undo group")
            editor.undoManager?.endUndoGrouping()
        }
        editor.undoManager?.undo()
        XCTAssertEqual(editor.string, "A signal.\n\n")
    }
}
