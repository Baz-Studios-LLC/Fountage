import XCTest
@testable import Fountage

final class QuickNoteTests: XCTestCase {
    func testUnfinishedDraftHideShowCaptureAndReopenPreserveAllFormatsAndOpaqueData() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.text = "Original 🌙\r\n"
            document.metadata["foreign"] = ["keep": true]
            let item = ShelfItem(id: "original", kind: .snippet, title: "Keep", content: "[ ] Keep raw 🌙\r\n", pinned: true)
            try document.saveShelfItem(item)
            var metadata = document.metadata; var own = metadata[ScreenplayDocument.namespace] as! [String: Any]
            var shelf = own["shelf"] as! [String: Any]
            shelf["quickEntry"] = ["future": ["keep": true]]; shelf["pinnedOrder"] = ["original", "future"]
            shelf["visibility"] = ["note": false]
            own["shelf"] = shelf; metadata[ScreenplayDocument.namespace] = own; document.metadata = metadata
            let draft = "  Quick Zoë 🌙\r\nSecond 世界\r\n"
            XCTAssertTrue(try document.setQuickNoteText(draft, replacing: ""))
            XCTAssertFalse(try document.setQuickNoteText(draft, replacing: draft))
            XCTAssertTrue(try document.setQuickNoteVisible(false))
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255, 1])); asset.preferredFilename = "asset.bin"; wrapper.addFileWrapper(asset)
            let link = FileWrapper(symbolicLinkWithDestinationURL: URL(string: "../unfollowed")!); link.preferredFilename = "link"; wrapper.addFileWrapper(link)
            var reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(Data(reopened.quickNoteText.utf8), Data(draft.utf8)); XCTAssertFalse(reopened.quickNoteVisible)
            XCTAssertTrue(try reopened.setQuickNoteVisible(true)); XCTAssertTrue(try reopened.setQuickNoteVisible(false))
            let before = reopened.metadata
            XCTAssertThrowsError(try reopened.addQuickNote(replacing: "stale"))
            XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: reopened.metadata))
            let id = try XCTUnwrap(reopened.addQuickNote(replacing: draft))
            XCTAssertEqual(reopened.quickNoteText, ""); XCTAssertFalse(reopened.quickNoteVisible)
            XCTAssertEqual(reopened.shelfItems.count, 2); XCTAssertEqual(reopened.visibleShelfItems.count, 1)
            let note = try XCTUnwrap(reopened.shelfItems.first { $0.id == id })
            XCTAssertEqual(note.kind, .note); XCTAssertEqual(note.title, "Note"); XCTAssertEqual(Data(note.content.utf8), Data(draft.utf8))
            XCTAssertTrue(reopened.shelfItems.first { $0.id == "original" }!.matches(item))
            XCTAssertThrowsError(try reopened.addQuickNote(replacing: draft)); XCTAssertNil(try reopened.addQuickNote(replacing: ""))
            let finalWrapper = try reopened.makeWrapper(contentType: .fountage)
            let final = try ScreenplayDocument(wrapper: finalWrapper, contentType: .fountage)
            XCTAssertEqual(Data(final.text.utf8), Data(document.text.utf8)); XCTAssertEqual(final.format, format)
            XCTAssertEqual(final.quickNoteText, ""); XCTAssertFalse(final.quickNoteVisible)
            XCTAssertEqual(final.shelfItems.count, 2)
            let saved = (final.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any]
            XCTAssertNotNil((saved["quickEntry"] as! [String: Any])["future"])
            XCTAssertEqual(saved["pinnedOrder"] as? [String], ["original", "future"])
            XCTAssertEqual(saved["order"] as? [String], [id, "original"])
            XCTAssertNotNil(final.metadata["foreign"])
            XCTAssertEqual(finalWrapper.fileWrappers?["asset.bin"]?.regularFileContents, Data([0, 255, 1]))
            XCTAssertEqual(finalWrapper.fileWrappers?["link"]?.symbolicLinkDestinationURL?.relativeString, "../unfollowed")
        }
    }
    func testQuickEntryNoopsValidationAndExactUnicodeStaleCheck() throws {
        var document = ScreenplayDocument(); let empty = document.metadata
        XCTAssertFalse(try document.setQuickNoteVisible(true)); XCTAssertFalse(try document.setQuickNoteText("", replacing: ""))
        XCTAssertNil(try document.addQuickNote(replacing: ""))
        XCTAssertTrue(NSDictionary(dictionary: empty).isEqual(to: document.metadata))
        try document.setQuickNoteText(" \r\n", replacing: ""); XCTAssertNil(try document.addQuickNote(replacing: " \r\n"))
        try document.setQuickNoteText("é", replacing: " \r\n")
        XCTAssertThrowsError(try document.setQuickNoteText("lost", replacing: "e\u{301}"))
        XCTAssertThrowsError(try document.addQuickNote(replacing: "e\u{301}"))
        XCTAssertThrowsError(try document.setQuickNoteText("bad\0", replacing: "é"))
        XCTAssertEqual(document.quickNoteText, "é")
        let pending = document.metadata
        XCTAssertThrowsError(try document.addQuickNote(replacing: "é", composing: true))
        XCTAssertTrue(NSDictionary(dictionary: pending).isEqual(to: document.metadata))
        for value: Any in [false, [], ["text": 1], ["text": NSNull()], ["text": "bad\0"], ["visible": 0]] {
            var candidate = document; var metadata = candidate.metadata; var own = metadata[ScreenplayDocument.namespace] as! [String: Any]
            var shelf = own["shelf"] as! [String: Any]; shelf["quickEntry"] = value; own["shelf"] = shelf; metadata[ScreenplayDocument.namespace] = own; candidate.metadata = metadata
            XCTAssertFalse(candidate.canEditShelf)
            XCTAssertThrowsError(try candidate.setQuickNoteVisible(true))
            XCTAssertThrowsError(try candidate.setQuickNoteText("new", replacing: candidate.quickNoteText))
            XCTAssertThrowsError(try candidate.addQuickNote(replacing: candidate.quickNoteText))
        }
    }
}
