import XCTest
@testable import Fountage

final class ShelfChecklistTests: XCTestCase {
    func testInlineToggleChangesOneStateCharacterPreservesMarkupAndForeignFields() throws {
        var document = ScreenplayDocument(); document.text = "Manuscript 🌙\r\n"
        let item = ShelfItem(id: "list", kind: .checklist, title: "Revision", content: "[ ] Zoë 🌙 [[private]]\r\n[X] Done\r\n/*\r\n[ ] hidden\r\n*/", pinned: true)
        try document.saveShelfItem(item)
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]; var shelf = own["shelf"] as! [String: Any]
        var records = shelf["items"] as! [String: Any]; var raw = records[item.id] as! [String: Any]
        raw["future"] = ["keep": true]; records[item.id] = raw; records["unknown"] = ["kind": "future"]
        shelf["items"] = records; own["shelf"] = shelf; document.metadata[ScreenplayDocument.namespace] = own
        let task = try XCTUnwrap(ManuscriptAnalysis.parse(item.content, format: .general).todos.first)
        XCTAssertTrue(try ShelfChecklistEdit.toggle(in: &document, item: item, task: task, completed: true))
        let changed = try XCTUnwrap(document.shelfItems.first)
        XCTAssertEqual(Data(changed.content.utf8), Data(item.content.replacingOccurrences(of: "[ ] Zoë", with: "[x] Zoë").utf8))
        XCTAssertTrue(changed.pinned); XCTAssertEqual(document.text, "Manuscript 🌙\r\n")
        let updatedTask = try XCTUnwrap(ManuscriptAnalysis.parse(changed.content, format: .general).todos.first)
        let before = document.metadata
        XCTAssertFalse(try ShelfChecklistEdit.toggle(in: &document, item: changed, task: updatedTask, completed: true))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        XCTAssertThrowsError(try ShelfChecklistEdit.toggle(in: &document, item: item, task: task, completed: false))
        let merged = ((document.metadata[ScreenplayDocument.namespace] as! [String: Any])["shelf"] as! [String: Any])["items"] as! [String: Any]
        XCTAssertNotNil((merged["list"] as! [String: Any])["future"]); XCTAssertNotNil(merged["unknown"])
    }
    func testChecklistAppendIsContiguousAndRejectsInvalidHiddenOrStaleInputsAtomically() throws {
        for (source, expected) in [("", "[ ] Review 🌙\n"), ("[ ] First", "[ ] First\n[ ] Review 🌙\n"), ("[ ] First\n", "[ ] First\n[ ] Review 🌙\n"), ("[ ] First\r\n", "[ ] First\r\n[ ] Review 🌙\r\n")] {
            var document = ScreenplayDocument()
            let item = ShelfItem(kind: .checklist, title: "Tasks", content: source)
            try document.saveShelfItem(item)
            try ShelfChecklistEdit.append(in: &document, item: item, text: "Review 🌙")
            XCTAssertEqual(document.shelfItems[0].content, expected)
            XCTAssertThrowsError(try ShelfChecklistEdit.append(in: &document, item: item, text: "Stale"))
        }
        for source in ["[ ] First", "/* unfinished"] {
            var document = ScreenplayDocument(); document.text = "Keep"
            let item = ShelfItem(kind: .checklist, title: "Tasks", content: source); try document.saveShelfItem(item)
            let before = document.metadata
            for text in ["", "Two\nlines", "Bad\0", "Hidden [[note]]"] {
                XCTAssertThrowsError(try ShelfChecklistEdit.append(in: &document, item: item, text: text))
                XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata)); XCTAssertEqual(document.text, "Keep")
            }
            if source.hasPrefix("/*") { XCTAssertThrowsError(try ShelfChecklistEdit.append(in: &document, item: item, text: "Valid but omitted")) }
        }
    }
    func testAllCategoryProjectReopenKeepsIndependentTasksAndAssets() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.text = "Original 🌙\r\n"
            document.metadata["foreign"] = ["keep": true]
            let original = ShelfItem(kind: .checklist, title: "Tasks", content: "[ ] Review Zoë\r\n"); try document.saveShelfItem(original)
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(original.content, format: .general).todos.first)
            try ShelfChecklistEdit.toggle(in: &document, item: original, task: task, completed: true)
            let changed = try XCTUnwrap(document.shelfItems.first)
            try ShelfChecklistEdit.append(in: &document, item: changed, text: "Send 🌙")
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(reopened.shelfItems[0].content, "[x] Review Zoë\r\n[ ] Send 🌙\r\n")
            XCTAssertEqual(Data(reopened.text.utf8), Data("Original 🌙\r\n".utf8)); XCTAssertEqual(reopened.format, format)
            XCTAssertNotNil(reopened.metadata["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        }
    }
}

