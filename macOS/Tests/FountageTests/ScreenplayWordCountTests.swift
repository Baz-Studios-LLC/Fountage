import XCTest
@testable import Fountage

final class ScreenplayWordCountTests: XCTestCase {
    func testNotesDoNotAdvanceGoalAndCountsSurviveReopen() throws {
        var document = ScreenplayDocument()
        document.text = "Title: Draft\r\n\r\nINT. ROOM - DAY #1#\r\n\r\nHello **world**."
        document.draftWordGoal = 6
        let before = ManuscriptAnalysis.parse(document.text, format: .screenplay).words
        XCTAssertEqual(before, 5)
        document.text += "\r\n\r\n[[A very long planning note]]\r\n\r\n/* Omitted old scene */"
        let after = ManuscriptAnalysis.parse(document.text, format: .screenplay).words
        XCTAssertEqual(after, before)
        XCTAssertFalse(DraftGoal(target: document.draftWordGoal!, words: after).reached)
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
        XCTAssertEqual(reopened.draftWordGoal, 6)
        XCTAssertEqual(ManuscriptAnalysis.parse(reopened.text, format: .screenplay).words, 5)
        document.text += "\r\n\r\nTomorrow."
        XCTAssertTrue(DraftGoal(target: 6, words: ManuscriptAnalysis.parse(document.text, format: .screenplay).words).reached)
    }
    func testSelectionUsesFullDocumentContextAndOriginalOffsets() {
        for newline in ["\n", "\r\n"] {
            let source = ["Title: Draft", "", "# Outline", "", "INT. CAFÉ - DAY #12A#", "", "🌙 Hello **world** [[private note]].", "", "/* Omitted words */"].joined(separator: newline)
            let analysis = ManuscriptAnalysis.parse(source, format: .screenplay)
            func count(_ ranges: [NSRange]) -> Int? { EditorPosition.selectedWords(in: source, ranges: ranges, manuscriptWords: analysis.word_ranges) }
            let ns = source as NSString
            XCTAssertEqual(count([NSRange(location: 0, length: ns.length)]), analysis.words)
            XCTAssertEqual(count([ns.range(of: "private note")]), 0)
            XCTAssertEqual(count([ns.range(of: "Omitted words")]), 0)
            XCTAssertEqual(count([ns.range(of: "Title: Draft")]), 0)
            XCTAssertEqual(count([ns.range(of: "12A")]), 0)
            XCTAssertEqual(count([ns.range(of: "Hello **world**")]), 2)
            XCTAssertEqual(count([ns.range(of: "CAFÉ")]), 1)
            let word = ns.range(of: "world")
            XCTAssertEqual(count([word, word, NSRange(location: word.location + 1, length: 2)]), 1)
            XCTAssertEqual(count([NSRange(location: word.location - 2, length: 2)]), 0)
            XCTAssertNil(count([NSRange(location: 0, length: 0)]))
            XCTAssertEqual(count([ns.range(of: "🌙")]), 0)
        }
    }
    func testOtherFormatsRetainTheirExistingCountingPolicy() {
        let source = "# Heading\n\nSome words [[note]]"
        for format in DocumentFormat.allCases where format != .screenplay {
            XCTAssertEqual(ManuscriptAnalysis.parse(source, format: format).words, source.split(whereSeparator: \.isWhitespace).count)
        }
    }
}
