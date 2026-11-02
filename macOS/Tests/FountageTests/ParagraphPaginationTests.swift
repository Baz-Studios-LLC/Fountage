import XCTest
import PDFKit
import AppKit
@testable import Fountage

final class ParagraphPaginationTests: XCTestCase {
    func testActionParagraphNeverLeavesOneLineOnEitherSideOfPageBreak() throws {
        for paper in ScreenplayPrintOptions.Paper.allCases {
            var options = ScreenplayPrintOptions(); options.paper = paper
            let limit = Int(floor((paper.size.height - 72 - 82) / ScreenplayPagination.lineHeight)) + 1
            for row in 0..<limit {
                for count in [2, 3, 5, limit + 1, limit * 2 + 1] {
                    let prefix = Array(repeating: "!Prefix.", count: max(0, row - 1)).joined(separator: "\n")
                    let text = (1...count).map { "!Paragraph line \($0)." }.joined(separator: "\n")
                    let source = row == 0 ? text : row == 1 ? "\n" + text : prefix + "\n\n" + text
                    let output = try ScreenplayOutput.parse(source)
                    let layout = try ScreenplayPagination(output: output, options: options)
                    let segments = layout.pages.map { $0.fragments.filter { $0.text.string.hasPrefix("Paragraph line") } }.filter { !$0.isEmpty }
                    XCTAssertTrue(segments.allSatisfy { $0.count >= 2 }, "\(paper) row \(row) count \(count): \(segments.map(\.count))")
                    XCTAssertEqual(segments.flatMap { $0 }.map { $0.text.string }, (1...count).map { "Paragraph line \($0)." })
                    for page in layout.pages { for fragment in page.fragments { XCTAssertLessThanOrEqual(fragment.top, paper.size.height - 72) } }
                }
            }
        }
    }
    func testWrappedActionAndLyricsPreserveStyledUnicodeAndSourceExactly() throws {
        for kind in ["!", "~"] {
            for newline in ["\n", "\r\n"] {
                let prefix = Array(repeating: "!Prefix.", count: 50).joined(separator: newline)
                let source = prefix + newline + newline + kind + "Zoë 🌙: **" + String(repeating: "the signal returns quietly ", count: 18).trimmingCharacters(in: .whitespaces) + "**"
                let output = try ScreenplayOutput.parse(source)
                let layout = try ScreenplayPagination(output: output)
                let lastLine = try XCTUnwrap(output.body.last)
                let segments = layout.pages.map { $0.fragments.filter { $0.sourceLine == lastLine.source_line } }.filter { !$0.isEmpty }
                XCTAssertTrue(segments.allSatisfy { $0.count >= 2 })
                XCTAssertEqual(segments.flatMap { $0 }.map { $0.text.string }.joined(), lastLine.text)
                var hasBold = false
                for fragment in segments.flatMap({ $0 }) {
                    fragment.text.enumerateAttribute(.font, in: NSRange(location: 0, length: fragment.text.length)) { value, _, _ in
                        if let font = value as? NSFont, NSFontManager.shared.traits(of: font).contains(.boldFontMask) { hasBold = true }
                    }
                }
                XCTAssertTrue(hasBold)
                let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
                XCTAssertTrue(pdf.string?.contains("Zoë") ?? false)
                var document = ScreenplayDocument(); document.text = source
                XCTAssertEqual(try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage).text, source)
            }
        }
    }
    func testOneLineParagraphAndForcedBreakRemainValid() throws {
        let source = "!One.\n\n===\n\n!Two."
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source))
        XCTAssertEqual(layout.pages.count, 2)
        XCTAssertTrue(layout.pages[0].fragments.contains { $0.text.string == "One." })
        XCTAssertTrue(layout.pages[1].fragments.contains { $0.text.string == "Two." })
    }
    func testWrappedDialogueParagraphsHaveNoSingleRowTailsOrHeads() throws {
        for paper in ScreenplayPrintOptions.Paper.allCases {
            for continuations in [true, false] {
                var options = ScreenplayPrintOptions(); options.paper = paper; options.dialogueContinuations = continuations
                for lead in 42...58 {
                    let source = "@MARA\n" + (1...lead).map { "Line \($0)." }.joined(separator: "\n") + "\n" + String(repeating: "A quiet signal returns. ", count: 8) + "\n" + (1...20).map { "Closing \($0)." }.joined(separator: "\n")
                    let output = try ScreenplayOutput.parse(source)
                    let paragraph = try XCTUnwrap(output.body.first { $0.text.hasPrefix("A quiet") })
                    let layout = try ScreenplayPagination(output: output, options: options)
                    let segments = layout.pages.map { $0.fragments.filter { !$0.generated && $0.sourceLine == paragraph.source_line } }.filter { !$0.isEmpty }
                    XCTAssertTrue(segments.allSatisfy { $0.count >= 2 }, "\(paper) \(continuations) lead \(lead): \(segments.map(\.count))")
                    for line in output.body where !line.text.isEmpty {
                        XCTAssertEqual(layout.pages.flatMap(\.fragments).filter { !$0.generated && $0.sourceLine == line.source_line }.map { $0.text.string }.joined(), line.text)
                    }
                }
            }
        }
    }

    func testSceneHeadingMovesWithFollowingThreeRowParagraphIncludingSourceBlank() throws {
        let prefix = Array(repeating: "!Prefix.", count: 49).joined(separator: "\n")
        let source = prefix + "\n\nINT. ROOM - DAY\n\n!First.\n!Second.\n!Third."
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source))
        XCTAssertEqual(layout.pages.count, 2)
        XCTAssertFalse(layout.pages[0].fragments.contains { $0.text.string == "INT. ROOM - DAY" })
        XCTAssertEqual(layout.pages[1].fragments.first?.text.string, "INT. ROOM - DAY")
        for text in ["First.", "Second.", "Third."] { XCTAssertTrue(layout.pages[1].fragments.contains { $0.text.string == text }) }
    }
    func testDualWrappedParagraphsRetainEveryCharacterWithoutSingleRowSegments() throws {
        let source = "@LEFT\n" + (1...49).map { "Left \($0)." }.joined(separator: "\n") + "\n" + String(repeating: "The signal is returning. ", count: 10) + "\nEnd.\n\n@RIGHT ^\n" + (1...50).map { "Right \($0)." }.joined(separator: "\n") + "\n" + String(repeating: "A second signal returns. ", count: 10) + "\nEnd."
        let output = try ScreenplayOutput.parse(source)
        let layout = try ScreenplayPagination(output: output)
        for line in output.body where !line.text.isEmpty {
            let segments = layout.pages.map { $0.fragments.filter { !$0.generated && $0.sourceLine == line.source_line } }.filter { !$0.isEmpty }
            XCTAssertEqual(segments.flatMap { $0 }.map { $0.text.string }.joined(), line.text)
            if line.text.hasPrefix("The signal") || line.text.hasPrefix("A second") {
                XCTAssertTrue(segments.allSatisfy { $0.count >= 2 })
            }
        }
    }

    func testBoundaryQAArtifact() throws {
        guard let path = ProcessInfo.processInfo.environment["FOUNTAGE_PARAGRAPH_QA_PATH"] else { return }
        let source = Array(repeating: "!The observatory hums quietly.", count: 49).joined(separator: "\n")
            + "\n\nINT. OBSERVATORY - NIGHT #12A#\n\n!Zoë reads the signal.\n!The clock stops.\n!Nothing moves.\n\nMARA\n"
            + (1...49).map { "Signal \($0) is returning." }.joined(separator: "\n")
            + "\n**" + String(repeating: "A quiet signal returns. ", count: 8).trimmingCharacters(in: .whitespaces) + "**\nEnd of transmission."
        try ScreenplayPagination(output: ScreenplayOutput.parse(source)).pdfData().write(to: URL(fileURLWithPath: path), options: .atomic)
    }

}
