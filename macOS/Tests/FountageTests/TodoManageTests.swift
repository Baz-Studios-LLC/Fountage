import XCTest
import AppKit
@testable import Fountage

final class TodoManageTests: XCTestCase {
    func testAppendPreservesSourceAndLineEndingsAndRejectsHiddenOrStaleInput() throws {
        for (source, suffix) in [("", "[ ] Zoë 🌙\n"), ("Body", "\n\n[ ] Zoë 🌙\n"), ("Body\r\n", "\r\n[ ] Zoë 🌙\r\n"), ("Body\r\n\r\n", "[ ] Zoë 🌙\r\n")] {
            let edit = try TodoListEdit.append(source: source, expectedSource: source, taskText: "  Zoë 🌙  ", format: .screenplay)
            let result = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
            XCTAssertEqual(Data(result.utf8), Data((source + suffix).utf8))
            XCTAssertEqual((result as NSString).substring(with: edit.selection), "[ ] Zoë 🌙")
        }
        for text in ["", " \t", "a\nb", "a\rb", "a\u{2028}b", "a\0b", "Hidden [[note]]", "/* hidden */"] {
            XCTAssertThrowsError(try TodoListEdit.append(source: "", expectedSource: "", taskText: text, format: .screenplay))
        }
        for format in DocumentFormat.allCases {
            for source in ["/* unfinished"] {
                XCTAssertThrowsError(try TodoListEdit.append(source: source, expectedSource: source, taskText: "Next", format: format))
            }
        }
        XCTAssertThrowsError(try TodoListEdit.append(source: "changed", expectedSource: "", taskText: "Next", format: .screenplay))
    }
    func testRemoveDeletesOnlyTaskLineAndProtectsMultilineAnnotations() throws {
        for ending in ["\n", "\r\n"] {
            let source = "Before 🌙" + ending + "  [X] Zoë [[inline]]" + ending + "After" + ending
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(source, format: .screenplay).todos.first)
            let edit = try TodoListEdit.remove(source: source, expectedSource: source, item: task, format: .screenplay)
            XCTAssertEqual(Data((source as NSString).replacingCharacters(in: edit.range, with: edit.replacement).utf8), Data(("Before 🌙" + ending + "After" + ending).utf8))
            XCTAssertThrowsError(try TodoListEdit.remove(source: source + "changed", expectedSource: source, item: task, format: .screenplay))
        }
        for format in DocumentFormat.allCases {
            for suffix in ["[[note\ncontinued]]", "/* omitted\ncontinued */"] {
                let source = "[ ] Keep " + suffix + "\nAfter"
                let task = try XCTUnwrap(ManuscriptAnalysis.parse(source, format: format).todos.first)
                XCTAssertThrowsError(try TodoListEdit.remove(source: source, expectedSource: source, item: task, format: format))
            }
        }
        let source = "[ ] Final task"
        let task = try XCTUnwrap(ManuscriptAnalysis.parse(source, format: .screenplay).todos.first)
        let edit = try TodoListEdit.remove(source: source, expectedSource: source, item: task, format: .screenplay)
        XCTAssertEqual((source as NSString).replacingCharacters(in: edit.range, with: edit.replacement), "")
    }
    @MainActor func testAddRemoveNativeUndoAndProjectReopenForAllCategories() throws {
        for format in DocumentFormat.allCases {
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false; defer { window.close() }
            let editor = ManuscriptTextView(frame: window.contentView!.bounds); window.contentView!.addSubview(editor); editor.allowsUndo = true
            let history = try XCTUnwrap(editor.undoManager); history.groupsByEvent = false
            let original = "Original 🌙\r\n"; editor.string = original
            history.beginUndoGrouping()
            XCTAssertTrue(try TodoListEdit.append(source: original, expectedSource: original, taskText: "Review Zoë", format: format).apply(to: editor))
            history.endUndoGrouping()
            let added = editor.string
            history.undo(); XCTAssertEqual(editor.string, original); history.redo(); XCTAssertEqual(editor.string, added)
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(added, format: format).todos.first)
            history.beginUndoGrouping()
            XCTAssertTrue(try TodoListEdit.remove(source: added, expectedSource: added, item: task, format: format).apply(to: editor))
            history.endUndoGrouping()
            XCTAssertEqual(editor.string, original + "\r\n")
            history.undo(); XCTAssertEqual(editor.string, added)
            var document = ScreenplayDocument(); document.format = format; document.text = editor.string
            document.metadata["foreign"] = ["keep": true]
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(added.utf8)); XCTAssertNotNil(reopened.metadata["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        }
    }
}
