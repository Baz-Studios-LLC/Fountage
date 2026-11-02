import XCTest
import AppKit
@testable import Fountage

final class TodoListTests: XCTestCase {
    func testToggleIsOneSourceCharacterAndRejectsStaleSnapshots() throws {
        let source = "# Work\r\n\r\n  [ ] Review Zoë 🌙 [[keep]]\r\n[x] Sent.\r\n/*\r\n[ ] hidden\r\n*/"
        let items = ManuscriptAnalysis.parse(source, format: .screenplay).todos
        XCTAssertEqual(items.count, 2)
        XCTAssertEqual(items[0].text, "Review Zoë 🌙")
        let edit = try TodoListEdit.toggle(source: source, expectedSource: source, item: items[0], format: .screenplay)
        XCTAssertEqual(edit.range.length, 1)
        let changed = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
        XCTAssertEqual(Data(changed.utf8), Data(source.replacingOccurrences(of: "  [ ]", with: "  [x]").utf8))
        XCTAssertThrowsError(try TodoListEdit.toggle(source: changed, expectedSource: source, item: items[0], format: .screenplay))
        XCTAssertThrowsError(try TodoListEdit.toggle(source: changed, expectedSource: changed, item: items[0], format: .screenplay))
        let done = ManuscriptAnalysis.parse(changed, format: .screenplay).todos[0]
        let reopen = try TodoListEdit.toggle(source: changed, expectedSource: changed, item: done, format: .screenplay)
        XCTAssertEqual(Data((changed as NSString).replacingCharacters(in: reopen.range, with: reopen.replacement).utf8), Data(source.utf8))
    }
    func testConversionPreservesIndentBlankLinesCRLFAndExistingTasks() throws {
        let source = "Before\r\n  First 🌙\r\n\r\n\tSecond\r\n[x] Done\r\nAfter"
        let selection = (source as NSString).range(of: "  First 🌙\r\n\r\n\tSecond\r\n[x] Done\r\n")
        let edit = try XCTUnwrap(TodoListEdit.convert(source: source, selection: selection))
        let changed = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
        XCTAssertEqual(Data(changed.utf8), Data("Before\r\n  [ ] First 🌙\r\n\r\n\t[ ] Second\r\n[x] Done\r\nAfter".utf8))
        XCTAssertNil(TodoListEdit.convert(source: changed, selection: edit.selection))
        XCTAssertNil(TodoListEdit.convert(source: "🌙", selection: NSRange(location: 1, length: 0)))
        XCTAssertNil(TodoListEdit.convert(source: source, selection: NSRange(location: NSNotFound, length: 0)))
        XCTAssertNil(TodoListEdit.convert(source: "\0", selection: NSRange(location: 0, length: 0)))
    }
    @MainActor func testNativeConversionUndoAndAllCategoryProjectReopen() throws {
        for format in DocumentFormat.allCases {
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false; defer { window.close() }
            let editor = ManuscriptTextView(frame: window.contentView!.bounds); window.contentView!.addSubview(editor); editor.allowsUndo = true
            let history = try XCTUnwrap(editor.undoManager); history.groupsByEvent = false
            editor.string = "Review Zoë 🌙\r\nSend draft"; editor.setSelectedRange(NSRange(location: 0, length: editor.string.utf16.count))
            history.beginUndoGrouping(); editor.convertToTodoList(nil); history.endUndoGrouping()
            XCTAssertEqual(editor.string, "[ ] Review Zoë 🌙\r\n[ ] Send draft")
            history.undo(); XCTAssertEqual(editor.string, "Review Zoë 🌙\r\nSend draft")
            history.redo()
            let item = try XCTUnwrap(ManuscriptAnalysis.parse(editor.string, format: format).todos.first)
            history.beginUndoGrouping(); try TodoListEdit.toggle(source: editor.string, expectedSource: editor.string, item: item, format: format).apply(to: editor); history.endUndoGrouping()
            XCTAssertTrue(editor.string.hasPrefix("[x]")); history.undo(); XCTAssertTrue(editor.string.hasPrefix("[ ]")); history.redo()
            var document = ScreenplayDocument(); document.format = format; document.text = editor.string; document.metadata["foreign"] = ["keep": true]
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(editor.string.utf8)); XCTAssertEqual(reopened.format, format)
            XCTAssertEqual(ManuscriptAnalysis.parse(reopened.text, format: format).todos.map(\.completed), [true, false])
            XCTAssertNotNil(reopened.metadata["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        }
    }
}
