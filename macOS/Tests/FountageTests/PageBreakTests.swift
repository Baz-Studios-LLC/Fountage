import XCTest
import AppKit
@testable import Fountage

final class PageBreakTests: XCTestCase {
    func testBreaksNavigateWithoutChangingSceneCountsOrMoveOwnership() throws {
        let source = "# Act\r\n\r\nINT. A - DAY\r\n\r\nFirst.\r\n\r\n===\r\n\r\nMore.\r\n\r\nEXT. B - NIGHT\r\n\r\nSecond."
        let analysis = ManuscriptAnalysis.parse(source, format: .screenplay)
        XCTAssertEqual(analysis.units, 2)
        let index = OutlineIndex(analysis.headings)
        let page = try XCTUnwrap(index.rows.first { $0.heading.kind == .pageBreak })
        XCTAssertEqual(page.heading.line, 6)
        XCTAssertEqual(page.ancestors.count, 2)
        XCTAssertNil(index.neighbor(of: 6, down: true))
        let found = index.visibleRows(collapsed: index.expandableKeys, query: "Page Break")
        XCTAssertEqual(found.map(\.heading.kind), [.section, .scene, .pageBreak])
        let edit = try XCTUnwrap(index.move(line: 2, down: true, source: source))
        let moved = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
        XCTAssertTrue(moved.hasSuffix("First.\r\n\r\n===\r\n\r\nMore."))
        XCTAssertEqual(ManuscriptAnalysis.parse(moved, format: .screenplay).units, 2)
        XCTAssertTrue(ManuscriptAnalysis.parse("/*\n===\n*/", format: .screenplay).headings.isEmpty)
    }
    @MainActor
    func testInsertionKeepsLineEndingsAndSupportsUndoRedo() throws {
        for newline in ["\n", "\r\n"] {
            let source = "INT. ROOM - DAY" + newline + newline + "Zoë 🌙."
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            defer { window.close() }
            let editor = ManuscriptTextView(frame: window.contentView!.bounds)
            window.contentView!.addSubview(editor)
            editor.allowsUndo = true
            editor.supportsFountainFormatting = true
            editor.string = source
            editor.setSelectedRange(NSRange(location: source.utf16.count, length: 0))
            editor.undoManager?.beginUndoGrouping()
            editor.insertFountainPageBreak(nil)
            editor.undoManager?.endUndoGrouping()
            let expected = source + newline + newline + "===" + newline + newline
            XCTAssertEqual(editor.string, expected)
            XCTAssertEqual(ManuscriptAnalysis.parse(expected, format: .screenplay).headings.last?.kind, .pageBreak)
            editor.undoManager?.undo()
            XCTAssertEqual(editor.string, source)
            editor.undoManager?.redo()
            XCTAssertEqual(editor.string, expected)
            var document = ScreenplayDocument()
            document.text = expected
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(expected.utf8))
        }
    }
}
