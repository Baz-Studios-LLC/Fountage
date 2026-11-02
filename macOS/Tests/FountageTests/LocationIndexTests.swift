import XCTest
@testable import Fountage

final class LocationIndexTests: XCTestCase {
    func testSceneLocationsDecodeAndJumpToExactLines() {
        let source = "Title: 🖋️\r\n\r\nINT. CAFÉ - DAY #1#\r\n\r\nZoë arrives.\r\n\r\nEXT. CAFÉ - NIGHT #2#\r\n\r\n/*\r\nINT. OMITTED - DAY\r\n*/"
        let analysis = ManuscriptAnalysis.parse(source, format: .screenplay)
        XCTAssertEqual(analysis.locations, ["CAFÉ": [2, 6]])
        for line in analysis.locations["CAFÉ"] ?? [] {
            let selection = EditorJump(line: line).selection(in: source)
            let tail = (source as NSString).substring(from: selection.location)
            XCTAssertTrue(tail.hasPrefix(line == 2 ? "INT. CAFÉ" : "EXT. CAFÉ"))
        }
        XCTAssertTrue(ManuscriptAnalysis.parse("Action only.", format: .screenplay).locations.isEmpty)
        XCTAssertEqual(ManuscriptAnalysis.parse("INT. PARK - DAY", format: .screenplay).locations, ["PARK": [0]])
    }

    func testOtherFormatsDoNotInferScreenplayLocations() {
        for format in DocumentFormat.allCases where format != .screenplay {
            XCTAssertTrue(ManuscriptAnalysis.parse("INT. ROOM - DAY\n\nBody.", format: format).locations.isEmpty)
        }
    }
}
