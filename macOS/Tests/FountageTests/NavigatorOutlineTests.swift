import XCTest
@testable import Fountage

final class NavigatorOutlineTests: XCTestCase {
    let source = "{{%m: Before 🌙}}\r\n\r\n# Act\r\n\r\nINT. ROOM - DAY{{%m: Cue}}{{%m}}\r\n\r\nAction.\r\n\r\n{{%m: Beat}}\r\n\r\n# Two\r\n\r\nEXT. ROOF - NIGHT"
    func entries(_ source: String? = nil, collapsed: Set<OutlineIndex.Key> = [], query: String = "", hidden: Set<ElementKind> = []) -> [NavigatorOutline.Entry] {
        let analysis = ManuscriptAnalysis.parse(source ?? self.source, format: .screenplay)
        return NavigatorOutline.entries(index: OutlineIndex(analysis.headings), markers: analysis.markers, collapsed: collapsed, query: query, hiddenKinds: hidden)
    }
    func labels(_ entries: [NavigatorOutline.Entry]) -> [String] {
        entries.map { $0.row?.heading.heading ?? ($0.marker!.text.isEmpty ? "Marker" : $0.marker!.text) }
    }
    func testSourceOrderDuplicateLineIdentityAndExactRanges() {
        let entries = entries()
        XCTAssertEqual(labels(entries), ["Before 🌙", "Act", "INT. ROOM - DAY", "Cue", "Marker", "Beat", "Two", "EXT. ROOF - NIGHT"])
        XCTAssertEqual(Set(entries.map(\.id)).count, entries.count)
        XCTAssertEqual(entries.map(\.depth), [0, 0, 1, 2, 2, 2, 0, 1])
        XCTAssertTrue(entries[2].row!.hasChildren)
        for entry in entries where entry.marker != nil {
            XCTAssertEqual((source as NSString).substring(with: entry.marker!.range).prefix(4), "{{%m")
        }
    }
    func testFoldingSearchAndHiddenParentsRemainConsistent() {
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: .screenplay).headings)
        let act = index.rows[0].key, scene = index.rows[1].key
        XCTAssertEqual(labels(entries(collapsed: [act, scene])), ["Before 🌙", "Act", "Two", "EXT. ROOF - NIGHT"])
        XCTAssertEqual(labels(entries(collapsed: [scene])), ["Before 🌙", "Act", "INT. ROOM - DAY", "Two", "EXT. ROOF - NIGHT"])
        XCTAssertEqual(labels(entries(collapsed: [act, scene], query: "beat")), ["Act", "INT. ROOM - DAY", "Beat"])
        let markersOnly = entries(collapsed: [act, scene], hidden: [.scene, .section])
        XCTAssertEqual(labels(markersOnly), ["Before 🌙", "Cue", "Marker", "Beat"])
        XCTAssertTrue(markersOnly.allSatisfy { $0.depth == 0 })
        let headingsOnly = entries(hidden: [.marker])
        XCTAssertEqual(labels(headingsOnly), ["Act", "INT. ROOM - DAY", "Two", "EXT. ROOF - NIGHT"])
        XCTAssertFalse(headingsOnly[1].row!.hasChildren)
        XCTAssertEqual(labels(entries(query: "beat", hidden: [.marker])), [])
    }
    func testMarkerOnlyDocumentsAndFilteredExportUseIdenticalVisibleEntries() throws {
        XCTAssertEqual(labels(entries("{{%m}}{{%m: Alone}}")), ["Marker", "Alone"])
        var document = ScreenplayDocument(); document.text = source
        let exported = try OutlineExport.make(from: document, collapsed: [], query: "beat")
        XCTAssertEqual(exported.text, "# Act\r\n\r\nINT. ROOM - DAY\r\n\r\n{{%m: Beat}}\r\n\r\n")
        let markerOnly = try OutlineExport.make(from: document, collapsed: [], query: "", hiddenKinds: [.section, .scene])
        XCTAssertEqual(markerOnly.text, "{{%m: Before 🌙}}\r\n\r\n{{%m: Cue}}\r\n\r\n{{%m}}\r\n\r\n{{%m: Beat}}\r\n\r\n")
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: .screenplay).headings)
        let collapsed = try OutlineExport.make(from: document, collapsed: [index.rows[0].key], query: "")
        XCTAssertEqual(collapsed.text, "{{%m: Before 🌙}}\r\n\r\n# Act\r\n\r\n# Two\r\n\r\nEXT. ROOF - NIGHT\r\n\r\n")
        XCTAssertEqual(Data(document.text.utf8), Data(source.utf8))
    }
    func testMarkerFiltersPersistForEveryCategoryAndPreserveUnknownSettings() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format; document.text = source
            document.metadata["foreign"] = ["keep": true]
            XCTAssertTrue(NavigatorFilters.kinds(for: format).contains(.marker))
            try document.setNavigatorHiddenKinds([.marker])
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(reopened.navigatorHiddenKinds, [.marker])
            XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
            XCTAssertNotNil(reopened.metadata["foreign"])
        }
    }
    func testCombinedNotesMarkersHaveIndependentExactRangesAndFolding() throws {
        let source = "[[Before 🌙]]\r\n\r\n# Act\r\n\r\nINT. CAFÉ - DAY [[inline]]{{%m: cue}}[[second]]\r\n\r\n[[Two lines:\r\nZoë 🌙]]\r\n\r\n/*[[omitted]]*/"
        let analysis = ManuscriptAnalysis.parse(source, format: .screenplay)
        let index = OutlineIndex(analysis.headings)
        func visible(_ collapsed: Set<OutlineIndex.Key> = [], _ query: String = "", _ hidden: Set<ElementKind> = []) -> [NavigatorOutline.Entry] {
            NavigatorOutline.entries(index: index, markers: analysis.markers, notes: analysis.notes, collapsed: collapsed, query: query, hiddenKinds: hidden)
        }
        let entries = visible()
        XCTAssertEqual(entries.map(\.kind), [.note, .section, .scene, .note, .marker, .note, .note])
        XCTAssertEqual(Set(entries.map(\.id)).count, entries.count)
        XCTAssertEqual(entries.compactMap(\.annotation).map { (source as NSString).substring(with: $0.range) }, ["[[Before 🌙]]", "[[inline]]", "{{%m: cue}}", "[[second]]", "[[Two lines:\r\nZoë 🌙]]"])
        let act = index.rows[0].key
        XCTAssertEqual(visible([act]).map(\.kind), [.note, .section])
        let search = visible([act], "Zoë")
        XCTAssertEqual(search.map(\.kind), [.section, .scene, .note])
        XCTAssertEqual(search.last?.note?.text, "Two lines:\r\nZoë 🌙")
        XCTAssertEqual(visible([], "Zoë", [.note]).count, 0)
        let annotations = visible([act], "", [.section, .scene])
        XCTAssertEqual(annotations.count, 5)
        XCTAssertTrue(annotations.allSatisfy { $0.depth == 0 })
    }
    func testCombinedNoteExportMatchesVisibilityAndPreservesMultilineDirectivesOnce() throws {
        var document = ScreenplayDocument()
        document.text = "# Act\r\n\r\nINT. ROOM - DAY [[inline]]{{%m: cue}}\r\n\r\nAction stays out.\r\n\r\n[[Two lines:\r\nZoë 🌙]]"
        let all = try OutlineExport.make(from: document, collapsed: [], query: "")
        XCTAssertEqual(all.text, "# Act\r\n\r\nINT. ROOM - DAY\r\n\r\n[[inline]]\r\n\r\n{{%m: cue}}\r\n\r\n[[Two lines:\r\nZoë 🌙]]\r\n\r\n")
        let hidden = try OutlineExport.make(from: document, collapsed: [], query: "", hiddenKinds: [.note, .marker])
        XCTAssertEqual(hidden.text, "# Act\r\n\r\nINT. ROOM - DAY\r\n\r\n")
        let filtered = try OutlineExport.make(from: document, collapsed: [], query: "Zoë")
        XCTAssertEqual(filtered.text, "# Act\r\n\r\nINT. ROOM - DAY\r\n\r\n[[Two lines:\r\nZoë 🌙]]\r\n\r\n")
        let reopened = try ScreenplayDocument(wrapper: filtered.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(filtered.text.utf8))
        XCTAssertEqual(ManuscriptAnalysis.parse(reopened.text, format: .screenplay).notes.count, 1)
        XCTAssertFalse(all.text.contains("Action stays out"))
        XCTAssertFalse(document.text.utf8.elementsEqual(all.text.utf8))
    }

}
