import XCTest
import UniformTypeIdentifiers
@testable import Fountage

final class SaveValidationTests: XCTestCase {
    func testInvalidPasteCannotReplaceSavedProjectForAnyFormat() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument()
            document.format = format
            document.text = "Café 🎬\r\nOriginal manuscript.\r\n"
            document.retainedEntries["assets"] = .directory(["keep.bin": .file(Data([0, 255, 1]))])
            let url = root.appendingPathComponent("\(format.rawValue).fountage")
            try document.makeWrapper(contentType: .fountage).write(to: url, options: .atomic, originalContentsURL: nil)
            let source = document.text
            document.text += "Pasted\0binary content"
            XCTAssertThrowsError(try document.makeWrapper(contentType: .fountage).write(to: url, options: .atomic, originalContentsURL: url)) { error in
                XCTAssertTrue(error is ProjectError)
                XCTAssertNotNil((error as? LocalizedError)?.recoverySuggestion)
            }
            let reopened = try ScreenplayDocument(wrapper: FileWrapper(url: url), contentType: .fountage)
            XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
            XCTAssertEqual(reopened.retainedEntries["assets"]?.fileWrapper().fileWrappers?["keep.bin"]?.regularFileContents, Data([0, 255, 1]))
            // Recovering the edit makes saving possible again without losing content.
            document.text = source + "Revised ending."
            try document.makeWrapper(contentType: .fountage).write(to: url, options: .atomic, originalContentsURL: url)
            XCTAssertEqual(try ScreenplayDocument(wrapper: FileWrapper(url: url), contentType: .fountage).text, document.text)
        }
    }

    func testPlainTextAndFountainRejectNullWithoutStrippingOtherUnicode() throws {
        for type in [UTType.plainText, .fountain] {
            var document = ScreenplayDocument()
            document.text = "Before\0after"
            XCTAssertThrowsError(try document.makeWrapper(contentType: type))
            document.text = "Cafe\u{301}\t世界\r\n🎬\n"
            let wrapper = try document.makeWrapper(contentType: type)
            XCTAssertEqual(wrapper.regularFileContents, Data(document.text.utf8))
            XCTAssertEqual(Data(try ScreenplayDocument(wrapper: wrapper, contentType: type).text.utf8), Data(document.text.utf8))
        }
    }
}
