import XCTest
import AppKit
@testable import Fountage

final class OutlineMoveTests: XCTestCase {
    func moved(_ source: String, line: Int, down: Bool, format: DocumentFormat = .screenplay) throws -> (String, ManuscriptEdit) {
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: format).headings)
        let edit = try XCTUnwrap(index.move(line: line, down: down, source: source))
        return ((source as NSString).replacingCharacters(in: edit.range, with: edit.replacement), edit)
    }
    func testScenesKeepNotesAndMissingFinalNewline() throws {
        for newline in ["\n", "\r\n"] {
            let prefix = "Title: Café" + newline + newline
            let a = "INT. ROOM - DAY" + newline + newline + "Zoë 🌙 [[First]]"
            let b = "EXT. PARK - NIGHT" + newline + newline + "[[Second]]"
            let source = prefix + a + newline + newline + b
            let (result, _) = try moved(source, line: 2, down: true)
            XCTAssertEqual(Data(result.utf8), Data((prefix + b + newline + newline + a).utf8))
            let analysis = ManuscriptAnalysis.parse(result, format: .screenplay)
            XCTAssertEqual(analysis.units, 2)
            XCTAssertEqual(analysis.notes.map(\.text), ["Second", "First"])
            for note in analysis.notes { XCTAssertEqual((result as NSString).substring(with: note.range), "[[\(note.text)]]") }
        }
    }
    func testSectionsMoveTheirEntireSubtreeAndRespectParents() throws {
        let a = "# One\n\nINT. A - DAY\n\n[[A]]"
        let b = "# Two\n\nINT. B - DAY\n\n[[B]]"
        let source = a + "\n\n" + b
        let (result, _) = try moved(source, line: 0, down: true)
        XCTAssertEqual(result, b + "\n\n" + a)
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: .screenplay).headings)
        XCTAssertNil(index.neighbor(of: 2, down: true))
        XCTAssertNil(index.neighbor(of: 0, down: false))
    }
    func testComicPagesMovePanelsAndPreserveMetadataFreeSource() throws {
        let a = "PAGE 1\n\nPanel 1\n\nMARA: Hello."
        let b = "PAGE 2\n\nPanel 1\n\nSFX: BOOM"
        let source = a + "\n\n" + b
        let (result, _) = try moved(source, line: 6, down: false, format: .comic)
        XCTAssertEqual(result, b + "\n\n" + a)
        XCTAssertEqual(ManuscriptAnalysis.parse(result, format: .comic).panels, 2)
    }
    @MainActor
    func testMoveHasOneNativeUndoAndRedoAndRejectsStaleSource() throws {
        let source = "INT. A - DAY\n\n[[A]]\n\nEXT. B - NIGHT\n\n[[B]]"
        let (expected, edit) = try moved(source, line: 0, down: true)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let editor = NSTextView(frame: window.contentView!.bounds)
        editor.allowsUndo = true
        window.contentView!.addSubview(editor)
        editor.string = source
        editor.undoManager?.beginUndoGrouping()
        XCTAssertTrue(edit.apply(to: editor))
        editor.undoManager?.endUndoGrouping()
        XCTAssertEqual(editor.string, expected)
        editor.undoManager?.undo()
        XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
        editor.undoManager?.redo()
        XCTAssertEqual(editor.string, expected)
        XCTAssertFalse(edit.apply(to: editor))
        XCTAssertEqual(editor.string, expected)
    }
}
