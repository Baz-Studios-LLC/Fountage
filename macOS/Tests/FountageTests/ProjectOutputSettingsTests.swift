import XCTest
import SwiftUI
import PDFKit
@testable import Fountage

final class ProjectOutputSettingsTests: XCTestCase {
    func testProjectRoundTripRetainsIndependentOptionsSourceAndUnknownData() throws {
        var first = ScreenplayDocument(), second = ScreenplayDocument()
        first.text = "Title: Zoë\r\n\r\nINT. CAFÉ - DAY\r\n\r\nA signal 🌙."
        var info = first.metadata, own = info[ScreenplayDocument.namespace] as! [String: Any]
        own["futurePrivate"] = ["keep": [1, 2, 3]]
        info[ScreenplayDocument.namespace] = own; info["foreign"] = ["retain": true]; first.metadata = info
        var custom = ScreenplayPrintOptions(); custom.paper = .a4; custom.bodyFontFamily = "Helvetica"
        custom.includeSynopses = true; custom.visibleSectionLevels = [1, 3, 6]; custom.dialogueContinuations = false
        try first.setOutputOptions(custom, previous: .init())
        var different = ScreenplayPrintOptions(); different.includeTitlePage = false
        try second.setOutputOptions(different, previous: .init())
        let wrapper = try first.makeWrapper(contentType: .fountage)
        let attachment = FileWrapper(regularFileWithContents: Data([0, 255, 1])); attachment.preferredFilename = "private.bin"; wrapper.addFileWrapper(attachment)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(reopened.outputOptions(), custom)
        XCTAssertEqual(second.outputOptions(), different)
        XCTAssertEqual(Data(reopened.text.utf8), Data(first.text.utf8))
        XCTAssertEqual((reopened.metadata["foreign"] as? [String: Bool])?["retain"], true)
        XCTAssertNotNil((reopened.metadata[ScreenplayDocument.namespace] as? [String: Any])?["futurePrivate"])
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255, 1]))
    }
    func testProjectsWithoutSettingsUseStableDefaultsWithoutWritingMetadata() {
        let document = ScreenplayDocument()
        XCTAssertNil(document.outputSettingsRecord)
        XCTAssertEqual(document.outputOptions(), ScreenplayPrintOptions())
        XCTAssertNil(document.outputSettingsRecord)
    }
    func testDeltaUpdatesKeepUnknownAndMalformedUneditedFields() throws {
        var document = ScreenplayDocument()
        var info = document.metadata, own = info[ScreenplayDocument.namespace] as! [String: Any]
        own["screenplayOutput"] = ["version": 1, "paper": "future-paper", "includeTitlePage": "future-policy", "futureOption": ["keep": 9], "bodyFontFamily": "Unavailable Family"]
        info[ScreenplayDocument.namespace] = own; document.metadata = info
        let previous = document.outputOptions()
        var changed = previous; changed.includeSynopses = true
        try document.setOutputOptions(changed, previous: previous)
        let record = try XCTUnwrap(document.outputSettingsRecord)
        let fields = try XCTUnwrap(JSONSerialization.jsonObject(with: record) as? [String: Any])
        XCTAssertEqual(fields["paper"] as? String, "future-paper")
        XCTAssertEqual(fields["includeTitlePage"] as? String, "future-policy")
        XCTAssertEqual(fields["bodyFontFamily"] as? String, "Unavailable Family")
        XCTAssertEqual((fields["futureOption"] as? [String: Int])?["keep"], 9)
        XCTAssertEqual(fields["includeSynopses"] as? Bool, true)
    }
    @MainActor func testMetadataUndoRedoNeverReplacesLaterManuscriptEdits() throws {
        var document = ScreenplayDocument(); document.text = "Original."
        let binding = Binding(get: { document }, set: { document = $0 })
        let manager = UndoManager(); manager.groupsByEvent = false
        let controller = ProjectOutputSettings()
        let original = document.outputSettingsRecord
        var custom = ScreenplayPrintOptions(); custom.paper = .a4
        manager.beginUndoGrouping(); try controller.change(custom, previous: .init(), document: binding, undoManager: manager); manager.endUndoGrouping()
        XCTAssertEqual(manager.undoActionName, "Change Output Settings")
        document.text += " More writing 🌙."
        manager.undo(); XCTAssertEqual(document.outputSettingsRecord, original)
        XCTAssertEqual(document.text, "Original. More writing 🌙.")
        manager.redo(); XCTAssertEqual(document.outputOptions().paper, .a4)
        XCTAssertEqual(document.text, "Original. More writing 🌙.")
        // A newer record from another owner must not be overwritten by a stale undo.
        var other = custom; other.includeSynopses = true
        try document.setOutputOptions(other, previous: custom)
        manager.undo(); XCTAssertEqual(document.outputOptions(), other)
    }
    @MainActor func testConsecutiveSettingsUndoRedoBranchAndNoOpHistory() throws {
        var document = ScreenplayDocument()
        document.text = "INT. CAFÉ - NIGHT\r\n\r\nZoë 🌙."
        let source = document.text
        let binding = Binding(get: { document }, set: { document = $0 })
        let manager = UndoManager(); manager.groupsByEvent = false
        let controller = ProjectOutputSettings()
        func change(_ options: ScreenplayPrintOptions) throws {
            manager.beginUndoGrouping()
            defer { manager.endUndoGrouping() }
            try controller.change(options, previous: document.outputOptions(), document: binding, undoManager: manager)
        }
        var a4 = ScreenplayPrintOptions(); a4.paper = .a4
        try change(a4)
        let firstRecord = document.outputSettingsRecord
        var synopsis = a4; synopsis.includeSynopses = true
        try change(synopsis)
        manager.undo()
        XCTAssertEqual(document.outputSettingsRecord, firstRecord)
        manager.undo()
        XCTAssertNil(document.outputSettingsRecord)
        XCTAssertFalse(manager.canUndo, "Each settings change must consume exactly one history step")
        manager.redo(); manager.redo()
        XCTAssertEqual(document.outputOptions(), synopsis)
        XCTAssertFalse(manager.canRedo)
        manager.undo()
        // A no-op must neither add history nor discard the available redo branch.
        try controller.change(a4, previous: document.outputOptions(), document: binding, undoManager: manager)
        XCTAssertTrue(manager.canRedo)
        var branch = a4; branch.includeTitlePage = false
        try change(branch)
        XCTAssertFalse(manager.canRedo)
        manager.undo(); XCTAssertEqual(document.outputSettingsRecord, firstRecord)
        manager.redo(); XCTAssertEqual(document.outputOptions(), branch)
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(reopened.outputOptions(), branch)
        XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
    }
    @MainActor func testPreviewKeepsProjectsSeparateAndReflectsUndoOptionsWithoutSavingAgain() throws {
        let name = "OutputProjectTests-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name)); defer { defaults.removePersistentDomain(forName: name) }
        var document = ScreenplayDocument(); document.text = "INT. ROOM - DAY\n\nA signal."
        let original = ScreenplayPrintOptions(defaults: defaults)
        let model = ScreenplayPreviewModel(source: document.text, title: "QA", defaults: defaults, options: original) { changed, before in
            try document.setOutputOptions(changed, previous: before)
        }
        model.render(); model.updateOptions { $0.paper = .a4; $0.includeTitlePage = false }
        XCTAssertEqual(document.outputOptions().paper, .a4)
        XCTAssertEqual(ScreenplayPrintOptions(defaults: defaults), original)
        let other = ScreenplayPreviewModel(source: "Other.", title: "Other", defaults: defaults)
        XCTAssertEqual(other.options, original)
        let after = document.outputSettingsRecord
        model.receiveOptions(original)
        XCTAssertEqual(document.outputSettingsRecord, after)
        XCTAssertEqual(model.pdf?.page(at: 0)?.bounds(for: .mediaBox).width, 612)
    }
    @MainActor func testUnsupportedRecordRejectsChangesAtomicallyAndKeepsExistingPreview() throws {
        for value: Any in ["foreign", ["version": 2], ["version": true]] {
            var document = ScreenplayDocument()
            var info = document.metadata, own = info[ScreenplayDocument.namespace] as! [String: Any]
            own["screenplayOutput"] = value; info[ScreenplayDocument.namespace] = own; document.metadata = info
            let before = document.outputSettingsRecord
            let model = ScreenplayPreviewModel(source: "Action.", title: "QA", options: .init()) { changed, previous in
                try document.setOutputOptions(changed, previous: previous)
            }
            model.render(); let pdf = model.pdf
            model.updateOptions { $0.paper = .a4 }
            XCTAssertEqual(document.outputSettingsRecord, before)
            XCTAssertEqual(model.options.paper, .letter)
            XCTAssertTrue(model.pdf === pdf)
            XCTAssertNotNil(model.error)
        }
    }
}
