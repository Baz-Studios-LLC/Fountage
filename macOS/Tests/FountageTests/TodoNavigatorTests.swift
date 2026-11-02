import XCTest
@testable import Fountage

final class TodoNavigatorTests: XCTestCase {
    let source = "# Act\r\n\r\nINT. CAFÉ - DAY\r\n\r\n  [ ] Review Zoë 🌙 [[private]]{{%m: cue}}\r\n[x] Send draft\r\n\r\n# Two\r\n\r\nEXT. ROOF - NIGHT"
    func testTaskRowsFoldSearchFilterAndKeepExactDistinctRanges() throws {
        let analysis = ManuscriptAnalysis.parse(source, format: .screenplay)
        let index = OutlineIndex(analysis.headings)
        func entries(_ collapsed: Set<OutlineIndex.Key> = [], _ query: String = "", _ hidden: Set<ElementKind> = []) -> [NavigatorOutline.Entry] {
            NavigatorOutline.entries(index: index, markers: analysis.markers, notes: analysis.notes, todos: analysis.todos, collapsed: collapsed, query: query, hiddenKinds: hidden)
        }
        let all = entries()
        XCTAssertEqual(all.map(\.kind), [.section, .scene, .todo, .note, .marker, .todo, .section, .scene])
        XCTAssertEqual(Set(all.map(\.id)).count, all.count)
        XCTAssertEqual(all.compactMap(\.todo).map(\.completed), [false, true])
        XCTAssertEqual((source as NSString).substring(with: all[2].annotation!.range), "  [ ] Review Zoë 🌙 [[private]]{{%m: cue}}")
        let act = index.rows[0].key
        XCTAssertEqual(entries([act]).map(\.kind), [.section, .section, .scene])
        XCTAssertEqual(entries([act], "Zoë").map(\.kind), [.section, .scene, .todo])
        XCTAssertTrue(entries([], "Zoë", [.todo]).isEmpty)
        let tasks = entries([act], "", [.section, .scene, .note, .marker])
        XCTAssertEqual(tasks.count, 2); XCTAssertTrue(tasks.allSatisfy { $0.depth == 0 })
    }
    func testVisibleExportRemovesHiddenTaskAnnotationsAndPreservesIndentStateAndProjectData() throws {
        var document = ScreenplayDocument(); document.text = source
        let taskOnly = try OutlineExport.make(from: document, collapsed: [], query: "", hiddenKinds: [.section, .scene, .note, .marker])
        XCTAssertEqual(taskOnly.text, "  [ ] Review Zoë 🌙\r\n\r\n[x] Send draft\r\n\r\n")
        let all = try OutlineExport.make(from: document, collapsed: [], query: "Zoë")
        XCTAssertEqual(all.text, "# Act\r\n\r\nINT. CAFÉ - DAY\r\n\r\n  [ ] Review Zoë 🌙\r\n\r\n")
        XCTAssertFalse(all.text.contains("private")); XCTAssertFalse(all.text.contains("cue"))
        let reopened = try ScreenplayDocument(wrapper: taskOnly.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(taskOnly.text.utf8))
        XCTAssertEqual(ManuscriptAnalysis.parse(reopened.text, format: .screenplay).todos.map(\.completed), [false, true])
        XCTAssertEqual(Data(document.text.utf8), Data(source.utf8))
        for format in DocumentFormat.allCases {
            document.format = format; document.metadata["foreign"] = ["keep": true]
            XCTAssertTrue(NavigatorFilters.kinds(for: format).contains(.todo))
            try document.setNavigatorHiddenKinds([.todo])
            let restored = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(restored.navigatorHiddenKinds, [.todo]); XCTAssertNotNil(restored.metadata["foreign"])
            XCTAssertEqual(Data(restored.text.utf8), Data(source.utf8))
        }
    }
}
