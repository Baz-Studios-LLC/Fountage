import XCTest
import SwiftUI
@testable import Fountage

@MainActor final class MilestoneWorkflowTests: XCTestCase {
    func testAppKitTextUndoCallbackRetainsHistoryWithoutDocumentSnapshots() throws {
        var document = ScreenplayDocument(); document.text = "Short."
        let cache = ManuscriptAnalysisCache()
        let id = try document.addMilestone(unit: .characters, target: 100,
                                           counts: WritingMilestoneCounts(words: 1, characters: 6, pages: nil))
        let binding = Binding(get: { document.text }, set: { source in
            MilestoneWorkflow.replaceText(source, in: &document, cache: cache)
        })
        let native = NativeEditor(text: binding, fontSize: 16, format: .screenplay, preset: .screenplay, onCursor: { _ in })
        let coordinator = NativeEditor.Coordinator(native)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; defer { window.close() }
        let editor = ManuscriptTextView(frame: window.contentView!.bounds)
        editor.allowsUndo = true; editor.string = document.text; editor.delegate = coordinator
        window.contentView!.addSubview(editor)
        editor.undoManager?.beginUndoGrouping()
        editor.insertText(String(repeating: "a", count: 105), replacementRange: NSRange(location: 0, length: 6))
        editor.undoManager?.endUndoGrouping()
        let completed = try document.writingMilestones()[0]
        XCTAssertEqual(completed.id, id); XCTAssertTrue(completed.completed)
        XCTAssertEqual(document.text, String(repeating: "a", count: 105))
        editor.undoManager?.undo()
        XCTAssertEqual(editor.string, "Short.")
        // The offscreen harness does not host DocumentGroup’s document notifications.
        coordinator.textDidChange(Notification(name: NSText.didChangeNotification, object: editor))
        XCTAssertEqual(document.text, "Short.")
        XCTAssertEqual(try document.writingMilestones()[0], completed)
        editor.undoManager?.redo()
        coordinator.textDidChange(Notification(name: NSText.didChangeNotification, object: editor))
        XCTAssertEqual(document.text, String(repeating: "a", count: 105))
        XCTAssertEqual(try document.writingMilestones()[0], completed)
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(try reopened.writingMilestones(), [completed])
    }
    func testPageCompletionUsesActualOutputAndUnsupportedRecordsDoNotPreventEditing() throws {
        var document = ScreenplayDocument(); document.text = "A signal."
        let counts = WritingMilestoneCounts(words: 2, characters: 9, pages: 1)
        try document.addMilestone(unit: .pages, target: 2, counts: counts)
        let cache = ManuscriptAnalysisCache()
        let source = "Title: Zoë\r\n\r\nA signal.\r\n\r\n===\r\n\r\nStars 🌙."
        MilestoneWorkflow.replaceText(source, in: &document, cache: cache)
        XCTAssertTrue(try document.writingMilestones()[0].completed)
        XCTAssertEqual(Data(document.text.utf8), Data(source.utf8))
        var invalid = ScreenplayDocument()
        try invalid.addMilestone(unit: .characters, target: 10, counts: counts)
        MilestoneWorkflow.replaceText(String(repeating: "a", count: 20) + "\0", in: &invalid, cache: cache)
        XCTAssertFalse(try invalid.writingMilestones()[0].completed)
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        own["goals"] = ["milestones": ["version": 99, "items": []]]
        document.metadata[ScreenplayDocument.namespace] = own
        let before = document.metadata
        MilestoneWorkflow.replaceText("New writing 🌙.", in: &document, cache: cache)
        XCTAssertEqual(document.text, "New writing 🌙.")
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
    }
    func testFormatChangeCompletesUsingNewCategoryAndPreservesSourceAndUnknownData() throws {
        let source = "Title: Zoë\r\n\r\n# Plan\r\n\r\nShort. [[one two three four five six seven eight]]"
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        for format in DocumentFormat.allCases where format != .screenplay {
            var document = ScreenplayDocument(); document.text = source
            let empty = WritingMilestoneCounts(words: 0, characters: 0, pages: nil)
            try document.addMilestone(unit: .words, target: 5, counts: empty)
            try document.addMilestone(unit: .characters, target: source.trimmingCharacters(in: .whitespacesAndNewlines).utf16.count, counts: empty)
            try document.addMilestone(unit: .pages, target: 1, counts: empty)
            var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
            own["foreign"] = ["keep": true]; document.metadata[ScreenplayDocument.namespace] = own
            document.draftWordGoal = 100; document.draftPageGoal = 10
            let cache = ManuscriptAnalysisCache()
            XCTAssertLessThan(cache.analysis(for: source, format: .screenplay).words, 5)
            XCTAssertTrue(try MilestoneWorkflow.changeFormat(format, in: &document, cache: cache, now: date))
            let milestones = try document.writingMilestones()
            XCTAssertEqual(milestones.map(\.completed), [true, true, false])
            XCTAssertEqual(milestones[0].completedAt, date)
            XCTAssertEqual(document.format, format); XCTAssertEqual(document.preset, format.presets[0])
            let wrapper = try document.makeWrapper(contentType: .fountage)
            let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
            let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
            XCTAssertEqual(reopened.format, format)
            XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
            XCTAssertEqual(try reopened.writingMilestones(), milestones)
            XCTAssertEqual(reopened.draftWordGoal, 100); XCTAssertEqual(reopened.draftPageGoal, 10)
            XCTAssertNotNil((reopened.metadata[ScreenplayDocument.namespace] as? [String: Any])?["foreign"])
            XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
        }
    }
    func testEnteringScreenplayCompletesPageTargetAndSameFormatDoesNotResetLayoutOrParse() throws {
        var document = ScreenplayDocument(); document.format = .comic; document.preset = .comicLettering
        document.text = "A signal.\r\n\r\n===\r\n\r\nStars 🌙."
        try document.addMilestone(unit: .pages, target: 2, counts: WritingMilestoneCounts(words: 0, characters: 0, pages: nil))
        let before = document.metadata
        var calls = 0
        let cache = ManuscriptAnalysisCache { text, format in calls += 1; return ManuscriptAnalysis.parse(text, format: format) }
        XCTAssertFalse(try MilestoneWorkflow.changeFormat(.comic, in: &document, cache: cache))
        XCTAssertEqual(document.preset, .comicLettering)
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata)); XCTAssertEqual(calls, 0)
        XCTAssertTrue(try MilestoneWorkflow.changeFormat(.screenplay, in: &document, cache: cache))
        let completed = try document.writingMilestones()[0]
        XCTAssertTrue(completed.completed)
        XCTAssertTrue(try MilestoneWorkflow.changeFormat(.general, in: &document, cache: cache))
        XCTAssertEqual(try document.writingMilestones()[0], completed, "Leaving a category must not erase previously reached history")
    }
    func testFormatChangeKeepsUnsupportedHistoryAndRejectsUnsupportedNamespaceAtomically() throws {
        var document = ScreenplayDocument(); document.text = "Original 🌙."
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        own["goals"] = ["milestones": ["version": 99, "private": [1, 2, 3]]]
        document.metadata[ScreenplayDocument.namespace] = own
        XCTAssertTrue(try MilestoneWorkflow.changeFormat(.general, in: &document, cache: .init()))
        let retained = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["goals"] as! [String: Any]
        XCTAssertEqual((retained["milestones"] as? [String: Any])?["version"] as? Int, 99)
        document.metadata[ScreenplayDocument.namespace] = "future namespace"
        let before = document.metadata
        XCTAssertThrowsError(try MilestoneWorkflow.changeFormat(.general, in: &document, cache: .init()))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
        XCTAssertEqual(document.text, "Original 🌙.")
    }

    func testStaleMilestoneEditAndRemovalRejectWithoutChangingNewerRecord() throws {
        var document = ScreenplayDocument()
        let counts = WritingMilestoneCounts(words: 0, characters: 0, pages: nil)
        let id = try document.addMilestone(unit: .words, target: 10, counts: counts)
        let stale = try document.writingMilestones()[0]
        try document.editMilestone(id: id, unit: .words, target: 20, counts: counts)
        let before = document.metadata
        XCTAssertThrowsError(try document.editMilestone(id: id, unit: .words, target: 30, counts: counts, expected: stale))
        XCTAssertThrowsError(try document.removeMilestone(id: id, expected: stale))
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata))
    }
}
