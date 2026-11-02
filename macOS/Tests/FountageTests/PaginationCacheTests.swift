import XCTest
@testable import Fountage

@MainActor final class PaginationCacheTests: XCTestCase {
    func testCachedAndUncachedLayoutsMatchTextStylingGeometryAndSourceReferences() throws {
        let source = "Title: Test\nAuthor: Zoë\n\n# One\n\n= Summary\n\nINT. CAFÉ - DAY #12#\n\nCafé 🌙.\n\nCafe\u{301} 🌙.\n\nMARA\n" + String(repeating: "A **bold**, *italic* and _underlined_ signal arrives. ", count: 140)
            + "\n\nVOICE ^\n" + String(repeating: "The signal echoes. ", count: 75)
            + "\n\n===\n\n" + ScreenplayPerformanceTests.featureDraft(scenes: 12)
        let output = try ScreenplayOutput.parse(source)
        var options = ScreenplayPrintOptions()
        for variant in 0..<3 {
            if variant == 1 { options.paper = .a4; options.bodyFontFamily = "Helvetica"; options.headingFontFamily = "Times New Roman" }
            if variant == 2 { options.includeTitlePage = false; options.includeSynopses = true; options.visibleSectionLevels = [1]; options.dialogueContinuations = false; options.underlineSceneHeadings = true }
            let cached = try ScreenplayPagination(output: output, options: options)
            let baseline = try ScreenplayPagination(output: output, options: options, cacheEnabled: false)
            XCTAssertEqual(cached.pages.count, baseline.pages.count)
            for (page, expected) in zip(cached.pages, baseline.pages) {
                XCTAssertEqual(page.number, expected.number)
                XCTAssertEqual(page.fragments.count, expected.fragments.count)
                for (fragment, reference) in zip(page.fragments, expected.fragments) {
                    XCTAssertEqual(Data(fragment.text.string.utf8), Data(reference.text.string.utf8))
                    XCTAssertTrue(fragment.text.isEqual(to: reference.text))
                    XCTAssertEqual(fragment.x, reference.x); XCTAssertEqual(fragment.top, reference.top)
                    XCTAssertEqual(fragment.width, reference.width); XCTAssertEqual(fragment.alignment, reference.alignment)
                    XCTAssertEqual(fragment.sourceLine, reference.sourceLine); XCTAssertEqual(fragment.generated, reference.generated)
                }
            }
        }
    }
}
