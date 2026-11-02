import XCTest
import AppKit
@testable import Fountage

final class ScriptMarkerTests: XCTestCase {
    func testExactBridgeRangesNavigationWrapAndHiddenDirectives() throws {
        let source = "Title: Zoë\r\n\r\nINT. CAFÉ - DAY{{%m: Café 🌙}}\r\n\r\n{{%m}}{{%m: second}}\r\n\r\n[[{{%m: private}}]]\r\n/*{{%m: omitted}}*/\r\n\\{{%m: literal}}\r\n{{%m broken}}"
        let markers = ManuscriptAnalysis.parse(source, format: .screenplay).markers
        XCTAssertEqual(markers.map(\.text), ["Café 🌙", "", "second"])
        XCTAssertEqual(markers.map { (source as NSString).substring(with: $0.range) }, ["{{%m: Café 🌙}}", "{{%m}}", "{{%m: second}}"])
        XCTAssertEqual(ScriptMarkers.next(in: source, selection: NSRange(location: 0, length: 0)), markers[0].range)
        XCTAssertEqual(ScriptMarkers.next(in: source, selection: markers[0].range), markers[1].range)
        XCTAssertEqual(ScriptMarkers.next(in: source, selection: markers[2].range), markers[0].range)
        XCTAssertNil(ScriptMarkers.next(in: "No markers", selection: NSRange(location: 0, length: 0)))
        XCTAssertNil(ScriptMarkers.next(in: source, selection: NSRange(location: -1, length: 1)))
        XCTAssertNil(ScriptMarkers.insertion(source: "🌙", selection: NSRange(location: 1, length: 0), named: true))
        XCTAssertNil(ScriptMarkers.removal(source: "\0{{%m}}", selection: NSRange(location: 0, length: 0)))
    }
    func testInsertionRemovalPreserveSourceAndRemapSelections() throws {
        let source = "Zoë 🌙\r\nOriginal."
        let selected = (source as NSString).range(of: "Original")
        let edit = try XCTUnwrap(ScriptMarkers.insertion(source: source, selection: selected, named: true))
        let inserted = (source as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
        XCTAssertTrue(inserted.hasSuffix("{{%m: }}Original."))
        XCTAssertEqual(edit.selection.location, selected.location + 6)
        let selection = (inserted as NSString).range(of: "Original")
        let removal = try XCTUnwrap(ScriptMarkers.removal(source: inserted, selection: selection))
        let removed = (inserted as NSString).replacingCharacters(in: removal.range, with: removal.replacement)
        XCTAssertEqual(Data(removed.utf8), Data(source.utf8))
        XCTAssertEqual((removed as NSString).substring(with: removal.selection), "Original")
        XCTAssertNil(ScriptMarkers.removal(source: source, selection: selected))
    }
    @MainActor func testOutputExclusionExportsAndProjectReopenPreserveMarkersAndForeignData() throws {
        var document = ScreenplayDocument()
        document.text = "INT. ROOM - DAY{{%m: Café 🌙}}\r\n\r\nA signal.\r\n\r\n{{%m}}"
        document.metadata["foreign"] = ["keep": true]
        let output = try ScreenplayOutput.parse(document.text)
        XCTAssertFalse(output.body.contains { $0.text.contains("%m") })
        XCTAssertFalse(output.body.contains { $0.kind == .marker })
        XCTAssertEqual(try DraftPageCount.calculate(output: output, options: ScreenplayPrintOptions()), 1)
        let outline = try OutlineExport.make(from: document, collapsed: [], query: "")
        XCTAssertEqual(outline.text, "INT. ROOM - DAY\r\n\r\n{{%m: Café 🌙}}\r\n\r\n{{%m}}\r\n\r\n")
        let exported = try OutlineExport.make(from: document, collapsed: [], query: "Café", markersOnly: true)
        XCTAssertEqual(exported.text, "{{%m: Café 🌙}}\r\n\r\n")
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
        XCTAssertEqual(ManuscriptAnalysis.parse(reopened.text, format: .screenplay).markers.count, 2)
        XCTAssertNotNil(reopened.metadata["foreign"])
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
    }
    @MainActor func testNativeCommandsUndoRedoAndFormatGuard() throws {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        let editor = ManuscriptTextView(frame: window.contentView!.bounds)
        window.contentView!.addSubview(editor); editor.allowsUndo = true
        let source = "INT. ROOM - DAY\r\n\r\nZoë 🌙 waits."
        editor.string = source; editor.setSelectedRange(NSRange(location: source.utf16.count, length: 0))
        editor.supportsFountainFormatting = true
        editor.undoManager?.beginUndoGrouping(); editor.insertNamedScriptMarker(nil); editor.undoManager?.endUndoGrouping()
        XCTAssertEqual(editor.string, source + "{{%m: }}")
        editor.undoManager?.undo(); XCTAssertEqual(editor.string, source)
        editor.undoManager?.redo(); XCTAssertEqual(editor.string, source + "{{%m: }}")
        editor.setSelectedRange(NSRange(location: 0, length: 0)); editor.goToNextScriptMarker(nil)
        XCTAssertEqual((editor.string as NSString).substring(with: editor.selectedRange()), "{{%m: }}")
        editor.undoManager?.beginUndoGrouping(); editor.removeScriptMarkers(nil); editor.undoManager?.endUndoGrouping()
        XCTAssertEqual(editor.string, source)
        editor.undoManager?.undo(); XCTAssertEqual(editor.string, source + "{{%m: }}")
    }
    @MainActor func testEveryCategorySupportsNativeMarkersAndProjectReopen() throws {
        for format in DocumentFormat.allCases {
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false; defer { window.close() }
            let editor = ManuscriptTextView(frame: window.contentView!.bounds)
            window.contentView!.addSubview(editor); editor.allowsUndo = true; editor.manuscriptFormat = format
            let history = try XCTUnwrap(editor.undoManager); history.groupsByEvent = false
            let source = format.starter(for: format.presets[0]) + "\r\nZoë 🌙."
            editor.string = source; editor.setSelectedRange(NSRange(location: source.utf16.count, length: 0))
            editor.undoManager?.beginUndoGrouping(); editor.insertScriptMarker(nil); editor.undoManager?.endUndoGrouping()
            XCTAssertTrue(editor.string.utf8.elementsEqual((source + "{{%m}}").utf8))
            let analysis = ManuscriptAnalysis.parse(editor.string, format: format)
            let marker = try XCTUnwrap(analysis.markers.first)
            XCTAssertEqual(EditorPosition.selectedWords(in: editor.string, ranges: [marker.range], manuscriptWords: analysis.word_ranges), 0)
            XCTAssertEqual(EditorPosition.selectedWords(in: editor.string, ranges: [NSRange(location: 0, length: editor.string.utf16.count)], manuscriptWords: analysis.word_ranges), analysis.words)
            editor.goToNextScriptMarker(nil); XCTAssertEqual(editor.selectedRange(), marker.range)
            var document = ScreenplayDocument(); document.format = format; document.text = editor.string
            document.metadata["foreign"] = ["keep": true]
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(reopened.format, format)
            XCTAssertEqual(Data(reopened.text.utf8), Data(editor.string.utf8))
            XCTAssertNotNil(reopened.metadata["foreign"])
            XCTAssertEqual(ManuscriptAnalysis.parse(reopened.text, format: format).markers.count, 1)
            editor.undoManager?.beginUndoGrouping(); editor.removeScriptMarkers(nil); editor.undoManager?.endUndoGrouping()
            XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
            editor.undoManager?.undo(); XCTAssertEqual(Data(editor.string.utf8), Data(reopened.text.utf8))
            editor.isEditable = false; editor.insertScriptMarker(nil)
            XCTAssertEqual(Data(editor.string.utf8), Data(reopened.text.utf8))
        }
    }

}
