import XCTest
import AppKit
@testable import Fountage

@MainActor final class ShelfCaptureTests: XCTestCase {
    func testCopyAndCutKeepExactSelectionAndProjectDataForAllCategories() throws {
        let source = "Before 🌙\r\n  Zoë’s **choice**\r\n[[keep]]\r\nAfter"
        let range = (source as NSString).range(of: "  Zoë’s **choice**\r\n[[keep]]\r\n")
        for format in DocumentFormat.allCases {
            for cut in [false, true] {
                var document = ScreenplayDocument(); document.format = format; document.text = source
                document.metadata["foreign"] = ["keep": true]
                let existing = ShelfItem(kind: .note, title: "Existing", content: "Keep")
                try document.saveShelfItem(existing)
                let item = try ShelfCapture.store(in: &document, expectedSource: source, selection: range, cut: cut)
                XCTAssertEqual(Data(item.content.utf8), Data((source as NSString).substring(with: range).utf8))
                XCTAssertEqual(item.title, "Zoë’s **choice**")
                XCTAssertEqual(document.shelfItems.map(\.id), [existing.id, item.id])
                let expected = cut ? (source as NSString).replacingCharacters(in: range, with: "") : source
                XCTAssertEqual(Data(document.text.utf8), Data(expected.utf8))
                let wrapper = try document.makeWrapper(contentType: .fountage)
                let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
                let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
                XCTAssertEqual(Data(reopened.text.utf8), Data(expected.utf8)); XCTAssertTrue(try XCTUnwrap(reopened.shelfItems.last).matches(item))
                XCTAssertNotNil(reopened.metadata["foreign"])
                XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
            }
        }
        var blank = ScreenplayDocument(); blank.text = "\r\n \t"
        let item = try ShelfCapture.store(in: &blank, expectedSource: blank.text, selection: NSRange(location: 0, length: blank.text.utf16.count), cut: true)
        XCTAssertEqual(item.title, "Selected text"); XCTAssertEqual(item.content, "\r\n \t"); XCTAssertEqual(blank.text, "")
    }
    func testFailedCaptureIsAtomicForStaleInvalidCollisionAndUnsupportedMetadata() throws {
        let source = "Before 🌙\r\nAfter"
        var document = ScreenplayDocument(); document.text = source
        try document.saveShelfItem(ShelfItem(id: "taken", kind: .note, title: "Existing"))
        let before = document.metadata
        for range in [NSRange(location: 0, length: 0), NSRange(location: NSNotFound, length: 1), NSRange(location: 8, length: 1), NSRange(location: 0, length: 1000)] {
            XCTAssertThrowsError(try ShelfCapture.store(in: &document, expectedSource: source, selection: range, cut: true))
            XCTAssertEqual(document.text, source); XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        }
        let range = NSRange(location: 0, length: 6)
        XCTAssertThrowsError(try ShelfCapture.store(in: &document, expectedSource: "stale", selection: range, cut: true))
        XCTAssertThrowsError(try ShelfCapture.store(in: &document, expectedSource: source, selection: range, cut: true, id: "taken"))
        XCTAssertEqual(document.text, source); XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        document.metadata[ScreenplayDocument.namespace] = ["shelf": ["version": 99, "foreign": true]]
        let unsupported = document.metadata
        XCTAssertThrowsError(try ShelfCapture.store(in: &document, expectedSource: source, selection: range, cut: true))
        XCTAssertEqual(document.text, source); XCTAssertTrue(NSDictionary(dictionary: unsupported).isEqual(to: document.metadata))
    }
    @MainActor func testNativeCaptureCommandValidationAndDispatch() throws {
        let editor = ManuscriptTextView(frame: .zero); editor.string = "Zoë 🌙\r\nNext"; editor.setSelectedRange(NSRange(location: 0, length: 6))
        let copy = NSMenuItem(title: "Copy Selection to Shelf", action: #selector(ManuscriptTextView.copySelectionToShelf(_:)), keyEquivalent: "")
        let cut = NSMenuItem(title: "Cut Selection to Shelf", action: #selector(ManuscriptTextView.cutSelectionToShelf(_:)), keyEquivalent: "")
        XCTAssertFalse(editor.validateUserInterfaceItem(copy))
        var captures: [(String, NSRange, Bool)] = []
        editor.onShelfCapture = { captures.append(($0, $1, $2)) }
        XCTAssertTrue(editor.validateUserInterfaceItem(copy)); XCTAssertTrue(editor.validateUserInterfaceItem(cut))
        editor.copySelectionToShelf(nil); editor.cutSelectionToShelf(nil)
        XCTAssertEqual(captures.count, 2); XCTAssertEqual(captures[0].0, editor.string); XCTAssertEqual(captures[0].1, editor.selectedRange())
        XCTAssertFalse(captures[0].2); XCTAssertTrue(captures[1].2)
        editor.isEditable = false; XCTAssertFalse(editor.validateUserInterfaceItem(cut)); editor.cutSelectionToShelf(nil); XCTAssertEqual(captures.count, 2)
        editor.isEditable = true; editor.setSelectedRange(NSRange(location: 0, length: 0))
        XCTAssertFalse(editor.validateUserInterfaceItem(copy)); editor.copySelectionToShelf(nil); XCTAssertEqual(captures.count, 2)
    }
    func testCommandRoutingValidatesEditorBeforeDispatch() throws {
        let editor = ManuscriptTextView(frame: .zero); editor.string = "Zoë"; editor.setSelectedRange(NSRange(location: 0, length: 3))
        var captured = false
        editor.onShelfCapture = { _, _, _ in captured = true }
        XCTAssertTrue(WriterMenuAction.perform(#selector(ManuscriptTextView.copySelectionToShelf(_:)), editor: editor))
        XCTAssertTrue(captured)
        editor.supportsFountainFormatting = false
        XCTAssertFalse(WriterMenuAction.perform(#selector(ManuscriptTextView.fountainBold(_:)), editor: editor))
        XCTAssertEqual(editor.string, "Zoë")
        editor.supportsFountainFormatting = true
        XCTAssertTrue(WriterMenuAction.perform(#selector(ManuscriptTextView.fountainBold(_:)), editor: editor))
        XCTAssertEqual(editor.string, "**Zoë**")
        editor.isEditable = false
        XCTAssertFalse(WriterMenuAction.perform(#selector(ManuscriptTextView.insertScriptMarker(_:)), editor: editor))
        XCTAssertEqual(editor.string, "**Zoë**")
    }

}
