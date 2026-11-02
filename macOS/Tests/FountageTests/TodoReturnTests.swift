import XCTest
import AppKit
@testable import Fountage

final class TodoReturnTests: XCTestCase {
    func testContinuationSplittingAndEmptyExitPreserveExactLineEndings() throws {
        for ending in ["\n", "\r\n"] {
            for state in [" ", "x", "X"] {
                let source = "# Work\(ending)\(ending)\t[\(state)] Zoë 🌙 waits."
                let position = (source as NSString).range(of: " waits.").location
                let split = try XCTUnwrap(TodoListEdit.newline(source: source, selection: NSRange(location: position, length: 0), format: .screenplay))
                let changed = (source as NSString).replacingCharacters(in: split.range, with: split.replacement)
                XCTAssertEqual(Data(changed.utf8), Data("# Work\(ending)\(ending)\t[\(state)] Zoë 🌙\(ending)\t[ ]  waits.".utf8))
                let continued = try XCTUnwrap(TodoListEdit.newline(source: source, selection: NSRange(location: source.utf16.count, length: 0), format: .screenplay))
                let next = (source as NSString).replacingCharacters(in: continued.range, with: continued.replacement)
                let exit = try XCTUnwrap(TodoListEdit.newline(source: next, selection: continued.selection, format: .screenplay))
                XCTAssertEqual(Data((next as NSString).replacingCharacters(in: exit.range, with: exit.replacement).utf8), Data((source + ending + ending).utf8))
            }
        }
        let mixed = "Before\n\t[ ] Task\r\nAfter"
        let caret = (mixed as NSString).range(of: "Task").location + 4
        XCTAssertEqual(try XCTUnwrap(TodoListEdit.newline(source: mixed, selection: NSRange(location: caret, length: 0), format: .general)).replacement, "\r\n\t[ ] ")
    }
    func testNonTasksSelectionsAndHiddenAnnotationsUseOrdinaryReturn() throws {
        for source in ["Normal prose.", "/*\n[ ] Hidden task\n*/", "[[\n[ ] Hidden task\n]]", "\\[ ] Literal", "[?] Not a task"] {
            XCTAssertNil(TodoListEdit.newline(source: source, selection: NSRange(location: source.utf16.count, length: 0), format: .screenplay))
        }
        let source = "[ ] Task [[private]] {{%m: name}}"
        for word in ["private", "name"] {
            XCTAssertNil(TodoListEdit.newline(source: source, selection: NSRange(location: (source as NSString).range(of: word).location + 1, length: 0), format: .screenplay))
        }
        XCTAssertNil(TodoListEdit.newline(source: source, selection: NSRange(location: 4, length: 4), format: .screenplay))
        XCTAssertNil(TodoListEdit.newline(source: source, selection: NSRange(location: NSNotFound, length: 0), format: .screenplay))
        XCTAssertNil(TodoListEdit.newline(source: "[ ] 🌙", selection: NSRange(location: 5, length: 0), format: .screenplay))
        // A task containing only an annotation is not empty source: do not erase it.
        let annotated = "[ ] [[keep]]"
        let edit = try XCTUnwrap(TodoListEdit.newline(source: annotated, selection: NSRange(location: annotated.utf16.count, length: 0), format: .screenplay))
        XCTAssertEqual(edit.range.length, 0)
        XCTAssertTrue((annotated as NSString).replacingCharacters(in: edit.range, with: edit.replacement).contains("[[keep]]"))
    }
    @MainActor func testActualTextViewReturnUndoRedoAndReopenAcrossEightCategories() throws {
        for format in DocumentFormat.allCases {
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false; defer { window.close() }
            let editor = ManuscriptTextView(frame: window.contentView!.bounds); window.contentView!.addSubview(editor)
            editor.allowsUndo = true; editor.manuscriptFormat = format
            let history = try XCTUnwrap(editor.undoManager); history.groupsByEvent = false
            let source = "[x] Review Zoë 🌙"; editor.string = source
            editor.setSelectedRange(NSRange(location: source.utf16.count, length: 0))
            history.beginUndoGrouping(); editor.insertNewline(nil); history.endUndoGrouping()
            XCTAssertEqual(editor.string, source + "\n[ ] ")
            history.undo(); XCTAssertEqual(editor.string, source); history.redo()
            history.beginUndoGrouping(); editor.insertNewline(nil); history.endUndoGrouping()
            XCTAssertEqual(editor.string, source + "\n\n")
            history.undo(); XCTAssertEqual(editor.string, source + "\n[ ] "); history.redo()
            var document = ScreenplayDocument(); document.format = format; document.text = editor.string; document.metadata["foreign"] = ["keep": true]
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(reopened.format, format); XCTAssertEqual(Data(reopened.text.utf8), Data(editor.string.utf8))
            XCTAssertNotNil(reopened.metadata["foreign"])
            let tasks = ManuscriptAnalysis.parse(reopened.text, format: format).todos
            XCTAssertEqual(tasks.count, 1); XCTAssertTrue(tasks[0].completed)
        }
    }
}
