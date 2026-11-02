import XCTest
import AppKit
@testable import Fountage

final class SourceLineTests: XCTestCase {
    func testLineRangesCoverExactUTF16SourceWithoutInventingParserLines() {
        for source in ["", "\n", "\r\n\n", "Zoë 🌙\u{2028}Café\n世界\r\nEnd", "a\u{2029}b\rc\n"] {
            let ns = source as NSString
            let ranges = EditorPosition.lineRanges(in: source)
            XCTAssertEqual(ranges.map { ns.substring(with: $0) }.joined().utf8.map { $0 }, Array(source.utf8))
            for (line, range) in ranges.enumerated() {
                XCTAssertEqual(EditorPosition.line(in: source, at: range.location), line)
                XCTAssertGreaterThan(range.length, 0)
            }
        }
    }

    @MainActor
    func testUnicodeSeparatorsDoNotShiftFollowingDialogueStyles() {
        for separator in ["\u{2028}", "\u{2029}", "\r"] {
            let source = "An action\(separator)continues.\n\nINT. ROOM - DAY\n\nMARA\n(quietly)\nHello, 世界.\n"
            let editor = NSTextView()
            editor.string = source
            let selected = (source as NSString).range(of: "世界")
            editor.setSelectedRange(selected)
            ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
            func paragraph(_ marker: String) -> NSParagraphStyle {
                let offset = (source as NSString).range(of: marker).location
                return editor.textStorage!.attribute(.paragraphStyle, at: offset, effectiveRange: nil) as! NSParagraphStyle
            }
            XCTAssertGreaterThan(paragraph("MARA").headIndent, paragraph("Hello").headIndent)
            XCTAssertGreaterThan(paragraph("Hello").headIndent, 0)
            XCTAssertEqual(paragraph("INT.").headIndent, 0)
            XCTAssertEqual(editor.selectedRange(), selected)
            XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
        }
    }
}
