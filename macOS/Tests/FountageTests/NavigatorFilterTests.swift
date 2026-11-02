import XCTest
import AppKit
@testable import Fountage

final class NavigatorFilterTests: XCTestCase {
    let source = "# One\n\nINT. A - DAY\n\n= A synopsis\n\n## Detail\n\n.The roof\n\n===\n\n# Two\n\nEXT. B - NIGHT"
    func testHiddenParentsDoNotTrapChildrenOrLeavePhantomIndentationAndDisclosures() throws {
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: .screenplay).headings)
        let sceneOnly: Set<ElementKind> = [.section, .synopsis, .pageBreak]
        let visible = index.visibleRows(collapsed: index.expandableKeys, query: "", hiddenKinds: sceneOnly)
        XCTAssertEqual(visible.map(\.heading.heading), ["INT. A - DAY", "The roof", "EXT. B - NIGHT"])
        XCTAssertTrue(visible.allSatisfy { $0.depth == 0 && !$0.hasChildren })
        let search = index.visibleRows(collapsed: index.expandableKeys, query: "roof", hiddenKinds: sceneOnly)
        XCTAssertEqual(search.map(\.heading.heading), ["The roof"])
        let synopsis = index.visibleRows(collapsed: index.expandableKeys, query: "", hiddenKinds: [.section, .scene, .pageBreak])
        XCTAssertEqual(synopsis.map(\.heading.heading), ["A synopsis"])
        XCTAssertEqual(synopsis.first?.depth, 0)
    }
    func testEveryScreenplayTypeCombinationExportMatchesDisplayedRows() throws {
        let index = OutlineIndex(ManuscriptAnalysis.parse(source, format: .screenplay).headings)
        let kinds = NavigatorFilters.kinds(for: .screenplay)
        var document = ScreenplayDocument(); document.text = source
        for mask in 0..<16 {
            let hidden = Set(kinds.enumerated().filter { mask & (1 << $0.offset) != 0 }.map(\.element))
            for collapsed in [Set<OutlineIndex.Key>(), index.expandableKeys] {
                let visible = index.visibleRows(collapsed: collapsed, query: "", hiddenKinds: hidden)
                XCTAssertTrue(visible.allSatisfy { !hidden.contains($0.heading.kind) })
                if visible.isEmpty {
                    XCTAssertThrowsError(try OutlineExport.make(from: document, collapsed: collapsed, query: "", hiddenKinds: hidden))
                } else {
                    let exported = try OutlineExport.make(from: document, collapsed: collapsed, query: "", hiddenKinds: hidden)
                    let reparsed = ManuscriptAnalysis.parse(exported.text, format: .screenplay).headings
                    XCTAssertEqual(reparsed.map(\.heading), visible.map(\.heading.heading), "Mask \(mask)")
                    XCTAssertEqual(reparsed.map(\.kind), visible.map(\.heading.kind), "Mask \(mask)")
                }
            }
        }
    }
    func testSavedFiltersKeepUnknownMetadataAndAttachmentsAndAllCategoryTypesAreCovered() throws {
        var document = ScreenplayDocument(); document.text = source
        var info = document.metadata, own = info[ScreenplayDocument.namespace] as! [String: Any]
        own["navigatorFilters"] = ["version": 1, "hiddenKinds": ["futureEntry"], "future": ["keep": true]]
        info[ScreenplayDocument.namespace] = own; info["foreign"] = "keep"; document.metadata = info
        try document.setNavigatorHiddenKinds([.section, .synopsis])
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let file = FileWrapper(regularFileWithContents: Data([0, 255])); file.preferredFilename = "keep.bin"; wrapper.addFileWrapper(file)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(reopened.navigatorHiddenKinds, [.section, .synopsis]); XCTAssertEqual(reopened.text, source)
        let settings = (reopened.metadata[ScreenplayDocument.namespace] as! [String: Any])["navigatorFilters"] as! [String: Any]
        XCTAssertTrue((settings["hiddenKinds"] as! [String]).contains("futureEntry")); XCTAssertNotNil(settings["future"])
        XCTAssertEqual(reopened.metadata["foreign"] as? String, "keep")
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["keep.bin"]?.regularFileContents, Data([0, 255]))
        for format in DocumentFormat.allCases {
            let headings = ManuscriptAnalysis.parse(format.starter(for: format.presets.last!), format: format).headings
            let choices = Set(NavigatorFilters.kinds(for: format))
            XCTAssertTrue(Set(headings.map(\.kind)).isSubset(of: choices), format.rawValue)
            XCTAssertTrue(OutlineIndex(headings).visibleRows(collapsed: [], query: "", hiddenKinds: choices).isEmpty)
        }
    }
    @MainActor func testNativeMenuActionsUpdateVisibilityCheckmarksAndPreserveInactiveKinds() throws {
        var document = ScreenplayDocument(); document.text = source
        try document.setNavigatorHiddenKinds([.chapter])
        let coordinator = OutlineFilterMenu.Coordinator(kinds: NavigatorFilters.kinds(for: .screenplay), format: .screenplay,
            hidden: document.navigatorHiddenKinds) { desired in try! document.setNavigatorHiddenKinds(desired); return true }
        let menu = coordinator.makeMenu()
        XCTAssertEqual(menu.item(withTitle: "Scenes")?.state, .on)
        menu.performActionForItem(at: menu.indexOfItem(withTitle: "Scenes"))
        XCTAssertEqual(menu.item(withTitle: "Scenes")?.state, .off)
        XCTAssertEqual(document.navigatorHiddenKinds, [.scene, .chapter])
        menu.performActionForItem(at: menu.indexOfItem(withTitle: "Hide All"))
        XCTAssertTrue(Set(coordinator.kinds).isSubset(of: document.navigatorHiddenKinds))
        XCTAssertEqual(menu.item(withTitle: "Hide All")?.isEnabled, false)
        menu.performActionForItem(at: menu.indexOfItem(withTitle: "Show All"))
        XCTAssertEqual(document.navigatorHiddenKinds, [.chapter])
        XCTAssertEqual(menu.item(withTitle: "Show All")?.isEnabled, false)
        XCTAssertEqual(document.text, source)
    }
    @MainActor func testUnsupportedFiltersRejectWithoutChangingMenuOrDocument() throws {
        for record: Any in ["foreign", ["version": 2], ["version": true], ["hiddenKinds": [false]]] {
            var document = ScreenplayDocument()
            var info = document.metadata, own = info[ScreenplayDocument.namespace] as! [String: Any]
            own["navigatorFilters"] = record; info[ScreenplayDocument.namespace] = own; document.metadata = info
            let before = try JSONSerialization.data(withJSONObject: document.metadata, options: .sortedKeys)
            let coordinator = OutlineFilterMenu.Coordinator(kinds: [.scene], format: .screenplay, hidden: []) { desired in
                do { try document.setNavigatorHiddenKinds(desired); return true } catch { return false }
            }
            let menu = coordinator.makeMenu(); menu.performActionForItem(at: menu.indexOfItem(withTitle: "Scenes"))
            XCTAssertEqual(menu.item(withTitle: "Scenes")?.state, .on)
            XCTAssertEqual(try JSONSerialization.data(withJSONObject: document.metadata, options: .sortedKeys), before)
        }
        var fresh = ScreenplayDocument(); let before = fresh.metadata
        try fresh.setNavigatorHiddenKinds([])
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: fresh.metadata))
    }
}
