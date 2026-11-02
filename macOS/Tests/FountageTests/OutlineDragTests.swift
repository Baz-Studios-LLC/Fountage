import XCTest
import AppKit
@testable import Fountage

final class OutlineDragTests: XCTestCase {
    func apply(_ edit: ManuscriptEdit, to source: String) -> String { (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement) }
    func testEveryFlatSceneInsertionKeepsExactBodiesAndFinalNewlinePolicy() throws {
        for nl in ["\n", "\r\n"] {
            let prefix = "Title: Zoë" + nl + nl
            let bodies = (0..<5).map { "INT. ROOM \($0) - DAY" + nl + nl + "Action 🌙 \($0). [[Note \($0)]]" }
            let separators = [nl+nl, nl+nl+nl, nl+nl, nl+nl, ""]
            let source = prefix + bodies.indices.map { bodies[$0] + separators[$0] }.joined()
            let analysis = ManuscriptAnalysis.parse(source, format: .screenplay)
            let scenes = analysis.headings.filter { $0.kind == .scene }
            XCTAssertEqual(scenes.count, 5)
            let index = OutlineIndex(analysis.headings)
            for from in 0..<5 { for target in 0..<5 { for after in [false, true] {
                var order = Array(0..<5); order.remove(at: from)
                var destination = target + (after ? 1 : 0)
                if destination > from { destination -= 1 }
                if from == target { destination = from }
                order.insert(from, at: destination)
                let edit = index.move(line: scenes[from].line, to: scenes[target].line, after: after, source: source)
                if order == Array(0..<5) { XCTAssertNil(edit); continue }
                let change = try XCTUnwrap(edit)
                let moved = apply(change, to: source)
                let expected = prefix + order.indices.map { bodies[order[$0]] + separators[$0] }.joined()
                XCTAssertEqual(Data(moved.utf8), Data(expected.utf8))
                XCTAssertTrue((moved as NSString).substring(from: change.selection.location).hasPrefix("INT. ROOM \(from)"))
                XCTAssertEqual(ManuscriptAnalysis.parse(moved, format: .screenplay).notes.map(\.text), order.map { "Note \($0)" })
            } } }
        }
    }
    func testCrossSectionMoveAndSubtreeCycleRejection() throws {
        let source = "# One\n\nINT. A - DAY\n\n[[A]]\n\nINT. B - DAY\n\n[[B]]\n\n# Two\n\nEXT. C - NIGHT\n\n[[C]]"
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: .screenplay).headings)
        let a = try XCTUnwrap(index.rows.first { $0.heading.heading == "INT. A - DAY" })
        let c = try XCTUnwrap(index.rows.first { $0.heading.heading == "EXT. C - NIGHT" })
        let moved = apply(try XCTUnwrap(index.move(line: a.heading.line, to: c.heading.line, after: false, source: source)), to: source)
        XCTAssertEqual(moved, "# One\n\nINT. B - DAY\n\n[[B]]\n\n# Two\n\nINT. A - DAY\n\n[[A]]\n\nEXT. C - NIGHT\n\n[[C]]")
        XCTAssertNil(index.move(line: 0, to: a.heading.line, after: false, source: source))
        let sectionTwo = try XCTUnwrap(index.rows.first { $0.heading.heading == "Two" })
        let swapped = apply(try XCTUnwrap(index.move(line: 0, to: sectionTwo.heading.line, after: true, source: source)), to: source)
        XCTAssertTrue(swapped.hasPrefix("# Two\n\nEXT. C"))
        XCTAssertTrue(swapped.hasSuffix("[[B]]"))
    }
    func testTokenRejectsForeignStaleNullAndChangedFormatWithoutReturningEdit() throws {
        let source = "INT. A - DAY\n\nA 🌙.\n\nEXT. B - NIGHT\n\nB."
        let drag = OutlineDragSession(line: 0, source: source, format: .screenplay)
        XCTAssertNotNil(drag.edit(token: drag.token, currentSource: source, currentFormat: .screenplay, targetLine: 4, after: true))
        XCTAssertNil(drag.edit(token: Data("foreign".utf8), currentSource: source, currentFormat: .screenplay, targetLine: 4, after: true))
        XCTAssertNil(drag.edit(token: drag.token, currentSource: source + "New edit.", currentFormat: .screenplay, targetLine: 4, after: true))
        XCTAssertNil(drag.edit(token: drag.token, currentSource: source, currentFormat: .comic, targetLine: 4, after: true))
        let foreign = OutlineDragSession(line: 0, source: source, format: .screenplay)
        XCTAssertNil(drag.edit(token: foreign.token, currentSource: source, currentFormat: .screenplay, targetLine: 4, after: true))
        let invalid = OutlineDragSession(line: 0, source: source + "\0", format: .screenplay)
        XCTAssertNil(invalid.edit(token: invalid.token, currentSource: source + "\0", currentFormat: .screenplay, targetLine: 4, after: true))
        XCTAssertFalse(String(data: drag.token, encoding: .utf8)!.contains("INT."))
    }
    @MainActor func testDragMoveNativeUndoRedoAndProjectAttachments() throws {
        let source = "Title: Zoë\r\n\r\nINT. A - DAY\r\n\r\n[[Keep A]]\r\n\r\nEXT. B - NIGHT\r\n\r\n[[Keep B]]"
        let drag = OutlineDragSession(line: 2, source: source, format: .screenplay)
        let edit = try XCTUnwrap(drag.edit(token: drag.token, currentSource: source, currentFormat: .screenplay, targetLine: 6, after: true))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        let editor = NSTextView(frame: window.contentView!.bounds); editor.allowsUndo = true; window.contentView!.addSubview(editor); editor.string = source
        editor.undoManager?.beginUndoGrouping(); XCTAssertTrue(edit.apply(to: editor)); editor.undoManager?.endUndoGrouping()
        let moved = editor.string
        editor.undoManager?.undo(); XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
        editor.undoManager?.redo(); XCTAssertEqual(Data(editor.string.utf8), Data(moved.utf8))
        var document = ScreenplayDocument(); document.text = moved
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let file = FileWrapper(regularFileWithContents: Data([0, 255, 7])); file.preferredFilename = "keep.bin"; wrapper.addFileWrapper(file)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(moved.utf8))
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["keep.bin"]?.regularFileContents, Data([0, 255, 7]))
    }
    func testProviderTransfersOnlyTheOpaqueToken() {
        let drag = OutlineDragSession(line: 0, source: "INT. A - DAY", format: .screenplay)
        let done = expectation(description: "local drag representation")
        drag.provider().loadDataRepresentation(forTypeIdentifier: OutlineDragSession.type.identifier) { data, error in
            XCTAssertNil(error); XCTAssertEqual(data, drag.token); done.fulfill()
        }
        wait(for: [done], timeout: 3)
    }

    func testMoveDialogDestinationsAndStaleSnapshot() throws {
        let source = "# One\n\nINT. A - DAY\n\nA.\n\n# Two\n\nEXT. B - NIGHT\n\nB."
        let request = OutlineMoveRequest(line: 2, source: source, format: .screenplay)
        XCTAssertTrue(request.destinations(after: false).contains { $0.heading.line == 8 })
        XCTAssertFalse(request.destinations(after: false).contains { $0.heading.line == 2 })
        XCTAssertNotNil(request.edit(target: 8, after: false, currentSource: source, currentFormat: .screenplay))
        XCTAssertNil(request.edit(target: 8, after: false, currentSource: source + "Edited.", currentFormat: .screenplay))
        XCTAssertNil(request.edit(target: 8, after: false, currentSource: source, currentFormat: .comic))
    }

    @MainActor func testDropReadsLiveStoreWithoutWaitingForViewRefreshAndRejectsCanceledSession() throws {
        let source = "INT. A - DAY\n\nA.\n\nEXT. B - NIGHT\n\nB."
        let store = OutlineDragStore()
        let drag = OutlineDragSession(line: 0, source: source, format: .screenplay)
        let reference = store
        store.begin(drag)
        XCTAssertEqual(reference.session?.id, drag.id)
        XCTAssertNotNil(reference.resolve(drag, token: drag.token, source: source, format: .screenplay, target: 4, after: true))
        store.cancel()
        XCTAssertNil(reference.resolve(drag, token: drag.token, source: source, format: .screenplay, target: 4, after: true))
        let replacement = OutlineDragSession(line: 0, source: source, format: .screenplay)
        store.begin(replacement)
        store.cancel(id: drag.id)
        XCTAssertEqual(store.session?.id, replacement.id)
        XCTAssertNil(reference.resolve(drag, token: drag.token, source: source, format: .screenplay, target: 4, after: true))
        XCTAssertTrue(OutlineDragHandleView().acceptsFirstMouse(for: nil))
    }

}
