import XCTest
import AppKit
@testable import Fountage

final class TodoTextEditTests: XCTestCase {
    func testTaskSourceEditingRetainsPrefixStateInlineMarkupAndSurroundings() throws {
        for format in DocumentFormat.allCases {
            let source = "Before 🌙\r\n\t[X]\tReview **Zoë** [[private]] {{%m: next}}\r\n[ ] Later\r\n"
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(source, format: format).todos.first)
            let raw = TodoListEdit.taskContent(source: source, item: task)
            XCTAssertEqual(raw, "Review **Zoë** [[private]] {{%m: next}}")
            XCTAssertNil(try TodoListEdit.update(source: source, expectedSource: source, item: task, content: raw, format: format))
            let edit = try XCTUnwrap(TodoListEdit.update(source: source, expectedSource: source, item: task, content: raw.replacingOccurrences(of: "Review", with: "Revise 🌙"), format: format))
            let changed = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
            XCTAssertEqual(Data(changed.utf8), Data(source.replacingOccurrences(of: "Review", with: "Revise 🌙").utf8))
            XCTAssertEqual(ManuscriptAnalysis.parse(changed, format: format).todos.map(\.completed), [true, false])
            XCTAssertEqual((changed as NSString).substring(with: edit.selection), "\t[X]\tRevise 🌙 **Zoë** [[private]] {{%m: next}}")
            XCTAssertThrowsError(try TodoListEdit.update(source: changed, expectedSource: source, item: task, content: "New", format: format))
        }
    }
    func testEmptyTaskAndInvalidOrCrossLineMarkup() throws {
        for (source, expected) in [("[ ]", "[ ] Next"), ("  [X]  ", "  [X]  Next")] {
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(source, format: .screenplay).todos.first)
            let edit = try XCTUnwrap(TodoListEdit.update(source: source, expectedSource: source, item: task, content: "Next", format: .screenplay))
            XCTAssertEqual((source as NSString).replacingCharacters(in: edit.range, with: edit.replacement), expected)
        }
        for source in ["[ ] Work [[private\ncontinued]]\n[ ] Later", "[ ] Work /*private\ncontinued*/\n[ ] Later"] {
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(source, format: .screenplay).todos.first)
            XCTAssertThrowsError(try TodoListEdit.update(source: source, expectedSource: source, item: task, content: "New", format: .screenplay))
        }
        let source = "[ ] Work\r\n[ ] Later"
        let task = try XCTUnwrap(ManuscriptAnalysis.parse(source, format: .screenplay).todos.first)
        for content in ["New\nline", "New\rline", "New\0line", "Work /* unfinished"] {
            XCTAssertThrowsError(try TodoListEdit.update(source: source, expectedSource: source, item: task, content: content, format: .screenplay))
        }
    }
    @MainActor func testNativeUndoRedoAndProjectPreservation() throws {
        for format in DocumentFormat.allCases {
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false; defer { window.close() }
            let editor = ManuscriptTextView(frame: window.contentView!.bounds); window.contentView!.addSubview(editor); editor.allowsUndo = true
            let history = try XCTUnwrap(editor.undoManager); history.groupsByEvent = false
            let original = "[X] Review Zoë [[note]]\r\n"; editor.string = original
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(original, format: format).todos.first)
            history.beginUndoGrouping()
            XCTAssertTrue(try XCTUnwrap(TodoListEdit.update(source: original, expectedSource: original, item: task, content: "Revise 🌙 [[note]]", format: format)).apply(to: editor))
            history.endUndoGrouping()
            let changed = editor.string
            history.undo(); XCTAssertEqual(editor.string, original); history.redo(); XCTAssertEqual(editor.string, changed)
            var document = ScreenplayDocument(); document.format = format; document.text = changed; document.metadata["foreign"] = ["keep": true]
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(changed.utf8)); XCTAssertNotNil(reopened.metadata["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        }
    }
}
