import XCTest
@testable import Fountage

final class ReferenceLibraryTests: XCTestCase {
    func testAllFormatsRoundTripManualDetailsAndExactSource() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument()
            document.format = format
            document.text = "Zoë 🌙\r\nOriginal manuscript."
            let character = ReferenceCard(kind: .character, name: "Zoë", notes: "Background\nSecond line")
            let location = ReferenceCard(kind: .location, name: "CAFÉ", notes: "Production notes")
            try document.saveReferenceCard(character)
            try document.saveReferenceCard(location)
            let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".fountage")
            defer { try? FileManager.default.removeItem(at: url) }
            try document.makeWrapper(contentType: .fountage).write(to: url, options: .atomic, originalContentsURL: nil)
            let reopened = try ScreenplayDocument(wrapper: FileWrapper(url: url), contentType: .fountage)
            XCTAssertEqual(Set(reopened.referenceCards.map(\.id)), Set([character.id, location.id]))
            XCTAssertEqual(reopened.referenceCards.first { $0.id == character.id }?.notes, character.notes)
            XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
        }
    }
    func testUpdatesAndRemovalPreserveUnknownRecordsAndFields() throws {
        var document = ScreenplayDocument()
        var info = document.metadata
        var own = info[ScreenplayDocument.namespace] as! [String: Any]
        own["referenceCards"] = ["known": ["kind": "character", "name": "MARA", "notes": "old", "future": 42], "unknown": ["kind": "future", "payload": "keep"]]
        info[ScreenplayDocument.namespace] = own
        document.metadata = info
        let original = document.referenceCards[0]
        var card = original
        card.notes = "new"
        try document.saveReferenceCard(card, replacing: original)
        var records = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["referenceCards"] as! [String: Any]
        XCTAssertEqual((records["known"] as! [String: Any])["future"] as? Int, 42)
        try document.removeReferenceCard(card)
        records = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["referenceCards"] as! [String: Any]
        XCTAssertNil(records["known"])
        XCTAssertEqual((records["unknown"] as! [String: Any])["payload"] as? String, "keep")
    }
    func testInvalidContainerAndBlankNameAreNotOverwritten() {
        var document = ScreenplayDocument()
        XCTAssertThrowsError(try document.saveReferenceCard(ReferenceCard(kind: .character, name: " \n")))
        XCTAssertTrue(document.referenceCards.isEmpty)
        var info = document.metadata
        var own = info[ScreenplayDocument.namespace] as! [String: Any]
        own["referenceCards"] = ["future array"]
        info[ScreenplayDocument.namespace] = own
        document.metadata = info
        XCTAssertFalse(document.canEditReferenceCards)
        XCTAssertThrowsError(try document.saveReferenceCard(ReferenceCard(kind: .location, name: "ROOM")))
        XCTAssertEqual((document.metadata[ScreenplayDocument.namespace] as! [String: Any])["referenceCards"] as? [String], ["future array"])
    }
    func testStaleEditRemovalAndIdentityCollisionsRejectAtomically() throws {
        var document = ScreenplayDocument()
        let original = ReferenceCard(kind: .character, name: "Zoë", notes: "Old details")
        try document.saveReferenceCard(original)
        var updated = original; updated.notes = "Latest details"
        try document.saveReferenceCard(updated, replacing: original)
        let before = document.metadata
        var stale = original; stale.notes = "Stale edit"
        XCTAssertThrowsError(try document.saveReferenceCard(stale, replacing: original))
        XCTAssertThrowsError(try document.removeReferenceCard(original))
        XCTAssertThrowsError(try document.saveReferenceCard(original))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        try document.removeReferenceCard(updated)
        let removed = document.metadata
        XCTAssertThrowsError(try document.saveReferenceCard(stale, replacing: original))
        XCTAssertThrowsError(try document.removeReferenceCard(updated))
        XCTAssertTrue(NSDictionary(dictionary: removed).isEqual(to: document.metadata))
    }

    func testUnsupportedNamespaceAndMalformedExistingRecordsStayIntact() throws {
        for value: Any in ["future namespace", ["referenceCards": NSNull()], ["referenceCards": ["fixed": ["kind": "future", "payload": [1, 2, 3]]]]] {
            var document = ScreenplayDocument()
            document.metadata[ScreenplayDocument.namespace] = value
            let before = document.metadata
            let card = ReferenceCard(id: "fixed", kind: .location, name: "ROOM")
            XCTAssertThrowsError(try document.saveReferenceCard(card))
            XCTAssertThrowsError(try document.removeReferenceCard(card))
            XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        }
    }

    func testNoOpUnicodeChangesAndUnknownFieldsAcrossReopen() throws {
        var document = ScreenplayDocument()
        document.text = "INT. CAFÉ - DAY\r\n\r\nZoë waits 🌙."
        let original = ReferenceCard(kind: .location, name: "CAFÉ", notes: "é")
        try document.saveReferenceCard(original)
        let before = document.metadata
        XCTAssertFalse(try document.saveReferenceCard(original, replacing: original))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        // Canonically equivalent text is still a distinct saved edit and stale snapshot.
        var updated = original; updated.notes = "e\u{301}"
        XCTAssertTrue(try document.saveReferenceCard(updated, replacing: original))
        XCTAssertThrowsError(try document.removeReferenceCard(original))
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        var records = own["referenceCards"] as! [String: Any]
        var record = records[original.id] as! [String: Any]
        record["future"] = ["keep": true]; records[original.id] = record
        records["unknown"] = ["kind": "future", "payload": [0, 255]]
        own["referenceCards"] = records; document.metadata[ScreenplayDocument.namespace] = own
        updated.name = "CAFÉ ROOF"
        var expected = original; expected.notes = "e\u{301}"
        try document.saveReferenceCard(updated, replacing: expected)
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
        XCTAssertTrue(try XCTUnwrap(reopened.referenceCards.first).matches(updated))
        let reopenedRecords = (reopened.metadata[ScreenplayDocument.namespace] as! [String: Any])["referenceCards"] as! [String: Any]
        let expectedRecords = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["referenceCards"] as! [String: Any]
        XCTAssertTrue(NSDictionary(dictionary: expectedRecords).isEqual(to: reopenedRecords))
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
    }

}
