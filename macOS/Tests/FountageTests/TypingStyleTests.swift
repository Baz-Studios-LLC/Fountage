import XCTest
import AppKit
@testable import Fountage

final class TypingStyleTests: XCTestCase {
    @MainActor
    func testCRLFTypingUsesCurrentElementRatherThanFirstLine() {
        let lf = "Title: Café\n\nINT. ROOM - DAY\n\nMARA\n(quietly)\nHello, 世界 🌙.\n\nCUT TO:\n"
        for ending in ["\n", "\r\n"] {
            let source = lf.replacingOccurrences(of: "\n", with: ending)
            let editor = NSTextView()
            editor.string = source
            for marker in ["INT. ROOM", "MARA", "(quietly)", "Hello", "CUT TO:"] {
                let range = (source as NSString).range(of: marker)
                editor.setSelectedRange(NSRange(location: range.location + range.length, length: 0))
                ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
                let displayed = editor.textStorage!.attributes(at: range.location, effectiveRange: nil)
                XCTAssertEqual(editor.typingAttributes[.paragraphStyle] as? NSParagraphStyle, displayed[.paragraphStyle] as? NSParagraphStyle, marker)
                XCTAssertEqual(editor.typingAttributes[.font] as? NSFont, displayed[.font] as? NSFont, marker)
                XCTAssertEqual(editor.typingAttributes[.foregroundColor] as? NSColor, displayed[.foregroundColor] as? NSColor, marker)
                XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
            }
        }
    }

    @MainActor
    func testTypingStylesAgreeAcrossLineEndingsForEveryFormat() {
        for format in DocumentFormat.allCases {
            let lf = format.starter
            let crlf = lf.replacingOccurrences(of: "\n", with: "\r\n")
            let a = NSTextView(), b = NSTextView()
            a.string = lf; b.string = crlf
            for heading in ManuscriptAnalysis.parse(lf, format: format).headings {
                a.setSelectedRange(EditorJump(line: heading.line).selection(in: lf))
                b.setSelectedRange(EditorJump(line: heading.line).selection(in: crlf))
                ManuscriptStyle.apply(to: a, format: format, preset: format.presets[0], size: 16)
                ManuscriptStyle.apply(to: b, format: format, preset: format.presets[0], size: 16)
                XCTAssertEqual(a.typingAttributes[.paragraphStyle] as? NSParagraphStyle, b.typingAttributes[.paragraphStyle] as? NSParagraphStyle, format.rawValue)
                XCTAssertEqual(a.typingAttributes[.font] as? NSFont, b.typingAttributes[.font] as? NSFont, format.rawValue)
            }
        }
    }
}
