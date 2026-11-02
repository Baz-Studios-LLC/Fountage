import XCTest
import UniformTypeIdentifiers
@testable import Fountage

final class DraftGoalTests: XCTestCase {
    func testGoalSurvivesEveryWritingFormatAndDiskReopen() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument()
            document.format = format
            document.text = "Zoë writes 🌙 and 世界.\r\n"
            document.draftWordGoal = 50_000
            let url = directory.appendingPathComponent("\(format.rawValue).fountage")
            try document.makeWrapper(contentType: .fountage).write(to: url, options: .atomic, originalContentsURL: nil)
            let reopened = try ScreenplayDocument(wrapper: FileWrapper(url: url), contentType: .fountage)
            XCTAssertEqual(reopened.draftWordGoal, 50_000)
            XCTAssertEqual(reopened.text, document.text)
            XCTAssertEqual(reopened.format, format)
            XCTAssertEqual(try reopened.makeWrapper(contentType: .plainText).regularFileContents, Data(document.text.utf8))
        }
    }

    func testCharacterAndWordGoalsRemainIndependentAcrossAllFormatsAndAssets() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format
            document.text = "Zoë 🌙 e\u{301}\r\n"
            var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
            own["goals"] = ["future": ["keep": true]]
            document.metadata[ScreenplayDocument.namespace] = own
            document.draftWordGoal = 100; document.draftCharacterGoal = 500
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255, 1])); asset.preferredFilename = "private.bin"
            wrapper.addFileWrapper(asset)
            var reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(reopened.draftWordGoal, 100)
            XCTAssertEqual(reopened.draftCharacterGoal, 500)
            reopened.draftCharacterGoal = nil
            XCTAssertEqual(reopened.draftWordGoal, 100)
            XCTAssertNil(reopened.draftCharacterGoal)
            let saved = try reopened.makeWrapper(contentType: .fountage)
            XCTAssertEqual(saved.fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255, 1]))
            XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
            let goals = (reopened.metadata[ScreenplayDocument.namespace] as! [String: Any])["goals"] as! [String: Any]
            XCTAssertEqual((goals["future"] as? [String: Bool])?["keep"], true)
        }
    }
    func testCharacterGoalRejectsMalformedAndUnsupportedRecordsWithoutErasingThem() throws {
        for invalid: Any in [true, 1.5, 0, -1, 10_000_001, "500", ["future": 1]] {
            var document = ScreenplayDocument()
            var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
            own["goals"] = ["draftCharacters": invalid]; document.metadata[ScreenplayDocument.namespace] = own
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertNil(reopened.draftCharacterGoal)
            XCTAssertNotNil(((reopened.metadata[ScreenplayDocument.namespace] as? [String: Any])?["goals"] as? [String: Any])?["draftCharacters"])
        }
        var unsupported = ScreenplayDocument()
        var own = unsupported.metadata[ScreenplayDocument.namespace] as! [String: Any]
        own["goals"] = "future record"; unsupported.metadata[ScreenplayDocument.namespace] = own
        unsupported.draftCharacterGoal = 100; unsupported.draftWordGoal = 200
        XCTAssertEqual((unsupported.metadata[ScreenplayDocument.namespace] as? [String: Any])?["goals"] as? String, "future record")
    }
    func testRustCharacterCountBridgeMatchesHighlandAndRawOtherCategories() {
        XCTAssertEqual(ManuscriptAnalysis.parse("A b 🌙\n\n[[private]]", format: .screenplay).character_count, 6)
        XCTAssertEqual(ManuscriptAnalysis.parse("**bold**\n\nPlain [[private]] end\n\n= Summary\n\n/*hidden*/", format: .screenplay).character_count, 20)
        for format in DocumentFormat.allCases where format != .screenplay {
            XCTAssertEqual(ManuscriptAnalysis.parse("  Zoë 🌙 e\u{301}\r\n", format: format).character_count, 9)
        }
    }

    func testChangingAndRemovingGoalPreservesOtherGoalMetadata() throws {
        var document = ScreenplayDocument()
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        own["goals"] = ["daily": ["futureSetting": true], "draftWords": 250] as [String: Any]
        document.metadata[ScreenplayDocument.namespace] = own
        document.draftWordGoal = 500
        document.format = .comic
        var reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(reopened.draftWordGoal, 500)
        reopened.draftWordGoal = nil
        let cleared = try ScreenplayDocument(wrapper: reopened.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertNil(cleared.draftWordGoal)
        let clearedOwn = cleared.metadata[ScreenplayDocument.namespace] as! [String: Any]
        let goals = clearedOwn["goals"] as! [String: Any]
        XCTAssertEqual((goals["daily"] as? [String: Bool])?["futureSetting"], true)
        XCTAssertNil(goals["draftWords"])
    }

    func testMalformedGoalIsIgnoredWithoutLosingItsStoredValue() throws {
        for invalid: Any in [true, 1.5, -1, 0, 10_000_001, "500", ["future": 1]] {
            var document = ScreenplayDocument()
            var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
            own["goals"] = ["draftWords": invalid]
            document.metadata[ScreenplayDocument.namespace] = own
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertNil(reopened.draftWordGoal)
            let goals = (reopened.metadata[ScreenplayDocument.namespace] as! [String: Any])["goals"] as! [String: Any]
            XCTAssertNotNil(goals["draftWords"])
        }
    }

    func testGoalInputAndCompletionRecoverAfterDeletingWords() {
        for invalid in ["", "0", "-5", "1.5", "one hundred", "999999999999999999999", "10000001"] {
            XCTAssertNil(DraftGoal.parseTarget(invalid))
        }
        XCTAssertEqual(DraftGoal.parseTarget(" 50000\n"), 50_000)
        XCTAssertEqual(DraftGoal.parseTarget("1"), 1)
        XCTAssertEqual(DraftGoal.parseTarget("10000000"), 10_000_000)
        let complete = DraftGoal(target: 500, words: 525)
        XCTAssertTrue(complete.reached)
        XCTAssertEqual(complete.remaining, 0)
        XCTAssertEqual(complete.progress, 1)
        let revised = DraftGoal(target: 500, words: 250)
        XCTAssertFalse(revised.reached)
        XCTAssertEqual(revised.remaining, 250)
        XCTAssertEqual(revised.progress, 0.5)
    }
}
