import XCTest
@testable import Fountage

final class ShelfPinOrderTests: XCTestCase {
    func testPinnedReorderKeepsBaseOrderAndUnpinReturnsOriginalPlaceAcrossFormats() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.text = "Zoë 🌙\r\n"
            for id in ["a", "b", "c"] { try document.saveShelfItem(ShelfItem(id: id, kind: .checklist, title: id, content: "[ ] " + id)) }
            for id in ["a", "b"] { let expected = document.shelfItems.first { $0.id == id }!; var changed = expected; changed.pinned = true; try document.saveShelfItem(changed, replacing: expected) }
            let b = document.shelfItems.first { $0.id == "b" }!
            XCTAssertTrue(try document.moveShelfItem(b, down: false, expectedOrder: document.shelfItems.map(\.id)))
            XCTAssertEqual(document.shelfItems.map(\.id), ["b", "a", "c"])
            let own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
            let shelf = own["shelf"] as! [String: Any]
            XCTAssertEqual(shelf["order"] as? [String], ["a", "b", "c"])
            XCTAssertEqual(shelf["pinnedOrder"] as? [String], ["b", "a"])
            var reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(reopened.shelfItems.map(\.id), ["b", "a", "c"])
            for id in ["a", "b"] { let expected = reopened.shelfItems.first { $0.id == id }!; var changed = expected; changed.pinned = false; try reopened.saveShelfItem(changed, replacing: expected) }
            XCTAssertEqual(reopened.shelfItems.map(\.id), ["a", "b", "c"])
            XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
            XCTAssertEqual(reopened.format, format)
        }
    }
    func testMalformedPinnedOrderDisablesWritesWithoutDiscardingMetadata() throws {
        var document = ScreenplayDocument()
        try document.saveShelfItem(ShelfItem(id: "a", kind: .checklist, title: "A"))
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        var shelf = own["shelf"] as! [String: Any]; shelf["pinnedOrder"] = ["a", "a"]; own["shelf"] = shelf
        var metadata = document.metadata; metadata[ScreenplayDocument.namespace] = own; document.metadata = metadata
        XCTAssertFalse(document.canEditShelf)
        XCTAssertThrowsError(try document.saveShelfItem(ShelfItem(id: "b", kind: .note, title: "B")))
        XCTAssertEqual((document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as? NSDictionary, shelf as NSDictionary)
    }
}

extension ShelfPinOrderTests {
    func testRustPinnedProjectsRoundtripThroughSwiftWithUnpinAndOpaqueAssets() throws {
        guard let input = ProcessInfo.processInfo.environment["FOUNTAGE_PIN_FIXTURES"], let output = ProcessInfo.processInfo.environment["FOUNTAGE_SWIFT_PIN_FIXTURES"] else { throw XCTSkip("Set pinned project interoperability fixture directories.") }
        try FileManager.default.createDirectory(atPath: output, withIntermediateDirectories: true)
        for (category, format) in DocumentFormat.allCases.enumerated() {
            let inputURL = URL(fileURLWithPath: input).appendingPathComponent("Pinned \(category).fountage")
            var document = try ScreenplayDocument(wrapper: FileWrapper(url: inputURL, options: .immediate), contentType: .fountage)
            XCTAssertEqual(document.format, format)
            XCTAssertEqual(document.shelfItems.filter { $0.kind == .checklist }.map(\.id), ["plan", "later", "last"])
            let source = Data(document.text.utf8)
            for id in ["plan", "later"] { let expected = document.shelfItems.first { $0.id == id }!; var item = expected; item.pinned = false; try document.saveShelfItem(item, replacing: expected) }
            XCTAssertEqual(document.shelfItems.filter { $0.kind == .checklist }.map(\.id), ["later", "plan", "last"])
            let wrapper = try document.makeWrapper(contentType: .fountage)
            XCTAssertEqual(wrapper.fileWrappers?["attachment.bin"]?.regularFileContents, Data([0,255,1]))
            XCTAssertEqual(wrapper.fileWrappers?["link"]?.symbolicLinkDestinationURL?.relativeString, "../unfollowed")
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), source)
            XCTAssertNotNil(reopened.metadata["foreign"])
            try wrapper.write(to: URL(fileURLWithPath: output).appendingPathComponent("Swift Pinned \(category).fountage"), options: .atomic, originalContentsURL: nil)
        }
    }
}