extension ShelfChecklistTests {
    func testRustShelfFixturesReadEditAndReopenAcrossAllWritingCategories() throws {
        guard let directory = ProcessInfo.processInfo.environment["FOUNTAGE_RUST_SHELF_FIXTURES"],
              let output = ProcessInfo.processInfo.environment["FOUNTAGE_SWIFT_SHELF_FIXTURES"] else {
            throw XCTSkip("Set both Shelf fixture directories for Rust/Swift interoperability testing.")
        }
        try FileManager.default.createDirectory(atPath: output, withIntermediateDirectories: true)
        for (category, format) in DocumentFormat.allCases.enumerated() {
            let url = URL(fileURLWithPath: directory).appendingPathComponent("Rust Shelf \(category).fountage")
            let wrapper = try FileWrapper(url: url, options: .immediate)
            var document = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(document.format, format)
            XCTAssertEqual(Data(document.text.utf8), Data("Title: Unchanged manuscript\r\n\r\nZoë 🌙 waits.\r\n".utf8))
            XCTAssertTrue(document.canEditShelf)
            XCTAssertEqual(document.shelfItems.map(\.id), ["b", "a", "c"])
            let item = try XCTUnwrap(document.shelfItems.first(where: { $0.id == "a" }))
            XCTAssertEqual(item.content, "[x] Review Zoë 🌙 [[keep]]\r\n[ ] Send draft\r\n")
            let task = try XCTUnwrap(ManuscriptAnalysis.parse(item.content, format: .general).todos.first)
            XCTAssertTrue(task.completed)
            XCTAssertTrue(try ShelfChecklistEdit.toggle(in: &document, item: item, task: task, completed: false))
            let result = try document.makeWrapper(contentType: .fountage)
            let reopened = try ScreenplayDocument(wrapper: result, contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
            XCTAssertEqual(reopened.shelfItems.first(where: { $0.id == "a" })?.content,
                           "[ ] Review Zoë 🌙 [[keep]]\r\n[ ] Send draft\r\n")
            let own = try XCTUnwrap(reopened.metadata[ScreenplayDocument.namespace] as? [String: Any])
            let shelf = try XCTUnwrap(own["shelf"] as? [String: Any])
            let records = try XCTUnwrap(shelf["items"] as? [String: Any])
            XCTAssertNotNil(shelf["future"]); XCTAssertNotNil(records["future"]); XCTAssertNotNil(records["malformed"])
            XCTAssertNotNil((records["a"] as? [String: Any])?["future"])
            XCTAssertEqual(result.fileWrappers?["assets"]?.fileWrappers?["private.bin"]?.regularFileContents, Data([0,255,1]))
            XCTAssertEqual(result.fileWrappers?["external-link"]?.symbolicLinkDestinationURL?.relativeString, "../unfollowed")
            let destination = URL(fileURLWithPath: output).appendingPathComponent("Swift Shelf \(category).fountage")
            XCTAssertFalse(FileManager.default.fileExists(atPath: destination.path))
            try result.write(to: destination, options: .atomic, originalContentsURL: nil)
        }
    }
}
