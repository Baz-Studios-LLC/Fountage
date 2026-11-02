import XCTest
import AppKit
@testable import Fountage

final class ShelfTests: XCTestCase {
    func testAllKindsOrderingPinningAndExactProjectReopenAcrossCategories() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.text = "Original 🌙\r\n"
            document.metadata["foreign"] = ["keep": true]
            var entries: [ShelfItem] = []
            for kind in ShelfItem.Kind.allCases {
                let item = ShelfItem(kind: kind, title: "Zoë " + kind.title, content: "[ ] Review 🌙\r\n[x] Done\r\n")
                try document.saveShelfItem(item); entries.append(item)
            }
            XCTAssertEqual(document.shelfItems.map(\.id), entries.map(\.id))
            XCTAssertTrue(try document.moveShelfItem(entries[1], down: false, expectedOrder: entries.map(\.id)))
            XCTAssertEqual(document.shelfItems.map(\.id), [entries[1].id, entries[0].id, entries[2].id])
            var pinned = entries[2]; pinned.pinned = true
            try document.saveShelfItem(pinned, replacing: entries[2])
            XCTAssertEqual(document.shelfItems.map(\.id), [entries[2].id, entries[1].id, entries[0].id])
            XCTAssertFalse(try document.moveShelfItem(pinned, down: true, expectedOrder: document.shelfItems.map(\.id)))
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
            XCTAssertEqual(reopened.shelfItems.map(\.id), document.shelfItems.map(\.id))
            for (a, b) in zip(reopened.shelfItems, document.shelfItems) { XCTAssertTrue(a.matches(b)) }
            XCTAssertNotNil(reopened.metadata["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        }
    }
    func testStaleNoOpDeletionAndForeignRecordPreservation() throws {
        var document = ScreenplayDocument()
        let original = ShelfItem(id: "one", kind: .snippet, title: "Café", content: "Original\r\n")
        try document.saveShelfItem(original)
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        var shelf = own["shelf"] as! [String: Any]
        var records = shelf["items"] as! [String: Any]
        var record = records[original.id] as! [String: Any]; record["future"] = ["asset": "private.bin"]; records[original.id] = record
        records["unknown"] = ["kind": "future", "payload": [1, 2]]
        shelf["items"] = records; shelf["order"] = ["unknown", "one"]; shelf["foreign"] = true
        own["shelf"] = shelf; document.metadata[ScreenplayDocument.namespace] = own
        let before = document.metadata
        XCTAssertFalse(try document.saveShelfItem(original, replacing: original))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        var updated = original; updated.content = "Revised 🌙"
        try document.saveShelfItem(updated, replacing: original)
        XCTAssertThrowsError(try document.saveShelfItem(original, replacing: original))
        XCTAssertThrowsError(try document.removeShelfItem(original))
        XCTAssertThrowsError(try document.saveShelfItem(ShelfItem(id: "unknown", kind: .note, title: "Collision")))
        XCTAssertThrowsError(try document.moveShelfItem(updated, down: false, expectedOrder: []))
        let merged = ((document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any])
        XCTAssertNotNil(((merged["items"] as! [String: Any])["one"] as! [String: Any])["future"])
        XCTAssertEqual(merged["foreign"] as? Bool, true)
        try document.removeShelfItem(updated)
        let after = ((document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any])
        XCTAssertNotNil((after["items"] as! [String: Any])["unknown"])
        XCTAssertEqual(after["order"] as? [String], ["unknown"])
        XCTAssertThrowsError(try document.saveShelfItem(original, replacing: updated))
    }
    func testMalformedFutureNamespacesAndInvalidItemsRejectWithoutMutation() throws {
        for value: Any in ["future", ["version": 2, "items": [:], "order": []], ["version": true, "items": [:], "order": []], ["version": 1, "items": [], "order": []], ["version": 1, "items": [:], "order": ["a", "a"]]] {
            var document = ScreenplayDocument(); document.metadata[ScreenplayDocument.namespace] = ["shelf": value]
            let before = document.metadata
            XCTAssertFalse(document.canEditShelf)
            XCTAssertThrowsError(try document.saveShelfItem(ShelfItem(kind: .note, title: "New")))
            XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        }
        var document = ScreenplayDocument(); document.metadata[ScreenplayDocument.namespace] = "foreign"
        XCTAssertThrowsError(try document.saveShelfItem(ShelfItem(kind: .note, title: "New")))
        document = ScreenplayDocument()
        for item in [ShelfItem(kind: .note, title: " "), ShelfItem(kind: .note, title: "New", content: "\0")] {
            XCTAssertThrowsError(try document.saveShelfItem(item))
        }
    }
    @MainActor func testInsertionReplacesOnlySelectionWithNativeUndoAndRejectsStaleOrSplitSurrogates() throws {
        let source = "Before 🌙\r\nReplace me\r\nAfter"
        let range = (source as NSString).range(of: "Replace me")
        let content = "Zoë\r\n[ ] Next\r\n"
        let edit = try ShelfInsertion.edit(source: source, expectedSource: source, selection: range, content: content)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        let editor = ManuscriptTextView(frame: window.contentView!.bounds); window.contentView!.addSubview(editor); editor.allowsUndo = true; editor.string = source
        let history = try XCTUnwrap(editor.undoManager); history.groupsByEvent = false
        history.beginUndoGrouping(); XCTAssertTrue(edit.apply(to: editor)); history.endUndoGrouping()
        XCTAssertEqual(editor.string, source.replacingOccurrences(of: "Replace me", with: content))
        history.undo(); XCTAssertEqual(editor.string, source); history.redo()
        XCTAssertEqual(editor.selectedRange(), edit.selection)
        XCTAssertThrowsError(try ShelfInsertion.edit(source: source + "!", expectedSource: source, selection: range, content: content))
        for range in [NSRange(location: NSNotFound, length: 0), NSRange(location: 8, length: 0), NSRange(location: 0, length: 999)] {
            XCTAssertThrowsError(try ShelfInsertion.edit(source: source, expectedSource: source, selection: range, content: content))
        }
    }
}
