import XCTest
import AppKit
import PDFKit
@testable import Fountage

final class OutlineOutputTests: XCTestCase {
    func testOutlineVisibilityKeepsSelectedLevelsAndSourceOrder() throws {
        for newline in ["\n", "\r\n"] {
            let source = (["# Act One", "", "= A private planning synopsis.", "", "INT. ROOM - DAY", "", "Action.", "", "## _**Sequence 世界**_", "", "= The signal returns.", "", "EXT. ROOF - NIGHT", "", "Another action.", "", "/* # Omitted section */"] + (3...6).flatMap { ["", String(repeating: "#", count: $0) + " Level\($0)"] }).joined(separator: newline)
            let output = try ScreenplayOutput.parse(source)
            let ordinary = try XCTUnwrap(PDFDocument(data: ScreenplayPagination(output: output).pdfData()))
            XCTAssertFalse(ordinary.string?.contains("Act One") ?? true)
            XCTAssertFalse(ordinary.string?.contains("planning synopsis") ?? true)
            for level in 1...6 {
                var options = ScreenplayPrintOptions(); options.visibleSectionLevels = [level]
                options.includeSynopses = true
                let layout = try ScreenplayPagination(output: output, options: options)
                let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
                let text = try XCTUnwrap(pdf.string)
                for (candidate, title) in [(1, "Act One"), (2, "Sequence 世界"), (3, "Level3"), (4, "Level4"), (5, "Level5"), (6, "Level6")] {
                    XCTAssertEqual(text.contains(title), candidate == level)
                }
                XCTAssertTrue(text.contains("planning synopsis"))
                XCTAssertTrue(text.contains("The signal returns."))
                XCTAssertFalse(text.contains("Omitted section"))
                XCTAssertFalse(text.contains("#")); XCTAssertFalse(text.contains("= "))
                let lines = layout.pages.flatMap(\.fragments).filter { !$0.generated }.map(\.sourceLine)
                XCTAssertEqual(lines, lines.sorted())
                XCTAssertEqual(ManuscriptAnalysis.parse(source, format: .screenplay).units, 2)
            }
            let section = try XCTUnwrap(output.outline.first { $0.section_level == 2 })
            XCTAssertEqual(section.text, "Sequence 世界")
            XCTAssertEqual(section.runs.first?.start, 0)
            XCTAssertEqual(section.runs.first?.length, section.text.utf16.count)
        }
    }
    @MainActor func testNativeMenuCommitsSnapshotAndCheckmarksBeforeReturning() throws {
        _ = NSApplication.shared
        let suite = "NativeOutputMenuTests." + UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let source = "@MARA\n" + (1...106).map { "Line \($0): signal." }.joined(separator: "\n")
        let model = ScreenplayPreviewModel(source: source, title: "QA", defaults: defaults)
        model.render(); XCTAssertEqual(model.pdf?.pageCount, 3)
        let coordinator = ScreenplayOutputMenu.Coordinator(model: model)
        let menu = coordinator.makeMenu()
        let index = menu.indexOfItem(withTag: ScreenplayOutputMenu.Coordinator.Command.continuations.rawValue)
        XCTAssertGreaterThanOrEqual(index, 0)
        for expected in [false, true, false, true] {
            menu.performActionForItem(at: index)
            XCTAssertEqual(model.continuations, expected)
            XCTAssertEqual(model.pdf?.pageCount, expected ? 3 : 2)
            XCTAssertEqual(menu.item(at: index)?.state, expected ? .on : .off)
            XCTAssertEqual(try XCTUnwrap(PDFDocument(data: XCTUnwrap(model.data))).pageCount, expected ? 3 : 2)
        }
    }
    @MainActor func testOutlineMenuOptionsPersistAndDoNotRewriteProject() throws {
        let suite = "OutlineOutputTests." + UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let source = "Title: SIGNAL\n\n# Act One\n\n= A synopsis.\n\nINT. ROOM - DAY\n\nAction."
        let model = ScreenplayPreviewModel(source: source, title: "QA", defaults: defaults)
        let coordinator = ScreenplayOutputMenu.Coordinator(model: model)
        let menu = coordinator.makeMenu()
        let synopsis = try XCTUnwrap(menu.item(withTag: ScreenplayOutputMenu.Coordinator.Command.synopses.rawValue))
        coordinator.choose(synopsis)
        let sections = try XCTUnwrap(menu.items.first { $0.title == "Section Levels" }?.submenu)
        for level in [1, 3, 6] { sections.performActionForItem(at: level - 1) }
        let reloaded = ScreenplayPrintOptions(defaults: defaults)
        XCTAssertEqual(reloaded.visibleSectionLevels, [1, 3, 6])
        XCTAssertTrue(reloaded.includeSynopses)
        var document = ScreenplayDocument(); document.text = source
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(source.utf8), Data(reopened.text.utf8))
        if let path = ProcessInfo.processInfo.environment["FOUNTAGE_OUTLINE_QA_PATH"] {
            try XCTUnwrap(model.data).write(to: URL(fileURLWithPath: path), options: .atomic)
        }
    }
}
