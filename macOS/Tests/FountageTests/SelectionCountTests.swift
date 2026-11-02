import XCTest
@testable import Fountage

final class SelectionCountTests: XCTestCase {
    func testSelectionCountsUnicodeWhitespaceAndMarkupWithoutChangingSource() {
        let source = "Zoë 🌙\r\nCafe\u{301}\t世界 [[a note]]"
        let ns = source as NSString
        XCTAssertEqual(EditorPosition.selectedWords(in: source, ranges: [NSRange(location: 0, length: ns.length)]), 6)
        XCTAssertEqual(EditorPosition.selectedWords(in: source, ranges: [ns.range(of: "[[a note]]")]), 2)
        XCTAssertEqual(EditorPosition.selectedWords(in: source, ranges: [ns.range(of: "\r\n")]), 0)
        XCTAssertNil(EditorPosition.selectedWords(in: source, ranges: [NSRange(location: 0, length: 0)]))
    }
    func testDisjointOverlappingAndStaleSelections() {
        let source = "one two three four"
        XCTAssertEqual(EditorPosition.selectedWords(in: source, ranges: [NSRange(location: 0, length: 7), NSRange(location: 4, length: 9)]), 3)
        XCTAssertEqual(EditorPosition.selectedWords(in: source, ranges: [NSRange(location: 0, length: 3), NSRange(location: 14, length: Int.max)]), 2)
        XCTAssertNil(EditorPosition.selectedWords(in: source, ranges: [NSRange(location: NSNotFound, length: 2)]))
        XCTAssertNil(EditorPosition.selectedWords(in: "", ranges: []))
    }
}
