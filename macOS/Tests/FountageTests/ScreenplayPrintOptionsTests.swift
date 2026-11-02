import XCTest
import AppKit
import PDFKit
@testable import Fountage

final class ScreenplayPrintOptionsTests: XCTestCase {
    func testPaperSizesRepaginateAndRetainEveryLine() throws {
        let source = (1...116).map { "!Passage\($0)" }.joined(separator: "\n")
        for paper in ScreenplayPrintOptions.Paper.allCases {
            var options = ScreenplayPrintOptions(); options.paper = paper
            let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source), options: options)
            let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
            XCTAssertEqual(layout.pages.count, paper == .letter ? 3 : 2)
            let box = try XCTUnwrap(pdf.page(at: 0)).bounds(for: .mediaBox)
            XCTAssertEqual(box.width, paper.size.width, accuracy: 0.01)
            XCTAssertEqual(box.height, paper.size.height, accuracy: 0.01)
            for index in 1...116 { XCTAssertTrue(pdf.string?.contains("Passage\(index)") ?? false) }
            for fragment in layout.pages.flatMap(\.fragments) {
                XCTAssertLessThanOrEqual(fragment.x + fragment.width, paper.size.width - 72 + 0.01)
                XCTAssertLessThanOrEqual(fragment.top, paper.size.height - 72)
            }
        }
    }
    func testHeadingOptionsRespectExplicitEmphasisAndSpacing() throws {
        let source = "!Action.\n\nINT. ROOM - DAY\n\n!Action.\n\n**EXT. ROOF - NIGHT**\n\n!End."
        let output = try ScreenplayOutput.parse(source)
        var options = ScreenplayPrintOptions(); options.boldSceneHeadings = false
        options.underlineSceneHeadings = true; options.doubleSpaceSceneHeadings = true
        let layout = try ScreenplayPagination(output: output, options: options)
        let scene = try XCTUnwrap(layout.pages[0].fragments.first { $0.text.string == "INT. ROOM - DAY" })
        let explicit = try XCTUnwrap(layout.pages[0].fragments.first { $0.text.string == "EXT. ROOF - NIGHT" })
        let font = try XCTUnwrap(scene.text.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)
        XCTAssertFalse(NSFontManager.shared.traits(of: font).contains(.boldFontMask))
        let bold = try XCTUnwrap(explicit.text.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)
        XCTAssertTrue(NSFontManager.shared.traits(of: bold).contains(.boldFontMask))
        XCTAssertEqual(scene.text.attribute(.underlineStyle, at: 0, effectiveRange: nil) as? Int, NSUnderlineStyle.single.rawValue)
        XCTAssertEqual(scene.top, 82 + 3 * ScreenplayPagination.lineHeight)
        let normal = try ScreenplayPagination(output: output)
        XCTAssertEqual(normal.pages[0].fragments.first { $0.text.string == "INT. ROOM - DAY" }?.top, 82 + 2 * ScreenplayPagination.lineHeight)
    }
    func testHiddenTitleAndA4DualDialogueKeepSourceAndColumns() throws {
        let source = "Title: Example\nAuthor: Zoë\n\n@LEFT\nLeft answer.\n\n@RIGHT ^\nRight answer."
        var options = ScreenplayPrintOptions(); options.paper = .a4; options.includeTitlePage = false
        let layout = try ScreenplayPagination(output: ScreenplayOutput.parse(source), options: options)
        XCTAssertEqual(layout.pages.count, 1)
        XCTAssertEqual(layout.pages[0].number, 1)
        let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
        XCTAssertFalse(pdf.string?.contains("Example") ?? true)
        let left = try XCTUnwrap(layout.pages[0].fragments.first { $0.text.string == "LEFT" })
        let right = try XCTUnwrap(layout.pages[0].fragments.first { $0.text.string == "RIGHT" })
        XCTAssertEqual(left.top, right.top)
        for fragment in layout.pages[0].fragments { XCTAssertLessThanOrEqual(fragment.x + fragment.width, options.paper.size.width - 72 + 0.01) }
        var document = ScreenplayDocument(); document.text = source
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
        if let path = ProcessInfo.processInfo.environment["FOUNTAGE_A4_QA_PATH"] {
            options.includeTitlePage = true; options.underlineSceneHeadings = true
            let sample = "Title: THE LAST SIGNAL\nCredit: Written by\nAuthor: Zoë Rivers\n\nINT. OBSERVATORY - NIGHT\n\nThe clock stops.\n\n@LEFT\nA quiet signal.\n\n@RIGHT ^\nAn answer in the dark."
            try ScreenplayPagination(output: ScreenplayOutput.parse(sample), options: options).pdfData().write(to: URL(fileURLWithPath: path), options: .atomic)
        }
    }
    @MainActor func testOptionsPersistAndPrintUsesAnIndependentMatchingPaper() throws {
        let suite = "FountagePrintTests." + UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        defaults.set("unknown", forKey: "screenplayPaper")
        XCTAssertEqual(ScreenplayPrintOptions(defaults: defaults).paper, .letter)
        let model = ScreenplayPreviewModel(source: "Title: Test\n\nINT. ROOM - DAY\n\nAction.", title: "Test", defaults: defaults)
        model.updateOptions { $0.paper = .a4; $0.boldSceneHeadings = false; $0.underlineSceneHeadings = true; $0.doubleSpaceSceneHeadings = true; $0.includeTitlePage = false }
        let reloaded = ScreenplayPreviewModel(source: "Action.", title: "Test", defaults: defaults)
        XCTAssertEqual(model.options, reloaded.options)
        XCTAssertEqual(model.pdf?.pageCount, 1)
        let info = model.printInfo()
        XCTAssertFalse(info === NSPrintInfo.shared)
        XCTAssertEqual(info.paperSize.width, model.options.paper.size.width, accuracy: 0.01)
        XCTAssertEqual(info.paperSize.height, model.options.paper.size.height, accuracy: 0.01)
        XCTAssertEqual(info.leftMargin, 0)
    }
    @MainActor func testNativeTitleMenuUsesTheActiveScreenplayCallback() {
        let editor = ManuscriptTextView()
        let item = NSMenuItem(title: "Title Page…", action: #selector(ManuscriptTextView.editTitlePage(_:)), keyEquivalent: "")
        var count = 0
        editor.onTitlePage = { count += 1 }
        XCTAssertFalse(editor.validateUserInterfaceItem(item))
        editor.editTitlePage(nil); XCTAssertEqual(count, 0)
        editor.supportsFountainFormatting = true
        XCTAssertTrue(editor.validateUserInterfaceItem(item))
        editor.editTitlePage(nil); XCTAssertEqual(count, 1)
        editor.isEditable = false
        XCTAssertFalse(editor.validateUserInterfaceItem(item))
        editor.editTitlePage(nil); XCTAssertEqual(count, 1)
    }
    @MainActor func testTitleFieldsNavigateWithoutInsertingTabs() {
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let first = TitleFieldTextView(frame: NSRect(x: 0, y: 0, width: 200, height: 50))
        let second = TitleFieldTextView(frame: NSRect(x: 0, y: 60, width: 200, height: 50))
        window.contentView?.addSubview(first); window.contentView?.addSubview(second)
        first.nextKeyView = second; second.nextKeyView = first
        first.string = "Title"; window.makeFirstResponder(first)
        first.insertTab(nil)
        XCTAssertTrue(window.firstResponder === second)
        second.insertBacktab(nil)
        XCTAssertTrue(window.firstResponder === first)
        XCTAssertEqual(first.string, "Title")
        XCTAssertEqual(second.string, "")
    }

}
