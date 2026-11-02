import XCTest
import AppKit
import PDFKit
@testable import Fountage

final class TitlePageEditTests: XCTestCase {
    private func updated(_ draft: TitlePageEdit, values: [String: String]) throws -> String {
        guard let edit = try draft.edit(values: values) else { return draft.source }
        return (draft.source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
    }
    func testInsertionMultilineUnicodeAndBodyBytes() throws {
        for ending in ["\n", "\r\n"] {
            let body = "INT. CAFÉ - DAY" + ending + ending + "@Zoë" + ending + "A signal 🌙."
            let draft = try TitlePageEdit(source: body)
            let result = try updated(draft, values: ["title": "_**THE SIGNAL**_", "author": "Zoë\nAnother writer", "contact": "Baz Studios\nwriter@example.com"])
            XCTAssertTrue(result.hasSuffix(body))
            let output = try ScreenplayOutput.parse(result)
            XCTAssertEqual(output.title_fields.first { $0.key == "title" }?.values.first?.text, "THE SIGNAL")
            XCTAssertEqual(output.title_fields.first { $0.key == "author" }?.values.map(\.text), ["Zoë", "Another writer"], "Source: \(result.debugDescription); title lines: \(output.title.map(\.text))")
            XCTAssertEqual(output.body.filter { $0.kind == .dialogue }.map(\.text), ["A signal 🌙."])
            XCTAssertNil(try TitlePageEdit(source: result).edit(values: TitlePageEdit(source: result).original))
        }
    }
    func testUntouchedMetadataAndDuplicateFieldsAreExact() throws {
        let source = "Title: **Original**  \r\nCustom Field: Cafe\u{301} 🌙  \r\nAuthors:\r\n\tFirst\r\nAuthor: Second\r\nContact: Kept  \r\n\r\n!Body.\r\n"
        let draft = try TitlePageEdit(source: source)
        XCTAssertEqual(draft.original["author"], "First\nSecond")
        XCTAssertNil(try draft.edit(values: draft.original))
        var values = draft.original; values["title"] = "New"
        let changed = try updated(draft, values: values)
        XCTAssertEqual(Data(changed.utf8), Data(source.replacingOccurrences(of: "Title: **Original**  ", with: "Title: New").utf8))
        values["author"] = "First\nSecond\nThird"
        let authors = try updated(draft, values: values)
        XCTAssertTrue(authors.contains("Custom Field: Cafe\u{301} 🌙  \r\n"))
        XCTAssertEqual(try ScreenplayOutput.parse(authors).title_fields.filter { ["author", "authors"].contains($0.key) }.flatMap(\.values).map(\.text), ["First", "Second", "Third"])
    }
    func testRemovalRetainsCustomMetadataAndRecognizableHeader() throws {
        let source = "Title: Remove\nCustom Field: Private metadata\nAuthor: Keep\n\n!Body."
        let draft = try TitlePageEdit(source: source)
        var values = draft.original; values["title"] = ""
        let changed = try updated(draft, values: values)
        let output = try ScreenplayOutput.parse(changed)
        XCTAssertEqual(output.title_fields.first { $0.key == "custom field" }?.values.first?.text, "Private metadata")
        XCTAssertFalse(output.body.contains { $0.text.contains("Private metadata") })
        XCTAssertTrue(changed.hasSuffix("\n\n!Body."))
    }
    func testEOFAndEmptyDraftCanGainFieldsWithoutFusingLines() throws {
        let source = "Title: Original"
        let draft = try TitlePageEdit(source: source)
        var values = draft.original; values["author"] = "Zoë"
        XCTAssertEqual(try updated(draft, values: values), "Title: Original\nAuthor: Zoë\n")
        let empty = try TitlePageEdit(source: "")
        XCTAssertNil(try empty.edit(values: [:]))
        XCTAssertEqual(try updated(empty, values: ["title": "First idea"]), "Title: First idea\n\n")
    }
    func testUnsafeFieldInputAndComplexHeaderAreRejected() throws {
        let draft = try TitlePageEdit(source: "!Body.")
        XCTAssertThrowsError(try draft.edit(values: ["title": "One\n\nTwo"]))
        XCTAssertThrowsError(try draft.edit(values: ["title": "One\0Two"]))
        XCTAssertThrowsError(try TitlePageEdit(source: "**Title:** Styled key\n\nBody."))
        XCTAssertThrowsError(try TitlePageEdit(source: "Title: Original\n[[Note]]\nAuthor: Writer\n\nBody."))
    }
    @MainActor func testNativeUndoRedoStaleGuardAndProjectReopen() throws {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let editor = ManuscriptTextView(frame: window.contentView!.bounds)
        window.contentView!.addSubview(editor); editor.allowsUndo = true
        let source = "INT. ROOM - DAY\r\n\r\nA signal 🌙."
        editor.string = source
        let draft = try TitlePageEdit(source: source)
        let edit = try XCTUnwrap(draft.edit(values: ["title": "THE SIGNAL", "credit": "Written by", "author": "Zoë"]))
        editor.undoManager?.beginUndoGrouping(); XCTAssertTrue(edit.apply(to: editor)); editor.undoManager?.endUndoGrouping()
        let changed = editor.string
        editor.undoManager?.undo(); XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
        editor.undoManager?.redo(); XCTAssertEqual(Data(editor.string.utf8), Data(changed.utf8))
        XCTAssertFalse(edit.apply(to: editor))
        var document = ScreenplayDocument(); document.text = changed
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let attachment = FileWrapper(regularFileWithContents: Data([0, 1, 255])); attachment.preferredFilename = "private.bin"; wrapper.addFileWrapper(attachment)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(changed.utf8))
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 1, 255]))
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(reopened.text))
        let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
        XCTAssertEqual(pdf.pageCount, 2)
        XCTAssertTrue(pdf.page(at: 0)?.string?.contains("THE SIGNAL") ?? false)
        XCTAssertTrue(pdf.page(at: 1)?.string?.contains("A signal") ?? false)
    }
}
