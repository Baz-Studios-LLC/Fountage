import XCTest
import PDFKit
@testable import Fountage

final class ScreenplayPreviewModelTests: XCTestCase {
    @MainActor func testOptionChangeImmediatelyReplacesExportAndPersistsChoice() throws {
        let suite = "FountagePreviewTests." + UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let source = "@MARA\n" + (1...110).map { "Line \($0): the signal continues." }.joined(separator: "\n")
        let model = ScreenplayPreviewModel(source: source, title: "QA", defaults: defaults)
        model.render()
        for enabled in [false, true, false, true] {
            model.setContinuations(enabled)
            let exported = try XCTUnwrap(PDFDocument(data: XCTUnwrap(model.data)))
            XCTAssertEqual(exported.string?.contains("(MORE)"), enabled)
            XCTAssertEqual(exported.string, model.pdf?.string)
            XCTAssertEqual(model.continuations, enabled)
            XCTAssertEqual(defaults.bool(forKey: ScreenplayPreviewModel.continuationKey), enabled)
        }
        model.setContinuations(false)
        let reopened = ScreenplayPreviewModel(source: source, title: "QA", defaults: defaults)
        reopened.render()
        XCTAssertFalse(reopened.continuations)
        XCTAssertFalse(reopened.pdf?.string?.contains("(MORE)") ?? true)
    }
    @MainActor func testInvalidSourceNeverProvidesAnExportSnapshot() throws {
        let model = ScreenplayPreviewModel(source: "Before\0After", title: "QA")
        model.render()
        XCTAssertNil(model.data)
        XCTAssertNil(model.pdf)
        XCTAssertNotNil(model.error)
    }
}
