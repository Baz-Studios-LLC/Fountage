import XCTest
import AppKit
@testable import Fountage

final class EmphasisTests: XCTestCase {
    @MainActor
    func testCombinedStylesPreserveUnicodeSourceAndSelection() {
        for ending in ["\n", "\r\n"] {
            let source = "INT. ROOM - DAY" + ending + ending + "🌙 _A **bold** and *italic* phrase_ ***both***"
            let editor = NSTextView()
            editor.string = source
            editor.setSelectedRange(NSRange(location: 3, length: 4))
            ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
            for (word, bold, italic, underline) in [("bold", true, false, true), ("italic", false, true, true), ("both", true, true, false)] {
                let index = (source as NSString).range(of: word).location
                let attributes = editor.textStorage!.attributes(at: index, effectiveRange: nil)
                let traits = NSFontManager.shared.traits(of: attributes[.font] as! NSFont)
                XCTAssertEqual(traits.contains(.boldFontMask), bold)
                XCTAssertEqual(traits.contains(.italicFontMask), italic)
                XCTAssertEqual(attributes[.underlineStyle] as? Int == 1, underline)
            }
            XCTAssertEqual(editor.selectedRange(), NSRange(location: 3, length: 4))
            XCTAssertEqual(Data(editor.string.utf8), Data(source.utf8))
            editor.string = "No emphasis remains."
            ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
            let attributes = editor.textStorage!.attributes(at: 0, effectiveRange: nil)
            XCTAssertNil(attributes[.underlineStyle])
            XCTAssertFalse(NSFontManager.shared.traits(of: attributes[.font] as! NSFont).contains(.italicFontMask))
        }
    }
    func testHiddenTextAndOtherFormatsDoNotIntroduceEmphasis() {
        let source = "[[**note 🌙**]]\n\n/* *omitted* */\n\n*visible*"
        let parsed = ManuscriptAnalysis.parse(source, format: .screenplay)
        XCTAssertEqual(parsed.emphasis.count, 1)
        let span = parsed.emphasis[0]
        XCTAssertEqual((source as NSString).substring(with: NSRange(location: span.start, length: span.length)), "visible")
        for format in DocumentFormat.allCases where format != .screenplay {
            XCTAssertTrue(ManuscriptAnalysis.parse(source, format: format).emphasis.isEmpty)
        }
    }
}
