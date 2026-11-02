import XCTest
import AppKit
import PDFKit
@testable import Fountage

final class TodoOutputTests: XCTestCase {
    let source = "INT. CAFÉ - DAY\r\n\r\n[ ] Open **Zoë** 🌙 [[private]]\r\n[x] Completed _task_\r\n[X] Another done\r\n[ ]\r\n\r\nAn ordinary action.\r\n\r\n/*\r\n[x] Omitted task\r\n*/"
    @MainActor func testCheckboxStyleVisibilityAndPDFUseSameSemanticState() throws {
        let output = try ScreenplayOutput.parse(source)
        let todos = output.body.filter { $0.kind == .todo }
        XCTAssertEqual(todos.map(\.todo_completed), [false, true, true, false])
        XCTAssertEqual(todos.map(\.text), ["☐ Open Zoë 🌙", "✓ Completed task", "✓ Another done", "☐ "])
        for paper in ScreenplayPrintOptions.Paper.allCases {
            for (include, completed, expected) in [(true, true, 4), (true, false, 2), (false, true, 0), (false, false, 0)] {
                var options = ScreenplayPrintOptions(); options.paper = paper; options.includeTodos = include; options.includeCompletedTodos = completed
                let layout = try ScreenplayPagination(output: output, options: options)
                let fragments = layout.pages.flatMap(\.fragments)
                let taskFragments = fragments.filter { [2, 3, 4, 5].contains($0.sourceLine) }
                XCTAssertEqual(taskFragments.count, expected)
                if include && completed {
                    let done = try XCTUnwrap(taskFragments.first { $0.sourceLine == 3 })
                    XCTAssertEqual(done.text.attribute(.strikethroughStyle, at: 2, effectiveRange: nil) as? Int, NSUnderlineStyle.single.rawValue)
                }
                let pdf = try XCTUnwrap(PDFDocument(data: layout.pdfData()))
                let text = try XCTUnwrap(pdf.string)
                XCTAssertEqual(text.contains("Completed task"), include && completed)
                XCTAssertEqual(text.contains("Open Zoë"), include)
                XCTAssertTrue(text.contains("ordinary action"))
                XCTAssertFalse(text.contains("private")); XCTAssertFalse(text.contains("Omitted task"))
                XCTAssertEqual(pdf.pageCount, layout.pages.count)
            }
        }
    }
    @MainActor func testNativeMenuCheckboxesCommitIndependentFlagsAndDefaultPersistence() throws {
        let suite = "TodoOutput." + UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite)); defer { defaults.removePersistentDomain(forName: suite) }
        let model = ScreenplayPreviewModel(source: source, title: "QA", defaults: defaults); model.render()
        let coordinator = ScreenplayOutputMenu.Coordinator(model: model); let menu = coordinator.makeMenu()
        let all = try XCTUnwrap(menu.item(withTag: ScreenplayOutputMenu.Coordinator.Command.todos.rawValue))
        let done = try XCTUnwrap(menu.item(withTag: ScreenplayOutputMenu.Coordinator.Command.completedTodos.rawValue))
        coordinator.choose(done)
        XCTAssertEqual(done.state, .off); XCTAssertTrue(model.options.includeTodos)
        XCTAssertFalse(model.pdf?.string?.contains("Completed task") ?? true)
        coordinator.choose(all)
        XCTAssertEqual(all.state, .off); XCTAssertFalse(model.pdf?.string?.contains("Open Zoë") ?? true)
        coordinator.choose(done)
        XCTAssertEqual(done.state, .on); XCTAssertFalse(model.options.includeTodos)
        let restored = ScreenplayPrintOptions(defaults: defaults)
        XCTAssertFalse(restored.includeTodos); XCTAssertTrue(restored.includeCompletedTodos)
        coordinator.choose(all)
        XCTAssertTrue(model.pdf?.string?.contains("Completed task") ?? false)
    }
    func testProjectFlagsAndForeignMalformedFieldsPreserveSourceAndAssetsOnReopen() throws {
        var document = ScreenplayDocument(); document.text = source
        var own = document.metadata[ScreenplayDocument.namespace] as! [String: Any]
        own["screenplayOutput"] = ["version": 1, "includeCompletedTodos": "future", "private": ["keep": true]]
        document.metadata[ScreenplayDocument.namespace] = own
        var options = document.outputOptions(); XCTAssertTrue(options.includeCompletedTodos)
        options.includeTodos = false
        try document.setOutputOptions(options, previous: document.outputOptions())
        let wrapper = try document.makeWrapper(contentType: .fountage)
        let asset = FileWrapper(regularFileWithContents: Data([0, 255])); asset.preferredFilename = "private.bin"; wrapper.addFileWrapper(asset)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertFalse(reopened.outputOptions().includeTodos); XCTAssertTrue(reopened.outputOptions().includeCompletedTodos)
        let fields = (reopened.metadata[ScreenplayDocument.namespace] as! [String: Any])["screenplayOutput"] as! [String: Any]
        XCTAssertEqual(fields["includeCompletedTodos"] as? String, "future"); XCTAssertNotNil(fields["private"])
        XCTAssertEqual(Data(reopened.text.utf8), Data(source.utf8))
        XCTAssertEqual(try reopened.makeWrapper(contentType: .fountage).fileWrappers?["private.bin"]?.regularFileContents, Data([0, 255]))
    }
    @MainActor func testHiddenTasksReflowAndPageGoalsUseFilteredBody() throws {
        let source = "INT. ROOM - DAY\n\n" + (0..<120).map { "[x] Completed task \($0)" }.joined(separator: "\n") + "\n\n[ ] Open task"
        let output = try ScreenplayOutput.parse(source)
        var options = ScreenplayPrintOptions()
        let all = try DraftPageCount.calculate(output: output, options: options)
        XCTAssertGreaterThan(all, 1)
        options.includeCompletedTodos = false
        XCTAssertEqual(try DraftPageCount.calculate(output: output, options: options), 1)
        let layout = try ScreenplayPagination(output: output, options: options)
        XCTAssertEqual(layout.pages.flatMap(\.fragments).filter { $0.text.string.contains("Open task") }.count, 1)
        let onlyTasks = try ScreenplayOutput.parse(Array(repeating: "[x] Hidden", count: 120).joined(separator: "\n\n"))
        XCTAssertEqual(try DraftPageCount.calculate(output: onlyTasks, options: options), 0)
        XCTAssertEqual(try ScreenplayPagination(output: onlyTasks, options: options).pages.count, 1)
    }
}
