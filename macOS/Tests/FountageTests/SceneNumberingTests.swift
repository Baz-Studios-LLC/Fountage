import XCTest
import AppKit
import PDFKit
import CoreText
@testable import Fountage

final class SceneNumberingTests: XCTestCase {
    func changed(_ source: String, remove: Bool, first: UInt32 = 1) throws -> String {
        guard let edit = try SceneNumbering.edit(source: source, selection: NSRange(location: source.utf16.count, length: 0), remove: remove, firstNumber: first) else { return source }
        return (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
    }
    func testNumberAndRemoveKeepSourceExactAndNoOpIsIdempotent() throws {
        for newline in ["\n", "\r\n"] {
            let source = "Title: Cafe\u{301}\(newline)Custom Field: Keep\(newline)\(newline)**INT. CAFÉ - DAY** [[private]]\(newline)\(newline)Action 🌙.\(newline)\(newline).The roof\(newline)\(newline)/* INT. OMITTED - DAY */"
            let numbered = try changed(source, remove: false)
            XCTAssertTrue(numbered.contains("**INT. CAFÉ - DAY #1#** [[private]]"))
            XCTAssertTrue(numbered.contains(".The roof #2#"))
            XCTAssertFalse(numbered.contains("OMITTED - DAY #"))
            XCTAssertNil(try SceneNumbering.edit(source: numbered, selection: NSRange(location: 0, length: 0), remove: false))
            XCTAssertEqual(Data(try changed(numbered, remove: true).utf8), Data(source.utf8))
            let custom = try changed(source, remove: false, first: 42)
            XCTAssertEqual(try ScreenplayOutput.parse(custom).body.filter { $0.kind == .scene }.compactMap(\.scene_number), ["42", "43"])
        }
    }
    func testSelectionRemainsAtTheSameBodyTextAfterChanges() throws {
        let source = "INT. ROOM - DAY\n\nZoë 🌙 waits.\n\nEXT. ROOF - NIGHT"
        let selected = (source as NSString).range(of: "Zoë 🌙")
        let edit = try XCTUnwrap(SceneNumbering.edit(source: source, selection: selected, remove: false))
        let numbered = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
        XCTAssertEqual((numbered as NSString).substring(with: edit.selection), "Zoë 🌙")
        let full = try XCTUnwrap(SceneNumbering.edit(source: source, selection: NSRange(location: 0, length: source.utf16.count), remove: false))
        XCTAssertEqual(full.selection, NSRange(location: 0, length: numbered.utf16.count))
        let removed = try XCTUnwrap(SceneNumbering.edit(source: numbered, selection: edit.selection, remove: true))
        let plain = (numbered as NSString).replacingCharacters(in: removed.range, with: removed.replacement)
        XCTAssertEqual((plain as NSString).substring(with: removed.selection), "Zoë 🌙")
    }
    func testUnsafeNumberMarkupAndInvalidInputNeverReturnAPartialEdit() throws {
        let source = "INT. ROOM - DAY\n\nAction.\n\nEXT. ROOF - NIGHT #**12A**#"
        XCTAssertThrowsError(try SceneNumbering.edit(source: source, selection: NSRange(location: 0, length: 0), remove: false))
        XCTAssertThrowsError(try SceneNumbering.edit(source: "Before\0After", selection: NSRange(location: 0, length: 0), remove: false))
        XCTAssertThrowsError(try SceneNumbering.edit(source: source, selection: NSRange(location: 0, length: 0), remove: false, firstNumber: UInt32.max))
        XCTAssertNil(try SceneNumbering.edit(source: "Action.", selection: NSRange(location: 0, length: 0), remove: true))
    }
    @MainActor func testNativeUndoRedoFormatGuardAndProjectAttachments() throws {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        let editor = ManuscriptTextView(frame: window.contentView!.bounds)
        window.contentView!.addSubview(editor); editor.allowsUndo = true
        let source = "INT. CAFÉ - DAY #A1# [[keep]]\r\n\r\nZoë 🌙 waits.\r\n\r\nEXT. ROOF - NIGHT"
        editor.string = source
        editor.addSceneNumbers(nil); XCTAssertEqual(editor.string, source)
        editor.supportsFountainFormatting = true
        editor.undoManager?.beginUndoGrouping(); editor.addSceneNumbers(nil); editor.undoManager?.endUndoGrouping()
        let numbered = editor.string
        XCTAssertTrue(numbered.contains("#1# [[keep]]")); XCTAssertTrue(numbered.hasSuffix("#2#"))
        editor.undoManager?.undo(); XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
        editor.undoManager?.redo(); XCTAssertEqual(Data(editor.string.utf8), Data(numbered.utf8))
        editor.undoManager?.beginUndoGrouping(); editor.removeSceneNumbers(nil); editor.undoManager?.endUndoGrouping()
        XCTAssertFalse(editor.string.contains("#1#")); XCTAssertTrue(editor.string.contains("[[keep]]"))
        editor.undoManager?.undo(); XCTAssertEqual(Data(editor.string.utf8), Data(numbered.utf8))
        var document = ScreenplayDocument(); document.text = numbered
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let attachment = FileWrapper(regularFileWithContents: Data([0, 255, 1])); attachment.preferredFilename = "private.bin"; wrapper.addFileWrapper(attachment)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(numbered.utf8))
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255, 1]))
    }
    func testSceneNumbersPrintOnBothSidesOfEachPaper() throws {
        let source = "INT. OBSERVATORY - NIGHT #12A# [[private]]\n\nA signal."
        for paper in ScreenplayPrintOptions.Paper.allCases {
            var options = ScreenplayPrintOptions(); options.paper = paper
            let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source), options: options)
            let labels = layout.pages[0].fragments.filter { $0.text.string == "12A" }
            XCTAssertEqual(labels.count, 2)
            XCTAssertEqual(labels[0].top, labels[1].top)
            XCTAssertLessThan(labels[0].x, 108)
            XCTAssertGreaterThan(labels[1].x, paper.size.width - 72)
            let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
            XCTAssertEqual(pdf.string?.components(separatedBy: "12A").count, 3)
        }
        if let path = ProcessInfo.processInfo.environment["FOUNTAGE_NUMBERING_QA_PATH"] {
            let numbered = try changed("Title: THE LAST SIGNAL\n\nINT. OBSERVATORY - NIGHT [[private]]\n\nThe clock stops.\n\n.The roof\n\nA signal flickers.", remove: false)
            try ScreenplayPagination(output: ScreenplayOutput.parse(numbered)).pdfData().write(to: URL(fileURLWithPath: path), options: .atomic)
        }
    }
    func testLongCustomNumberDoesNotOverlapTheHeadingOrClipThePage() throws {
        let source = "INT. ROOM - DAY #1234567890#\n\nAction."
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source))
        let labels = layout.pages[0].fragments.filter { $0.text.string == "1234567890" }
        XCTAssertEqual(labels.count, 2)
        for label in labels {
            let width = CTLineGetTypographicBounds(CTLineCreateWithAttributedString(label.text), nil, nil, nil)
            XCTAssertLessThanOrEqual(width, 30.01)
            XCTAssertLessThan(label.x + width, ScreenplayPagination.pageSize.width)
        }
    }

    func testSelectedRangeUsesWholeDocumentContextAndPreservesOutsideNumbers() throws {
        for nl in ["\n", "\r\n"] {
            let source = "INT. FIRST - DAY #A#\(nl)\(nl)Zoë 🌙.\(nl)\(nl)**EXT. SECOND - NIGHT #B#** [[keep]]\(nl)\(nl)/*\(nl)INT. OMITTED - DAY\(nl)*/\(nl)\(nl).Third\(nl)\(nl)EXT. LAST - DAY #Z#"
            let begin = (source as NSString).range(of: "SECOND").location
            let end = NSMaxRange((source as NSString).range(of: ".Third"))
            let selection = NSRange(location: begin, length: end - begin)
            let edit = try XCTUnwrap(SceneNumbering.edit(source: source, selection: selection, remove: false, firstNumber: 42, selectedOnly: true))
            let changed = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
            XCTAssertEqual(changed, source.replacingOccurrences(of: "#B#", with: "#42#").replacingOccurrences(of: ".Third", with: ".Third #43#"))
            let removal = try XCTUnwrap(SceneNumbering.edit(source: changed, selection: edit.selection, remove: true, selectedOnly: true))
            let removed = (changed as NSString).replacingCharacters(in: removal.range, with: removal.replacement)
            XCTAssertEqual(removed, source.replacingOccurrences(of: " #B#", with: ""))
            XCTAssertThrowsError(try SceneNumbering.edit(source: source, selection: NSRange(location: begin, length: 0), remove: false, selectedOnly: true))
            let action = (source as NSString).range(of: "Zoë 🌙.")
            XCTAssertNil(try SceneNumbering.edit(source: source, selection: action, remove: false, selectedOnly: true))
        }
    }

    @MainActor func testRangeEditNativeUndoAndSaveReopen() throws {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        let editor = ManuscriptTextView(frame: window.contentView!.bounds)
        window.contentView!.addSubview(editor); editor.allowsUndo = true; editor.supportsFountainFormatting = true
        let source = "INT. FIRST - DAY #A#\n\nEXT. SECOND - NIGHT #B# [[keep]]\n\n.The roof\n\nEXT. LAST - DAY #Z#"
        editor.string = source
        let begin = (source as NSString).range(of: "SECOND").location
        let end = NSMaxRange((source as NSString).range(of: ".The roof"))
        let edit = try XCTUnwrap(SceneNumbering.edit(source: source, selection: NSRange(location: begin, length: end-begin), remove: false, firstNumber: 42, selectedOnly: true))
        editor.undoManager?.beginUndoGrouping(); try edit.apply(to: editor); editor.undoManager?.endUndoGrouping()
        let numbered = editor.string
        XCTAssertTrue(numbered.contains("#42# [[keep]]")); XCTAssertTrue(numbered.contains(".The roof #43#"))
        editor.undoManager?.undo(); XCTAssertEqual(editor.string, source)
        editor.undoManager?.redo(); XCTAssertEqual(editor.string, numbered)
        editor.setSelectedRange(edit.selection)
        editor.undoManager?.beginUndoGrouping(); editor.removeSelectedSceneNumbers(nil); editor.undoManager?.endUndoGrouping()
        XCTAssertEqual(editor.string, source.replacingOccurrences(of: " #B#", with: ""))
        editor.undoManager?.undo(); XCTAssertEqual(editor.string, numbered)
        var document = ScreenplayDocument(); document.text = numbered
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(numbered.utf8))
    }

}
