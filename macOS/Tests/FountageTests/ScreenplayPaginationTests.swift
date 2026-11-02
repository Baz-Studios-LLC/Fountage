import XCTest
import PDFKit
@testable import Fountage

final class ScreenplayPaginationTests: XCTestCase {
    func testPDFContainsFormattedManuscriptAndParallelDialogue() throws {
        let source = """
        Title: **THE LAST SIGNAL**
        Credit: Written by
        Author: Zoë Rivers
        Contact:
            Baz Studios
            writer@example.com

        INT. OBSERVATORY - NIGHT #12A#

        The clock stops. _Moonlight_ falls across the silent instruments.

        MARA
        There hasn't been a transmission in years.

        VOICE ^
        Then I suppose you weren't expecting me.

        [[Private note must not print.]]
        /* Secret deleted scene. */

        ===

        EXT. ROOF - NIGHT

        A signal flickers: 世界 🌙.

        > THE END <
        """
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source))
        let data = try layout.pdfData()
        let pdf = try XCTUnwrap(PDFDocument(data: data))
        XCTAssertEqual(pdf.pageCount, 3)
        XCTAssertEqual(pdf.page(at: 0)?.bounds(for: .mediaBox).size, CGSize(width: 612, height: 792))
        let text = try XCTUnwrap(pdf.string)
        for value in ["THE LAST SIGNAL", "Zoë Rivers", "writer@example.com", "12A", "MARA", "VOICE", "THE END", "世界"] { XCTAssertTrue(text.contains(value), value) }
        for value in ["Title:", "Private note", "Secret deleted", "===", "**", "^"] { XCTAssertFalse(text.contains(value), value) }
        let left = try XCTUnwrap(layout.pages[1].fragments.first { $0.text.string == "MARA" })
        let right = try XCTUnwrap(layout.pages[1].fragments.first { $0.text.string == "VOICE" })
        XCTAssertEqual(left.top, right.top)
        XCTAssertLessThan(left.x, right.x)
        XCTAssertEqual(layout.pages[1].fragments.first?.top, 82)
        XCTAssertNil(layout.pages[0].number)
        XCTAssertEqual(layout.pages[2].number, 2)
        if let path = ProcessInfo.processInfo.environment["FOUNTAGE_PDF_QA_PATH"] {
            try data.write(to: URL(fileURLWithPath: path), options: .atomic)
        }
    }
    func testLongManuscriptWrapsWithoutLosingWordsOrClippingRows() throws {
        let source = (1...100).map { "!Passage\($0) " + String(repeating: "The signal crosses the dark sky. ", count: 8) }.joined(separator: "\n\n")
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source))
        XCTAssertGreaterThan(layout.pages.count, 5)
        let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
        let text = try XCTUnwrap(pdf.string)
        for index in 1...100 { XCTAssertTrue(text.contains("Passage\(index) "), "Passage \(index)") }
        for page in layout.pages {
            for fragment in page.fragments {
                XCTAssertGreaterThanOrEqual(fragment.top, 82)
                XCTAssertLessThanOrEqual(fragment.top, 718)
                XCTAssertGreaterThanOrEqual(fragment.x, 72)
                XCTAssertLessThanOrEqual(fragment.x + fragment.width, 540)
            }
        }
    }
    func testTitleOverflowFailsInsteadOfClippingAndUnknownFieldsStayMetadata() throws {
        let hugeTitle = "Title:\n" + Array(repeating: "    Another title line", count: 80).joined(separator: "\n") + "\n\nAction."
        XCTAssertThrowsError(try ScreenplayPagination(output: ScreenplayOutput.parse(hugeTitle)))
        let source = "Title: Example\nCustom Field: Unprinted metadata\n\nAction."
        let pdf = try XCTUnwrap(PDFDocument(data: ScreenplayPagination(output: ScreenplayOutput.parse(source)).pdfData()))
        XCTAssertFalse(pdf.string?.contains("Unprinted") ?? true)
        XCTAssertEqual(try ScreenplayPagination(output: ScreenplayOutput.parse("")).pages.count, 1)
    }
    func testDialogueUnitMovesTogetherAtPageBoundaryAndSourceStaysExact() throws {
        let prefix = Array(repeating: "!Action.", count: 51).joined(separator: "\n")
        let source = prefix + "\n\nMARA\nOne.\nTwo.\nThree."
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source))
        let cuePage = try XCTUnwrap(layout.pages.firstIndex { $0.fragments.contains { $0.text.string == "MARA" } })
        XCTAssertEqual(cuePage, 1)
        XCTAssertTrue(layout.pages[cuePage].fragments.contains { $0.text.string == "Three." })
        var document = ScreenplayDocument(); document.text = source
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(source.utf8), Data(reopened.text.utf8))
    }
    func testLongSpeechContinuesWithoutDroppingOrRepeatingSource() throws {
        for newline in ["\n", "\r\n"] {
            let source = (["@Zoë (V.O.)"] + (1...130).map { "Utterance\($0): a quiet signal." }).joined(separator: newline)
            let output = try ScreenplayOutput.parse(source)
            let layout = try ScreenplayPagination(output: output)
            XCTAssertGreaterThanOrEqual(layout.pages.count, 3)
            let originals = layout.pages.flatMap(\.fragments).filter { !$0.generated }.map { $0.text.string }
            XCTAssertEqual(originals.joined(), output.body.map(\.text).joined())
            let markers = layout.pages.flatMap(\.fragments).filter(\.generated).map { $0.text.string }
            XCTAssertEqual(markers.filter { $0 == "(MORE)" }.count, layout.pages.count - 1)
            XCTAssertEqual(markers.filter { $0 == "Zoë (V.O.) (CONT'D)" }.count, layout.pages.count - 1)
            for page in layout.pages.dropFirst() {
                XCTAssertEqual(page.fragments.first?.text.string, "Zoë (V.O.) (CONT'D)")
                XCTAssertEqual(page.fragments.first?.top, 82)
            }
            let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
            for index in 1...130 { XCTAssertTrue(pdf.string?.contains("Utterance\(index):") ?? false) }
            for fragment in layout.pages.flatMap(\.fragments) { XCTAssertLessThanOrEqual(fragment.top, 718) }
            if newline == "\n", let path = ProcessInfo.processInfo.environment["FOUNTAGE_CONTINUATION_QA_PATH"] {
                try layout.pdfData().write(to: URL(fileURLWithPath: path), options: .atomic)
            }
        }
    }
    func testDualContinuationRepeatsOnlyTheUnfinishedSpeaker() throws {
        let source = "@LEFT\nShort answer.\n\n@RIGHT ^\n" + (1...120).map { "Response\($0): still speaking." }.joined(separator: "\n")
        let output = try ScreenplayOutput.parse(source)
        let layout = try ScreenplayPagination(output: output)
        XCTAssertGreaterThan(layout.pages.count, 2)
        for page in layout.pages.dropFirst() {
            XCTAssertTrue(page.fragments.contains { $0.text.string.contains("RIGHT (CONT'D)") })
            XCTAssertFalse(page.fragments.contains { $0.text.string.contains("LEFT") })
            XCTAssertTrue(page.fragments.allSatisfy { $0.x >= 342 })
        }
        let originals = layout.pages.flatMap(\.fragments).filter { !$0.generated }
        for line in output.body where !line.text.isEmpty {
            XCTAssertEqual(originals.filter { $0.sourceLine == line.source_line }.map { $0.text.string }.joined(), line.text)
        }
        if let path = ProcessInfo.processInfo.environment["FOUNTAGE_DUAL_CONTINUATION_QA_PATH"] {
            try layout.pdfData().write(to: URL(fileURLWithPath: path), options: .atomic)
        }
    }
    func testContinuationOptionDoesNotChangeManuscriptAndCanRemoveGeneratedCues() throws {
        let source = "@MARA\n" + (1...100).map { "Spoken line \($0)." }.joined(separator: "\n")
        let output = try ScreenplayOutput.parse(source)
        let on = try ScreenplayPagination(output: output)
        let off = try ScreenplayPagination(output: output, dialogueContinuations: false)
        XCTAssertTrue(on.pages.flatMap(\.fragments).contains(where: \.generated))
        XCTAssertFalse(off.pages.flatMap(\.fragments).contains(where: \.generated))
        XCTAssertEqual(on.pages.flatMap(\.fragments).filter { !$0.generated }.map { $0.text.string }, off.pages.flatMap(\.fragments).map { $0.text.string })
        let existingCue = try ScreenplayPagination(output: ScreenplayOutput.parse(source.replacingOccurrences(of: "@MARA", with: "@MARA (CONT'D)")))
        XCTAssertFalse(existingCue.pages.flatMap(\.fragments).contains { $0.text.string.contains("(CONT'D) (CONT'D)") })
        var document = ScreenplayDocument(); document.text = source
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(source.utf8), Data(reopened.text.utf8))
    }
    func testParentheticalStaysWithFollowingSpeechAtSplit() throws {
        // Cue + 51 dialogue lines + parenthetical would previously strand the parenthetical.
        let source = "@MARA\n" + (1...51).map { "Line \($0)." }.joined(separator: "\n") + "\n(quietly)\nThe final signal.\nAnother answer."
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source))
        let lastPage = try XCTUnwrap(layout.pages.last)
        XCTAssertTrue(lastPage.fragments.contains { $0.text.string == "(quietly)" })
        XCTAssertTrue(lastPage.fragments.contains { $0.text.string == "The final signal." })
        for page in layout.pages.dropLast() {
            XCTAssertFalse(page.fragments.last(where: { !$0.generated })?.text.string == "(quietly)")
        }
    }
    func testExcessivelyLongCueFailsWithoutHangingOrTruncating() throws {
        let source = "@" + String(repeating: "CHARACTER ", count: 600) + "\nA line."
        XCTAssertThrowsError(try ScreenplayPagination(output: ScreenplayOutput.parse(source)))
    }

    func testDualSpeechesRetainAllSourceAcrossEveryStartingRow() throws {
        for start in 0..<ScreenplayPagination.rowsPerPage {
            let prefix = Array(repeating: "!Action.", count: start).joined(separator: "\n")
            let source = (prefix.isEmpty ? "" : prefix + "\n\n") + "@LEFT\n" + (1...110).map { "Left \($0)." }.joined(separator: "\n") + "\n\n@RIGHT ^\n" + (1...125).map { "Right \($0)." }.joined(separator: "\n")
            let output = try ScreenplayOutput.parse(source)
            let layout = try ScreenplayPagination(output: output)
            let originals = layout.pages.flatMap(\.fragments).filter { !$0.generated }
            for line in output.body where !line.text.isEmpty {
                XCTAssertEqual(originals.filter { $0.sourceLine == line.source_line }.map { $0.text.string }.joined(), line.text, "Start row \(start), source line \(line.source_line)")
            }
            for fragment in layout.pages.flatMap(\.fragments) { XCTAssertLessThanOrEqual(fragment.top, 718) }
        }
    }

}
