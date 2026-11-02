import XCTest
import AppKit
@testable import Fountage

final class ShelfVisibilityTests: XCTestCase {
    func testVisibilityPreservesRecordsAndUnknownSettingsAcrossAllFormats() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.text = "Zoë 🌙 世界\r\n"
            document.metadata["foreign"] = ["keep": [true, false]]
            for kind in ShelfItem.Kind.allCases {
                try document.saveShelfItem(ShelfItem(id: kind.rawValue, kind: kind, title: kind.title, content: "[ ] Plain text 🌙\r\n", pinned: true))
            }
            var metadata = document.metadata
            var own = metadata[ScreenplayDocument.namespace] as! [String: Any]
            var shelf = own["shelf"] as! [String: Any]
            var records = shelf["items"] as! [String: Any]
            records["future"] = ["kind": "future", "keep": [1, 2]]
            shelf["items"] = records; shelf["visibility"] = ["future": ["keep": true]]
            shelf["pinnedOrder"] = ["snippet", "note", "checklist", "future"]
            own["shelf"] = shelf; metadata[ScreenplayDocument.namespace] = own; document.metadata = metadata
            let original = document.metadata; let items = document.shelfItems
            XCTAssertFalse(try document.setShelfVisibility(true, expected: original))
            XCTAssertTrue(try document.setShelfVisibility(false, kind: .note, expected: original))
            XCTAssertEqual(document.visibleShelfItems.map(\.id), ["snippet", "checklist"])
            XCTAssertEqual(document.shelfItems.map(\.id), items.map(\.id))
            let after = document.metadata
            XCTAssertThrowsError(try document.setShelfVisibility(true, expected: original))
            XCTAssertTrue(NSDictionary(dictionary: after).isEqual(to: document.metadata))
            XCTAssertTrue(try document.setShelfVisibility(false, expected: after))
            XCTAssertTrue(document.visibleShelfItems.isEmpty)
            XCTAssertFalse(try document.setShelfVisibility(false, expected: document.metadata))
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255, 1])); asset.preferredFilename = "asset.bin"; wrapper.addFileWrapper(asset)
            let link = FileWrapper(symbolicLinkWithDestinationURL: URL(string: "../unfollowed")!); link.preferredFilename = "link"; wrapper.addFileWrapper(link)
            var reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8)); XCTAssertEqual(reopened.format, format)
            XCTAssertTrue(reopened.visibleShelfItems.isEmpty)
            XCTAssertTrue(try reopened.setShelfVisibility(true, expected: reopened.metadata))
            XCTAssertEqual(reopened.visibleShelfItems.map(\.id), items.map(\.id))
            for (a, b) in zip(items, reopened.shelfItems) { XCTAssertTrue(a.matches(b)) }
            let current = (reopened.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any]
            for key in ["items", "order", "pinnedOrder"] {
                XCTAssertTrue(NSDictionary(dictionary: [key: current[key]!]).isEqual(to: [key: shelf[key]!]))
            }
            XCTAssertNotNil((current["visibility"] as? [String: Any])?["future"])
            let final = try reopened.makeWrapper(contentType: .fountage)
            XCTAssertEqual(final.fileWrappers?["asset.bin"]?.regularFileContents, Data([0, 255, 1]))
            XCTAssertEqual(final.fileWrappers?["link"]?.symbolicLinkDestinationURL?.relativeString, "../unfollowed")
        }
    }
    func testVisibleMovesRetainHiddenSlotsAndPinnedDropDoesNotRewriteBaseOrder() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format
            for (id, kind, pinned) in [("a", ShelfItem.Kind.note, false), ("b", .snippet, false), ("c", .note, false), ("p1", .note, true), ("p2", .snippet, true), ("p3", .note, true)] {
                try document.saveShelfItem(ShelfItem(id: id, kind: kind, title: id, pinned: pinned))
            }
            try document.setShelfVisibility(false, kind: .snippet, expected: document.metadata)
            let hidden = document.shelfItems.first { $0.id == "b" }!
            let c = document.shelfItems.first { $0.id == "c" }!
            XCTAssertThrowsError(try document.moveShelfItem(hidden, down: false, expectedOrder: document.shelfItems.map(\.id)))
            XCTAssertThrowsError(try document.reorderShelfItem(hidden, before: c.id, expectedOrder: document.shelfItems.map(\.id)))
            XCTAssertThrowsError(try document.reorderShelfItem(c, before: hidden.id, expectedOrder: document.shelfItems.map(\.id)))
            XCTAssertTrue(try document.moveShelfItem(c, down: false, expectedOrder: document.shelfItems.map(\.id)))
            var shelf = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any]
            let base = try XCTUnwrap(shelf["order"] as? [String])
            XCTAssertEqual(base, ["c", "b", "a", "p1", "p2", "p3"])
            let p3 = document.shelfItems.first { $0.id == "p3" }!
            XCTAssertTrue(try document.reorderShelfItem(p3, before: "p1", expectedOrder: document.shelfItems.map(\.id)))
            shelf = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any]
            XCTAssertEqual(shelf["order"] as? [String], base)
            XCTAssertEqual(shelf["pinnedOrder"] as? [String], ["p3", "p2", "p1"])
            XCTAssertEqual(document.visibleShelfItems.map(\.id), ["p3", "p1", "c", "a"])
            let p2 = document.shelfItems.first { $0.id == "p2" }!
            XCTAssertThrowsError(try document.reorderShelfItem(p3, before: p2.id, expectedOrder: document.shelfItems.map(\.id)))
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(reopened.visibleShelfItems.map(\.id), document.visibleShelfItems.map(\.id))
        }
    }
    func testMalformedKnownVisibilityRejectsWritesButKeepsUnknownValues() throws {
        for value: Any in [false, [], ["note": NSNull()], ["snippet": "false"], ["checklist": 1], ["note": 0]] {
            var document = ScreenplayDocument()
            try document.saveShelfItem(ShelfItem(kind: .note, title: "Keep"))
            var metadata = document.metadata; var own = metadata[ScreenplayDocument.namespace] as! [String: Any]
            var shelf = own["shelf"] as! [String: Any]; shelf["visibility"] = value; own["shelf"] = shelf; metadata[ScreenplayDocument.namespace] = own; document.metadata = metadata
            let before = document.metadata
            XCTAssertFalse(document.canEditShelf)
            XCTAssertThrowsError(try document.setShelfVisibility(true, expected: before))
            XCTAssertThrowsError(try document.saveShelfItem(ShelfItem(kind: .note, title: "New")))
            XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        }
        var empty = ScreenplayDocument(); let before = empty.metadata
        XCTAssertFalse(try empty.setShelfVisibility(true, expected: before))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: empty.metadata))
        empty.metadata["future"] = true; let stale = empty.metadata
        empty.metadata["future"] = 1
        XCTAssertThrowsError(try empty.setShelfVisibility(false, expected: stale), "Bool/number changes are distinct JSON metadata")
    }
    func testRustHiddenProjectsReadAndRoundtripMixedVisibilityThroughSwift() throws {
        guard let input = ProcessInfo.processInfo.environment["FOUNTAGE_VISIBILITY_FIXTURES"], let output = ProcessInfo.processInfo.environment["FOUNTAGE_SWIFT_VISIBILITY_FIXTURES"] else { throw XCTSkip("Set visibility interoperability fixture directories.") }
        try FileManager.default.createDirectory(atPath: output, withIntermediateDirectories: true)
        for (category, format) in DocumentFormat.allCases.enumerated() {
            let inputURL = URL(fileURLWithPath: input).appendingPathComponent("Hidden \(category).fountage")
            var document = try ScreenplayDocument(wrapper: FileWrapper(url: inputURL, options: .immediate), contentType: .fountage)
            let original = document.metadata; let source = Data(document.text.utf8); let ids = document.shelfItems.map(\.id)
            XCTAssertEqual(document.format, format); XCTAssertTrue(document.visibleShelfItems.isEmpty)
            XCTAssertTrue(try document.setShelfVisibility(true, expected: document.metadata))
            XCTAssertEqual(document.visibleShelfItems.map(\.id), ids)
            XCTAssertTrue(try document.setShelfVisibility(false, kind: .note, expected: document.metadata))
            XCTAssertFalse(document.visibleShelfItems.contains { $0.kind == .note })
            let beforeShelf = (original[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any]
            let afterShelf = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any]
            for key in ["items", "order"] { XCTAssertTrue(NSDictionary(dictionary: [key: beforeShelf[key]!]).isEqual(to: [key: afterShelf[key]!])) }
            let wrapper = try document.makeWrapper(contentType: .fountage)
            XCTAssertEqual(wrapper.fileWrappers?["attachment.bin"]?.regularFileContents, Data([0, 255, 1]))
            XCTAssertEqual(wrapper.fileWrappers?["link"]?.symbolicLinkDestinationURL?.relativeString, "../unfollowed")
            XCTAssertEqual(Data(document.text.utf8), source)
            try wrapper.write(to: URL(fileURLWithPath: output).appendingPathComponent("Swift Mixed \(category).fountage"), options: .atomic, originalContentsURL: nil)
        }
    }
}
