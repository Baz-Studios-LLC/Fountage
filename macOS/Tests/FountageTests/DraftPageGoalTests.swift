import XCTest
@testable import Fountage

@MainActor final class DraftPageGoalTests: XCTestCase {
    private let source = "Title: Test\nAuthor: Writer\n\nINT. ROOM - DAY\n\nA signal.\n\n===\n\nEXT. ROOF - NIGHT\n\nStars."
    func testBodyPagesMatchHighlandFixturesAndExcludeTitle() throws {
        XCTAssertEqual(try DraftPageCount.calculate(source: "", options: .init()), 0)
        XCTAssertEqual(try DraftPageCount.calculate(source: "Title: Test\n\n", options: .init()), 0)
        XCTAssertEqual(try DraftPageCount.calculate(source: "[[private]]\n\n/*hidden*/", options: .init()), 0)
        XCTAssertEqual(try DraftPageCount.calculate(source: "Title: Test\nAuthor: Writer\n\nINT. ROOM - DAY\n\nA signal.", options: .init()), 1)
        XCTAssertEqual(try DraftPageCount.calculate(source: source, options: .init()), 2)
        var options = ScreenplayPrintOptions(); options.includeTitlePage = false
        XCTAssertEqual(try DraftPageCount.calculate(source: source, options: options), 2)
        // Title overflow does not prevent counting the otherwise valid body.
        let longTitle = "Title: " + String(repeating: "Long title ", count: 500) + "\n\nA signal."
        XCTAssertEqual(try DraftPageCount.calculate(source: longTitle, options: options), 1)
        XCTAssertThrowsError(try DraftPageCount.calculate(source: "Invalid\0source", options: options))
    }
    func testCountsShareActualPaginationForPaperFontsAndInclusionOptions() throws {
        let manuscript = "Title: Zoë 🌙\r\n\r\n# One\r\n\r\n= Synopsis\r\n\r\nINT. CAFÉ - NIGHT\r\n\r\n" + String(repeating: "The signal carries a message across the stars.\r\n\r\n", count: 400)
        var counts: [Int] = []
        for paper in ScreenplayPrintOptions.Paper.allCases {
            for font in ["Courier", "Helvetica"] {
                var options = ScreenplayPrintOptions(); options.paper = paper
                options.bodyFontFamily = font; options.includeSynopses = true; options.visibleSectionLevels = [1]
                let expected = try ScreenplayPagination(output: ScreenplayOutput.parse(manuscript), options: options)
                let count = try DraftPageCount.calculate(source: manuscript, options: options)
                XCTAssertEqual(count, expected.pages.filter { $0.number != nil }.count)
                counts.append(count)
            }
        }
        XCTAssertGreaterThan(Set(counts).count, 1, "The count must respond to layout changes")
    }
    func testPageTargetPreservesOtherTargetsUnknownMetadataAndAttachmentsOnReopen() throws {
        var document = ScreenplayDocument(); document.text = source
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        own["goals"] = ["future": ["keep": true]]; document.metadata[ScreenplayDocument.namespace] = own
        document.draftWordGoal = 1000; document.draftCharacterGoal = 5000; document.draftPageGoal = 10
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let attachment = FileWrapper(regularFileWithContents: Data([0, 255])); attachment.preferredFilename = "private.bin"; wrapper.addFileWrapper(attachment)
        var reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(reopened.draftPageGoal, 10)
        XCTAssertEqual(try DraftPageCount.calculate(source: reopened.text, options: reopened.outputOptions()), 2)
        reopened.draftPageGoal = nil
        XCTAssertEqual(reopened.draftWordGoal, 1000); XCTAssertEqual(reopened.draftCharacterGoal, 5000)
        XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
        let goals = (reopened.metadata[ScreenplayDocument.namespace] as! [String: Any])["goals"] as! [String: Any]
        XCTAssertNotNil(goals["future"])
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        reopened.draftPageGoal = -1; XCTAssertNil(reopened.draftPageGoal)
    }
    func testDebouncedCountRejectsCancelledAndSupersededRequestsAndRecovers() async {
        let model = DraftPageCount()
        let older = PageGoalRequest(source: ExactSource(source), options: .init(), enabled: true)
        let latest = PageGoalRequest(source: ExactSource("A signal."), options: .init(), enabled: true)
        let task = Task { await model.refresh(older, delay: .seconds(60)) }
        await Task.yield()
        await model.refresh(latest, delay: .zero)
        task.cancel(); await task.value
        XCTAssertEqual(model.count, 1); XCTAssertEqual(model.resolvedRequest, latest)
        await model.refresh(PageGoalRequest(source: ExactSource("\0"), options: .init(), enabled: true), delay: .zero)
        XCTAssertNil(model.count); XCTAssertNotNil(model.error)
        await model.refresh(older, delay: .zero)
        XCTAssertEqual(model.count, 2); XCTAssertNil(model.error)
        await model.refresh(PageGoalRequest(source: older.source, options: older.options, enabled: false), delay: .zero)
        XCTAssertNil(model.count); XCTAssertNil(model.resolvedRequest)
    }
}
