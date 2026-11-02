import XCTest
@testable import Fountage

@MainActor final class ScreenplayPerformanceTests: XCTestCase {
    static func featureDraft(scenes: Int = 120, unique: Bool = false) -> String {
        let action = "Zoë follows the signal through the quiet station. The old receiver glows beneath the window while distant lights cross the sky."
        return (1...scenes).map { scene in
            "INT. STATION \(scene) - NIGHT\n\n" + (1...5).map { paragraph in action + (unique ? " Reference \(scene)-\(paragraph)." : "") }.joined(separator: "\n\n")
            + "\n\nMARA\nI can hear the signal. **Someone** is still out there. 🌙\n"
        }.joined(separator: "\n===\n\n")
    }
    func testFeatureLengthPendingPageMilestoneTimingAndSourcePreservation() throws {
        for unique in [false, true] {
            var document = ScreenplayDocument(); document.text = Self.featureDraft(unique: unique)
            let source = document.text
            try document.addMilestone(unit: .pages, target: 1000,
                counts: WritingMilestoneCounts(words: 0, characters: 0, pages: nil))
            let clock = ContinuousClock()
            let started = clock.now
            let output = try ScreenplayOutput.parse(source)
            let parsed = clock.now
            let baseline = try ScreenplayPagination(output: output, cacheEnabled: false)
            let uncached = clock.now
            let layout = try ScreenplayPagination(output: output)
            let paginated = clock.now
            let cache = ManuscriptAnalysisCache()
            MilestoneWorkflow.replaceText(source + "A", in: &document, cache: cache)
            let edited = clock.now
            XCTAssertEqual(layout.pages.count, 120)
            XCTAssertEqual(layout.pages.count, baseline.pages.count)
            XCTAssertEqual(try document.writingMilestones().first?.completed, false)
            XCTAssertEqual(Data(document.text.utf8), Data((source + "A").utf8))
            print("FOUNTAGE_FEATURE_TIMING unique=\(unique) bytes=\(source.utf8.count) pages=\(layout.pages.count) parse=\(started.duration(to: parsed)) uncached=\(parsed.duration(to: uncached)) cached=\(uncached.duration(to: paginated)) edit=\(paginated.duration(to: edited))")
        }
    }
}
