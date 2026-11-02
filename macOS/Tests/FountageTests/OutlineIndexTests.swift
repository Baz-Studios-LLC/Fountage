import XCTest
@testable import Fountage

final class OutlineIndexTests: XCTestCase {
    private let screenplay = "# Act One\n\n= A signal arrives.\n\n## The call\n\nINT. STATION - NIGHT\n\n= Mara answers.\n\n# Act Two\n\nEXT. ROOF - NIGHT\n"

    func testNestedFoldsRetainIndependentChildState() {
        let index = OutlineIndex(ManuscriptAnalysis.parse(screenplay, format: .screenplay).headings)
        XCTAssertEqual(index.rows.map(\.depth), [0, 1, 1, 2, 3, 0, 1])
        let firstAct = index.rows[0].key
        let sequence = index.rows[2].key
        XCTAssertEqual(index.visibleRows(collapsed: [firstAct, sequence], query: "").map(\.heading.heading),
                       ["Act One", "Act Two", "EXT. ROOF - NIGHT"])
        XCTAssertEqual(index.visibleRows(collapsed: [sequence], query: "").map(\.heading.heading),
                       ["Act One", "A signal arrives.", "The call", "Act Two", "EXT. ROOF - NIGHT"])
        XCTAssertEqual(index.visibleRows(collapsed: [], query: "").count, 7)
    }

    func testSearchRevealsHiddenMatchesWithTheirAncestors() {
        let index = OutlineIndex(ManuscriptAnalysis.parse(screenplay, format: .screenplay).headings)
        let collapsed = index.expandableKeys
        XCTAssertEqual(index.visibleRows(collapsed: collapsed, query: "station").map(\.heading.heading),
                       ["Act One", "The call", "INT. STATION - NIGHT"])
        XCTAssertEqual(index.visibleRows(collapsed: collapsed, query: "MARA").map(\.heading.heading),
                       ["Act One", "The call", "INT. STATION - NIGHT", "Mara answers."])
        XCTAssertTrue(index.visibleRows(collapsed: collapsed, query: "not found").isEmpty)
        XCTAssertEqual(index.visibleRows(collapsed: collapsed, query: "").map(\.heading.heading), ["Act One", "Act Two"])
    }

    func testBodyEditsKeepFoldIdentityAndUpdateUnicodeNavigation() {
        let before = OutlineIndex(ManuscriptAnalysis.parse(screenplay, format: .screenplay).headings)
        let edited = screenplay.replacingOccurrences(of: "INT. STATION", with: "Zoë watches 🌙.\n\nINT. STATION")
        let after = OutlineIndex(ManuscriptAnalysis.parse(edited, format: .screenplay).headings)
        XCTAssertEqual(before.keys, after.keys)
        let station = after.rows[3].heading
        XCTAssertGreaterThan(station.line, before.rows[3].heading.line)
        let range = EditorJump(line: station.line, focusEditor: false).selection(in: edited)
        XCTAssertTrue((edited as NSString).substring(from: range.location).hasPrefix("INT. STATION"))
        XCTAssertEqual(after.visibleRows(collapsed: [before.rows[0].key], query: "").map(\.heading.heading),
                       ["Act One", "Act Two", "EXT. ROOF - NIGHT"])
    }

    func testDuplicateHeadingsRemainIndependentAndStructuralEditsChangeIdentity() {
        let source = "# Act\n\nINT. ROOM - DAY\n\n# Act\n\nEXT. ROOM - NIGHT\n"
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: .screenplay).headings)
        XCTAssertNotEqual(index.rows[0].key, index.rows[2].key)
        XCTAssertEqual(index.visibleRows(collapsed: [index.rows[0].key], query: "").map(\.heading.heading),
                       ["Act", "Act", "EXT. ROOM - NIGHT"])
        let revised = OutlineIndex(ManuscriptAnalysis.parse("# Act\n\n" + source, format: .screenplay).headings)
        XCTAssertNotEqual(index.keys, revised.keys)
        let nested = OutlineIndex(ManuscriptAnalysis.parse(source.replacingOccurrences(of: "# Act", with: "## Act"), format: .screenplay).headings)
        XCTAssertNotEqual(index.keys, nested.keys)
    }

    func testComicPagesAndAcademicSectionsUseTheirOwnHierarchy() {
        let comic = OutlineIndex(ManuscriptAnalysis.parse("PAGE 1\n\nPanel 1\n\nPanel 2\n\nPAGE 2\n\nPanel 1", format: .comic).headings)
        XCTAssertEqual(comic.visibleRows(collapsed: [comic.rows[0].key], query: "").map(\.heading.heading), ["PAGE 1", "PAGE 2", "Panel 1"])
        let paper = OutlineIndex(ManuscriptAnalysis.parse("### Context\n\n# Introduction\n\n### Evidence\n\n## Method\n\n# Conclusion", format: .academic).headings)
        XCTAssertEqual(paper.rows.map(\.depth), [0, 0, 1, 1, 0])
        XCTAssertEqual(paper.visibleRows(collapsed: [paper.rows[1].key], query: "").map(\.heading.heading), ["Context", "Introduction", "Conclusion"])
    }
}
