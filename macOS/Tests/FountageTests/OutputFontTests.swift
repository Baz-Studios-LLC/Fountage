import XCTest
import AppKit
import PDFKit
@testable import Fountage

final class OutputFontTests: XCTestCase {
    func testIndependentFontRolesEmphasisGeneratedCuesAndWrapping() throws {
        let source = "INT. ROOM - DAY #1#\n\n!A **bold** and *italic* sentence.\n\nMARA\n" + String(repeating: "The signal returns. ", count: 250)
        var options = ScreenplayPrintOptions(); options.headingFontFamily = "Helvetica"; options.bodyFontFamily = "Times New Roman"
        let output = try ScreenplayOutput.parse(source)
        let layout = try ScreenplayPagination(output: output, options: options)
        XCTAssertGreaterThan(layout.pages.count, 1)
        let fragments = layout.pages.flatMap(\.fragments)
        for fragment in fragments where fragment.text.length > 0 {
            let font = try XCTUnwrap(fragment.text.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)
            let heading = fragment.text.string.hasPrefix("INT.") || fragment.text.string == "1"
            XCTAssertEqual(font.familyName, heading ? "Helvetica" : "Times New Roman")
            XCTAssertLessThanOrEqual(fragment.x + fragment.width, options.paper.size.width)
        }
        XCTAssertTrue(fragments.contains { $0.generated && $0.text.string == "(MORE)" })
        let action = try XCTUnwrap(fragments.first { $0.text.string.contains("bold") })
        let range = (action.text.string as NSString).range(of: "bold")
        let bold = try XCTUnwrap(action.text.attribute(.font, at: range.location, effectiveRange: nil) as? NSFont)
        XCTAssertTrue(NSFontManager.shared.traits(of: bold).contains(.boldFontMask))
        let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
        XCTAssertTrue(pdf.string?.contains("A bold and italic sentence.") ?? false)
        var courier = options; courier.bodyFontFamily = "Courier"
        let original = try ScreenplayPagination(output: output, options: courier)
        XCTAssertNotEqual(original.pages.flatMap(\.fragments).count, fragments.count)
        if let path = ProcessInfo.processInfo.environment["FOUNTAGE_FONT_QA_PATH"] {
            try layout.pdfData().write(to: URL(fileURLWithPath: path), options: .atomic)
        }
    }
    @MainActor func testNativeFontMenusUpdatePDFCheckmarksAndPersist() throws {
        let name = "FountageFontTests-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name)); defer { defaults.removePersistentDomain(forName: name) }
        let source = "INT. ROOM - DAY\n\n!A signal returns."
        let model = ScreenplayPreviewModel(source: source, title: "QA", defaults: defaults); model.render()
        let coordinator = ScreenplayOutputMenu.Coordinator(model: model)
        let menu = coordinator.makeMenu()
        for (title, family) in [("Heading Font", "Helvetica"), ("Body Font", "Times New Roman")] {
            let fonts = try XCTUnwrap(menu.item(withTitle: title)?.submenu)
            let index = fonts.indexOfItem(withTitle: family)
            guard index >= 0 else { return XCTFail("Installed font is absent from menu: \(family)") }
            fonts.performActionForItem(at: index)
            XCTAssertEqual(fonts.item(at: index)?.state, .on)
            XCTAssertEqual(fonts.item(withTitle: "Courier")?.state, .off)
            XCTAssertNotNil(model.pdf); XCTAssertNil(model.error)
        }
        let loaded = ScreenplayPrintOptions(defaults: defaults)
        XCTAssertEqual(loaded.headingFontFamily, "Helvetica"); XCTAssertEqual(loaded.bodyFontFamily, "Times New Roman")
        let reopened = ScreenplayPreviewModel(source: source, title: "QA", defaults: defaults); reopened.render()
        XCTAssertEqual(reopened.options, model.options)
        XCTAssertEqual(reopened.pdf?.string, model.pdf?.string)
        defaults.set("Missing Font 123", forKey: "screenplayBodyFontFamily")
        XCTAssertEqual(ScreenplayPrintOptions(defaults: defaults).bodyFontFamily, "Courier")
        XCTAssertEqual(ScreenplayPagination.font(family: "Missing Font 123").familyName, "Courier")
        var document = ScreenplayDocument(); document.text = source
        let wrapper = try document.makeWrapper(contentType: .fountage)
        XCTAssertEqual(try ScreenplayDocument(wrapper: wrapper, contentType: .fountage).text, source)
    }
}
