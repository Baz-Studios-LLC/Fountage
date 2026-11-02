import XCTest
@testable import Fountage

final class ScreenplayOutputTests: XCTestCase {
    func testOutputBridgeRetainsPrintDataAndStyleCoordinates() throws {
        let source = "Title: Example\r\n\r\n# Act One\r\n\r\n**INT. ROOM - DAY #12A#**\r\n\r\n@Zoë ^\r\nHello _**世界**_ [[note]].\r\n\r\n===\r\n\r\n/* omitted */"
        let output = try ScreenplayOutput.parse(source)
        XCTAssertEqual(output.title.first?.text, "Title: Example")
        XCTAssertEqual(output.outline.count, 1)
        let scene = try XCTUnwrap(output.body.first { $0.kind == .scene })
        XCTAssertEqual(scene.scene_number, "12A")
        XCTAssertEqual(scene.text, "INT. ROOM - DAY")
        XCTAssertEqual(scene.source_line, 4)
        let cue = try XCTUnwrap(output.body.first { $0.kind == .character })
        XCTAssertEqual(cue.text, "Zoë")
        XCTAssertTrue(cue.dual_dialogue)
        let dialogue = try XCTUnwrap(output.body.first { $0.kind == .dialogue })
        let styled = try XCTUnwrap(dialogue.runs.first { $0.style.bold && $0.style.underline })
        XCTAssertEqual((dialogue.text as NSString).substring(with: NSRange(location: styled.start, length: styled.length)), "世界")
        XCTAssertFalse(output.body.contains { $0.text.contains("note") || $0.text.contains("omitted") })
        XCTAssertTrue(output.body.contains { $0.kind == .pageBreak })
        var document = ScreenplayDocument()
        document.text = source
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
    }
    func testTitleFieldsPreserveMultilineValuesAndRebaseStyles() throws {
        for newline in ["\n", "\r\n"] {
            let source = ["Title: **世界**", "    _Second line_", "Authors:", "\tZoë", "\tAnother author", "Contact:", "    Company: Studio", "Draft date: Tomorrow", "Custom Field: Private", "", "Action."].joined(separator: newline)
            let output = try ScreenplayOutput.parse(source)
            XCTAssertEqual(output.title_fields.map(\.key), ["title", "authors", "contact", "draft date", "custom field"])
            let title = output.title_fields[0]
            XCTAssertEqual(title.placement, .center)
            XCTAssertEqual(title.values.map(\.text), ["世界", "Second line"])
            XCTAssertEqual(title.values[0].runs.first?.start, 0)
            XCTAssertEqual(title.values[0].runs.first?.length, 2)
            XCTAssertEqual(title.values[0].runs.first?.style.bold, true)
            XCTAssertEqual(title.values[1].runs.first?.style.underline, true)
            XCTAssertEqual(output.title_fields[1].values.map(\.text), ["Zoë", "Another author"])
            XCTAssertEqual(output.title_fields[2].values.first?.text, "Company: Studio")
            XCTAssertEqual(output.title_fields[2].placement, .lowerLeft)
            XCTAssertEqual(output.title_fields[4].placement, .metadata)
            XCTAssertFalse(output.body.contains { $0.text.contains("Private") })
        }
    }
    func testNullInputIsRejectedAndEmptyDraftIsValid() throws {
        XCTAssertThrowsError(try ScreenplayOutput.parse("Before\0After"))
        XCTAssertTrue(try ScreenplayOutput.parse("").body.isEmpty)
    }
}
