import XCTest
import AppKit
@testable import Fountage

final class FountainFormattingTests: XCTestCase {
    func result(_ text: String, _ style: FountainFormatting.Style, range: NSRange? = nil) throws -> String {
        let edit = try XCTUnwrap(FountainFormatting.edit(source: text, selection: range ?? NSRange(location: 0, length: text.utf16.count), style: style))
        return (text as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
    }
    func testLineEndingsWhitespaceAndToggle() throws {
        for style in FountainFormatting.Style.allCases {
            let text = "  Zoë 🌙\r\n\r\nSecond line.  "
            let marked = try result(text, style)
            XCTAssertEqual(marked, "  \(style.marker)Zoë 🌙\(style.marker)\r\n\r\n\(style.marker)Second line.\(style.marker)  ")
            XCTAssertEqual(try result(marked, style), text)
        }
    }
    func testCombinationsAndSelectionInsideMarkers() throws {
        XCTAssertEqual(try result("**bold**", .italic), "***bold***")
        XCTAssertEqual(try result("***both***", .italic), "**both**")
        XCTAssertEqual(try result("***both***", .bold), "*both*")
        XCTAssertEqual(try result("A **word**.", .bold, range: NSRange(location: 4, length: 4)), "A word.")
        XCTAssertEqual(try result("**bold**", .italic, range: NSRange(location: 2, length: 4)), "***bold***")
    }
    func testEmptySelectionAndInvalidUnicodeBoundary() throws {
        let edit = try XCTUnwrap(FountainFormatting.edit(source: "Hi", selection: NSRange(location: 2, length: 0), style: .bold))
        XCTAssertEqual(edit.replacement, "****")
        XCTAssertEqual(edit.selection, NSRange(location: 4, length: 0))
        XCTAssertNil(FountainFormatting.edit(source: "🌙", selection: NSRange(location: 1, length: 1), style: .bold))
        XCTAssertNil(FountainFormatting.edit(source: " ", selection: NSRange(location: 0, length: 1), style: .bold))
    }
    func testFormattingFullScriptPreservesStructureAndProjectSource() throws {
        let source = "Title: Café\r\n\r\n# Act One\r\n\r\nINT. CAFÉ - DAY\r\n\r\n@Zoë\r\n(quietly)\r\nHello 🌙.\r\n\r\n> CUT TO:"
        let before = ManuscriptAnalysis.parse(source, format: .screenplay)
        for style in FountainFormatting.Style.allCases {
            let marked = try result(source, style)
            let after = ManuscriptAnalysis.parse(marked, format: .screenplay)
            XCTAssertEqual(after.headings.map(\.heading), before.headings.map(\.heading))
            XCTAssertEqual(after.headings.map(\.line), before.headings.map(\.line))
            XCTAssertEqual(after.elements.map(\.kind), before.elements.map(\.kind))
            XCTAssertEqual(after.characters, before.characters)
            XCTAssertEqual(after.locations, before.locations)
            var document = ScreenplayDocument()
            document.text = marked
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(marked.utf8))
            XCTAssertEqual(try result(marked, style), source)
        }
    }
    @MainActor
    func testNativeCommandUndoRedoAndNonScreenplayGuard() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let editor = ManuscriptTextView(frame: window.contentView!.bounds)
        window.contentView!.addSubview(editor)
        editor.allowsUndo = true
        editor.string = "Zoë"
        editor.setSelectedRange(NSRange(location: 0, length: 3))
        editor.fountainBold(nil)
        XCTAssertEqual(editor.string, "Zoë")
        editor.supportsFountainFormatting = true
        editor.undoManager?.beginUndoGrouping()
        editor.fountainBold(nil)
        editor.undoManager?.endUndoGrouping()
        XCTAssertEqual(editor.string, "**Zoë**")
        editor.undoManager?.undo()
        XCTAssertEqual(editor.string, "Zoë")
        editor.undoManager?.redo()
        XCTAssertEqual(editor.string, "**Zoë**")
    }
}
