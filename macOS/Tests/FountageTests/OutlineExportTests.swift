import XCTest
@testable import Fountage

final class OutlineExportTests: XCTestCase {
    func fixture(_ newline: String) -> ScreenplayDocument {
        var document = ScreenplayDocument()
        document.needsTypeSelection = false
        document.text = ["Title: Private title", "Contact: Private contact", "", "# One", "", "**INT. CAFÉ - DAY #A1#** [[private]]", "", "Action must stay out 🌙.", "", "= A synopsis", "", "## Detail", "", ".The roof", "", "[[A note]]", "", "/* EXT. OMITTED - NIGHT */", "", "# Two", "", "EXT. PARK - NIGHT"].joined(separator: newline)
        return document
    }
    func testExportMatchesVisibleHeadingsWithNotesHiddenWithoutLeakingBodyOrNotes() throws {
        for nl in ["\n", "\r\n"] {
            let document = fixture(nl)
            let original = document.text
            let exported = try OutlineExport.make(from: document, collapsed: [], query: "", hiddenKinds: [.note])
            let expected = ["# One", "**INT. CAFÉ - DAY #A1#**", "= A synopsis", "## Detail", ".The roof", "# Two", "EXT. PARK - NIGHT"].joined(separator: nl + nl) + nl + nl
            XCTAssertEqual(Data(exported.text.utf8), Data(expected.utf8))
            for hidden in ["Private", "private", "Action", "A note", "OMITTED"] { XCTAssertFalse(exported.text.contains(hidden)) }
            XCTAssertEqual(Data(document.text.utf8), Data(original.utf8))
            XCTAssertFalse(exported.needsTypeSelection)
            let headings = ManuscriptAnalysis.parse(exported.text, format: .screenplay).headings
            XCTAssertEqual(headings.count, 7)
            XCTAssertTrue(headings.contains { $0.heading == "The roof" && $0.kind == .scene })
        }
    }
    func testCollapsedExportMatchesInstalledHighlandReferenceAndSearchRetainsContext() throws {
        let document = fixture("\n")
        let index = OutlineIndex(ManuscriptAnalysis.parse(document.text, format: .screenplay).headings)
        let one = try XCTUnwrap(index.rows.first { $0.heading.heading == "One" })
        let exported = try OutlineExport.make(from: document, collapsed: [one.key], query: "")
        XCTAssertEqual(exported.text, "# One\n\n# Two\n\nEXT. PARK - NIGHT\n\n")
        let filtered = try OutlineExport.make(from: document, collapsed: [one.key], query: "roof")
        XCTAssertEqual(filtered.text, "# One\n\n## Detail\n\n.The roof\n\n")
        XCTAssertThrowsError(try OutlineExport.make(from: document, collapsed: [], query: "nothing matches"))
    }
    func testNotesExportOnlyMatchingVisibleNotesAndPreservesMultilineSource() throws {
        var document = fixture("\r\n")
        document.text += "\r\n\r\n[[Two lines:\r\nZoë 🌙]]"
        let exported = try OutlineExport.make(from: document, collapsed: [], query: "Zoë", notesOnly: true)
        XCTAssertEqual(exported.text, "[[Two lines:\r\nZoë 🌙]]\r\n\r\n")
        let notes = ManuscriptAnalysis.parse(exported.text, format: .screenplay).notes
        XCTAssertEqual(notes.count, 1)
        XCTAssertTrue(notes[0].text.contains("Zoë 🌙"))
        XCTAssertThrowsError(try OutlineExport.make(from: document, collapsed: [], query: "unmatched", notesOnly: true))
    }
    func testAllEightCategoriesAndLayoutAcademicEditionSurviveSaveReopen() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.preset = format.presets.last!
            document.text = format.starter(for: document.preset)
            if format == .academic { document.academicStyle = AcademicStyle(family: "APA", edition: 7, variant: "student") }
            let exported = try OutlineExport.make(from: document, collapsed: [], query: "")
            XCTAssertEqual(exported.format, format); XCTAssertEqual(exported.preset, document.preset)
            XCTAssertEqual(exported.academicStyle, document.academicStyle)
            let before = ManuscriptAnalysis.parse(document.text, format: format).headings
            let after = ManuscriptAnalysis.parse(exported.text, format: format).headings
            XCTAssertEqual(after.map(\.heading), before.map(\.heading), format.rawValue)
            XCTAssertEqual(after.map(\.kind), before.map(\.kind), format.rawValue)
            let reopened = try ScreenplayDocument(wrapper: exported.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(exported.text.utf8))
            XCTAssertEqual(reopened.format, format); XCTAssertEqual(reopened.preset, document.preset)
            XCTAssertEqual(reopened.academicStyle, document.academicStyle)
        }
    }
    func testOriginalPrivateMetadataAndAttachmentsAreNotCopiedOrModified() throws {
        var original = fixture("\n")
        var metadata = original.metadata
        metadata["private"] = ["secret": "keep in original"]
        original.metadata = metadata
        original.retainedEntries["private.bin"] = .file(Data([0, 255, 1]))
        let record = try JSONSerialization.data(withJSONObject: original.metadata, options: .sortedKeys)
        let exported = try OutlineExport.make(from: original, collapsed: [], query: "")
        XCTAssertNil(exported.metadata["private"]); XCTAssertTrue(exported.retainedEntries.isEmpty)
        let oldOwn = original.metadata[ScreenplayDocument.namespace] as! [String: Any]
        let newOwn = exported.metadata[ScreenplayDocument.namespace] as! [String: Any]
        XCTAssertNotEqual(oldOwn["projectID"] as? String, newOwn["projectID"] as? String)
        XCTAssertEqual(try JSONSerialization.data(withJSONObject: original.metadata, options: .sortedKeys), record)
        XCTAssertEqual(try original.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255, 1]))
        var invalid = original; invalid.text += "\0"
        XCTAssertThrowsError(try OutlineExport.make(from: invalid, collapsed: [], query: ""))
        XCTAssertThrowsError(try OutlineExport.make(from: ScreenplayDocument(), collapsed: [], query: ""))
    }
}
