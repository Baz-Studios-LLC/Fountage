import XCTest
import AppKit
@testable import Fountage

final class ShelfDragTests: XCTestCase {
    func testArbitraryReorderPreservesUnknownSlotsAndRejectsStaleOrPinCrossing() throws {
        var document = ScreenplayDocument()
        let a = ShelfItem(id: "a", kind: .note, title: "A")
        let b = ShelfItem(id: "b", kind: .snippet, title: "B", content: "Zoë\r\n")
        let c = ShelfItem(id: "c", kind: .checklist, title: "C")
        let pin = ShelfItem(id: "p", kind: .note, title: "Pinned", pinned: true)
        for item in [a, b, c, pin] { try document.saveShelfItem(item) }
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        var shelf = own["shelf"] as! [String: Any]; var records = shelf["items"] as! [String: Any]
        records["future"] = ["kind": "future", "payload": "keep"]
        shelf["items"] = records; shelf["order"] = ["a", "future", "b", "c", "p"]; shelf["foreign"] = true
        own["shelf"] = shelf; document.metadata[ScreenplayDocument.namespace] = own
        let order = document.shelfItems.map(\.id)
        XCTAssertTrue(try document.reorderShelfItem(c, before: a.id, expectedOrder: order))
        XCTAssertEqual(document.shelfItems.map(\.id), ["p", "c", "a", "b"])
        let after = ((document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any])
        XCTAssertEqual(after["order"] as? [String], ["c", "future", "a", "b", "p"])
        XCTAssertEqual(after["foreign"] as? Bool, true); XCTAssertNotNil((after["items"] as! [String: Any])["future"])
        XCTAssertThrowsError(try document.reorderShelfItem(a, before: b.id, expectedOrder: order))
        XCTAssertThrowsError(try document.reorderShelfItem(a, before: pin.id, expectedOrder: document.shelfItems.map(\.id)))
        XCTAssertFalse(try document.reorderShelfItem(a, before: a.id, expectedOrder: document.shelfItems.map(\.id)))
        XCTAssertTrue(try document.reorderShelfItem(c, before: nil, expectedOrder: document.shelfItems.map(\.id)))
        XCTAssertEqual(document.shelfItems.map(\.id), ["p", "a", "b", "c"])
        var edited = b; edited.content += "changed"; try document.saveShelfItem(edited, replacing: b)
        XCTAssertThrowsError(try document.reorderShelfItem(b, before: a.id, expectedOrder: document.shelfItems.map(\.id)))
    }
    func testMultiItemDropIsAtomicAndSourceAssetsSurviveAllCategoryReopen() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.text = "Original 🌙\r\n"
            document.metadata["foreign"] = ["keep": true]
            let before = document.metadata
            XCTAssertThrowsError(try document.addDroppedShelfText(["Valid", "Invalid\0"]))
            XCTAssertTrue(document.shelfItems.isEmpty); XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
            let values = ["  Zoë 🌙\r\nSecond\r\n", "[ ] Review\n[x] Done"]
            try document.addDroppedShelfText(values)
            XCTAssertEqual(document.shelfItems.map(\.content), values)
            XCTAssertEqual(document.shelfItems[0].title, "Zoë 🌙")
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(reopened.text, "Original 🌙\r\n"); XCTAssertEqual(reopened.shelfItems.map(\.content), values)
            XCTAssertNotNil(reopened.metadata["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        }
        var unsupported = ScreenplayDocument(); unsupported.metadata[ScreenplayDocument.namespace] = ["shelf": ["version": 99]]
        let before = unsupported.metadata
        XCTAssertThrowsError(try unsupported.addDroppedShelfText(["Text"]))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: unsupported.metadata))
    }
    func testNativeDragWriterCarriesExactTextAndOpaqueToken() throws {
        let item = ShelfItem(kind: .snippet, title: "Private title", content: "Zoë 🌙\r\nSecond\r\n")
        let session = ShelfDragSession(item: item, order: [item.id])
        let writer = session.pasteboardItem()
        let text = try XCTUnwrap(writer.string(forType: .string))
        XCTAssertEqual(Data(text.utf8), Data(item.content.utf8))
        let token = try XCTUnwrap(writer.data(forType: NSPasteboard.PasteboardType(ShelfDragSession.type.identifier)))
        XCTAssertEqual(token, session.token)
        XCTAssertFalse(String(decoding: token, as: UTF8.self).contains(item.content))
        XCTAssertNotEqual(token, ShelfDragSession(item: item, order: [item.id]).token)
    }
}
