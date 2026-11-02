import XCTest
@testable import Fountage

final class WritingMilestoneTests: XCTestCase {
    private let early = Date(timeIntervalSince1970: 1_700_000_000)
    private let later = Date(timeIntervalSince1970: 1_700_100_000)
    private func counts(_ words: Int = 0, _ characters: Int = 0, _ pages: Int? = nil) -> WritingMilestoneCounts {
        WritingMilestoneCounts(words: words, characters: characters, pages: pages)
    }
    func testMultipleMilestonesCompleteIndependentlyAndFreezeAfterDeletion() throws {
        var document = ScreenplayDocument()
        let first = try document.addMilestone(unit: .words, target: 10, counts: counts(), now: early)
        let second = try document.addMilestone(unit: .words, target: 20, counts: counts(), now: early)
        let character = try document.addMilestone(unit: .characters, target: 100, counts: counts(), now: early)
        let pages = try document.addMilestone(unit: .pages, target: 2, counts: counts(), now: early)
        try document.advanceMilestones(counts: counts(15, 105), now: early)
        var milestones = try document.writingMilestones()
        XCTAssertEqual(milestones.map(\.id), [first, second, character, pages])
        XCTAssertEqual(milestones.map(\.completed), [true, false, true, false])
        XCTAssertEqual(milestones[2].completedAt, early)
        try document.advanceMilestones(counts: counts(0, 6, 2), now: later)
        milestones = try document.writingMilestones()
        XCTAssertEqual(milestones.map(\.completed), [true, false, true, true])
        XCTAssertEqual(milestones[0].completedAt, early)
        XCTAssertEqual(milestones[3].completedAt, later)
        let fields = try rawItems(document)
        XCTAssertEqual(fields[2]["completedCount"] as? Int, 100, "Overshoot is capped at the target, like Highland")
    }
    func testEditingNoOpKeepsHistoryAndChangedTargetStartsNewCompletion() throws {
        var document = ScreenplayDocument()
        let id = try document.addMilestone(unit: .characters, target: 100, counts: counts(0, 105), now: early)
        let before = document.metadata
        try document.editMilestone(id: id, unit: .characters, target: 100, counts: counts(), now: later)
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        try document.editMilestone(id: id, unit: .characters, target: 200, counts: counts(0, 105), now: later)
        XCTAssertFalse(try document.writingMilestones()[0].completed)
        try document.advanceMilestones(counts: counts(0, 250), now: later)
        XCTAssertEqual(try document.writingMilestones()[0].completedAt, later)
        try document.removeMilestone(id: id)
        XCTAssertTrue(try document.writingMilestones().isEmpty)
        XCTAssertThrowsError(try document.removeMilestone(id: id))
    }
    func testPreservesLegacyTargetsUnknownFieldsFutureUnitsAndAssetsThroughReopen() throws {
        for format in DocumentFormat.allCases {
            var document = ScreenplayDocument(); document.format = format
            document.text = "Zoë 🌙 e\u{301}\r\n"
            document.draftWordGoal = 1000; document.draftCharacterGoal = 5000; document.draftPageGoal = 10
            var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
            var goals = own["goals"] as! [String: Any]
            goals["futureGoal"] = ["private": true]
            goals["milestones"] = ["version": 1, "futureSetting": ["keep": true], "items": [["id": "future-id", "unit": "future-unit", "payload": [1, 2, 3]]]] as [String: Any]
            own["goals"] = goals; document.metadata[ScreenplayDocument.namespace] = own
            let id = try document.addMilestone(unit: .characters, target: 100, counts: counts(0, 105), now: early)
            var items = try rawItems(document)
            items[1]["foreign"] = ["keep": true]
            setItems(items, in: &document)
            try document.advanceMilestones(counts: counts(), now: later)
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            var reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(try reopened.writingMilestones().map(\.id), [id])
            XCTAssertEqual(reopened.draftWordGoal, 1000); XCTAssertEqual(reopened.draftCharacterGoal, 5000); XCTAssertEqual(reopened.draftPageGoal, 10)
            XCTAssertEqual(Data(reopened.text.utf8), Data(document.text.utf8))
            try reopened.editMilestone(id: id, unit: .words, target: 200, counts: counts(), now: later)
            let retained = try rawItems(reopened)
            XCTAssertEqual(retained[0]["unit"] as? String, "future-unit")
            XCTAssertNotNil(retained[1]["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
            try reopened.removeMilestone(id: id)
            XCTAssertEqual(try rawItems(reopened).count, 1)
        }
    }
    func testUnsupportedRecordsRejectEveryMutationAtomically() throws {
        let invalidRecords: [Any] = ["future", ["version": 2, "items": []], ["version": true, "items": []],
            ["version": 1, "items": [["id": "a", "unit": "words", "target": true]]],
            ["version": 1, "items": [["id": "a", "unit": "words", "target": 10, "completedAt": "invalid", "completedCount": 10]]],
            ["version": 1, "items": [["id": "a", "unit": "words", "target": 10], ["id": "a", "unit": "words", "target": 20]]]]
        for invalid in invalidRecords {
            var document = ScreenplayDocument()
            var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
            own["goals"] = ["milestones": invalid]; document.metadata[ScreenplayDocument.namespace] = own
            let before = document.metadata
            XCTAssertThrowsError(try document.writingMilestones())
            XCTAssertThrowsError(try document.addMilestone(unit: .words, target: 10, counts: counts()))
            XCTAssertThrowsError(try document.editMilestone(id: "a", unit: .words, target: 20, counts: counts()))
            XCTAssertThrowsError(try document.removeMilestone(id: "a"))
            XCTAssertThrowsError(try document.advanceMilestones(counts: counts(100, 100, 100)))
            XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        }
    }
    func testUnsupportedWholeGoalsAndNamespaceStayIntact() throws {
        for unsupportedNamespace in [false, true] {
            var document = ScreenplayDocument()
            if unsupportedNamespace { document.metadata[ScreenplayDocument.namespace] = "future namespace" }
            else {
                var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
                own["goals"] = "future goals"; document.metadata[ScreenplayDocument.namespace] = own
            }
            let before = document.metadata
            XCTAssertThrowsError(try document.addMilestone(unit: .words, target: 10, counts: counts()))
            XCTAssertThrowsError(try document.advanceMilestones(counts: counts(100, 100, 100)))
            XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        }
    }
    func testCompletedHistorySurvivesAtomicDiskSaveAndReopen() throws {
        var document = ScreenplayDocument(); document.text = "Short."
        let id = try document.addMilestone(unit: .characters, target: 100, counts: counts(0, 105), now: early)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("Milestones.fountage")
        try document.makeWrapper(contentType: .fountage).write(to: url, options: .atomic, originalContentsURL: nil)
        var reopened = try ScreenplayDocument(wrapper: FileWrapper(url: url), contentType: .fountage)
        try reopened.advanceMilestones(counts: counts(1, 6), now: later)
        let history = try reopened.writingMilestones()
        XCTAssertEqual(history.map(\.id), [id]); XCTAssertEqual(history[0].completedAt, early)
        XCTAssertEqual(reopened.text, "Short.")
    }
    func testNoReadOrUnreachedAdvanceCreatesMetadataAndInvalidTargetIsAtomic() throws {
        var document = ScreenplayDocument()
        let before = document.metadata
        XCTAssertTrue(try document.writingMilestones().isEmpty)
        try document.advanceMilestones(counts: counts(100, 100, 100))
        XCTAssertThrowsError(try document.addMilestone(unit: .words, target: 0, counts: counts()))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        let id = try document.addMilestone(unit: .words, target: 100, counts: counts())
        let stored = document.metadata
        try document.advanceMilestones(counts: counts(99, 1000, 100))
        XCTAssertTrue(NSDictionary(dictionary: stored).isEqual(to: document.metadata))
        XCTAssertThrowsError(try document.editMilestone(id: id, unit: .words, target: -1, counts: counts()))
        XCTAssertTrue(NSDictionary(dictionary: stored).isEqual(to: document.metadata))
    }
    private func rawItems(_ document: ScreenplayDocument) throws -> [[String: Any]] {
        let own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        let goals = own["goals"] as! [String: Any]
        return (goals["milestones"] as! [String: Any])["items"] as! [[String: Any]]
    }
    private func setItems(_ items: [[String: Any]], in document: inout ScreenplayDocument) {
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        var goals = own["goals"] as! [String: Any]
        var record = goals["milestones"] as! [String: Any]
        record["items"] = items; goals["milestones"] = record; own["goals"] = goals; document.metadata[ScreenplayDocument.namespace] = own
    }
}
