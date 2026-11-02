import XCTest
import SwiftUI
import AppKit
@testable import Fountage

final class AnalysisCacheTests: XCTestCase {
    func testAllFormatsInvalidateOnEditsAndDoNotRetainHistory() {
        var calls = 0
        let cache = ManuscriptAnalysisCache { text, format in
            calls += 1
            return ManuscriptAnalysis.parse(text, format: format)
        }
        for format in DocumentFormat.allCases {
            let source = format.starter
            let before = calls
            let analysis = cache.analysis(for: source, format: format)
            _ = cache.analysis(for: source, format: format)
            XCTAssertEqual(calls, before + 1)
            XCTAssertEqual(analysis.words, ManuscriptAnalysis.parse(source, format: format).words)
            _ = cache.analysis(for: source + "\nA changed ending.", format: format)
            _ = cache.analysis(for: source, format: format)
            XCTAssertEqual(calls, before + 3, "Only the latest snapshot should be retained")
        }
        let source = "# Heading\n\nPAGE 1\n\nPanel 1"
        let before = calls
        _ = cache.analysis(for: source, format: .screenplay)
        let comic = cache.analysis(for: source, format: .comic)
        XCTAssertEqual(calls, before + 2)
        XCTAssertEqual(comic.panels, 1)
    }

    func testCanonicallyEqualUnicodeStillRefreshesExactNoteRanges() {
        let composed = "Café 🌙 [[A note]]\r\n"
        let decomposed = "Cafe\u{301} 🌙 [[A note]]\r\n"
        XCTAssertEqual(composed, decomposed) // Swift's normal equality is intentionally insufficient.
        XCTAssertNotEqual(ExactSource(composed), ExactSource(decomposed))
        let cache = ManuscriptAnalysisCache()
        let first = cache.analysis(for: composed, format: .screenplay).notes[0]
        let second = cache.analysis(for: decomposed, format: .screenplay).notes[0]
        XCTAssertEqual(second.start, first.start + 1)
        XCTAssertEqual((decomposed as NSString).substring(with: second.range), "[[A note]]")
        let lf = decomposed.replacingOccurrences(of: "\r\n", with: "\n")
        XCTAssertNotEqual(ExactSource(decomposed), ExactSource(lf))
    }

    func testCombinedSnapshotSharesIndexesAndInvalidatesOnExactSourceAndFormat() throws {
        var analysisCalls = 0, snapshotCalls = 0
        let cache = ManuscriptAnalysisCache(parse: { text, format in
            analysisCalls += 1; return ManuscriptAnalysis.parse(text, format: format)
        }, snapshot: { source in snapshotCalls += 1; return try ScreenplaySnapshot.parse(source) })
        let source = "Title: Café\r\n\r\nINT. CAFÉ - DAY\r\n\r\nZoë 🌙 [[private]]."
        let snapshot = try cache.screenplay(for: source)
        XCTAssertEqual(snapshot.analysis.units, 1)
        XCTAssertEqual(snapshot.output.body.filter { $0.kind == .scene }.count, 1)
        XCTAssertEqual(cache.analysis(for: source, format: .screenplay).notes.count, 1)
        _ = try cache.screenplay(for: source)
        XCTAssertEqual(snapshotCalls, 1); XCTAssertEqual(analysisCalls, 0)
        let decomposed = source.replacingOccurrences(of: "é", with: "e\u{301}")
        let next = try cache.screenplay(for: decomposed)
        XCTAssertEqual(snapshotCalls, 2)
        XCTAssertGreaterThan(next.analysis.notes[0].start, snapshot.analysis.notes[0].start)
        XCTAssertThrowsError(try cache.screenplay(for: "invalid\0source"))
        _ = cache.analysis(for: decomposed, format: .comic)
        XCTAssertEqual(analysisCalls, 1)
        _ = try cache.screenplay(for: decomposed)
        XCTAssertEqual(snapshotCalls, 4, "Failed parse and format change must not masquerade as cached output")
        _ = try cache.screenplay(for: source)
        XCTAssertEqual(snapshotCalls, 5, "Only one source snapshot may be retained")
    }

    @MainActor func testPageMilestoneEditingUsesOneSharedEnginePass() throws {
        var analysisCalls = 0, snapshotCalls = 0
        let cache = ManuscriptAnalysisCache(parse: { text, format in
            analysisCalls += 1; return ManuscriptAnalysis.parse(text, format: format)
        }, snapshot: { source in snapshotCalls += 1; return try ScreenplaySnapshot.parse(source) })
        var document = ScreenplayDocument()
        try document.addMilestone(unit: .pages, target: 1000, counts: WritingMilestoneCounts(words: 0, characters: 0, pages: nil))
        MilestoneWorkflow.replaceText("INT. ROOM - DAY\n\nA signal.", in: &document, cache: cache)
        let analysis = cache.analysis(for: document.text, format: document.format)
        _ = try cache.screenplay(for: document.text)
        XCTAssertEqual(analysis.units, 1)
        XCTAssertEqual(analysisCalls, 0); XCTAssertEqual(snapshotCalls, 1)
        MilestoneWorkflow.replaceText(document.text + " 🌙", in: &document, cache: cache)
        XCTAssertEqual(snapshotCalls, 2)
        XCTAssertFalse(try document.writingMilestones()[0].completed)
    }

    @MainActor
    func testLongManuscriptSharesOneParseAndDoesNotRestyleUnchangedText() {
        let source = (1...150).map { "# Sequence \($0)\n\nINT. STATION - NIGHT\n\nZoë watches 🌙. [[Keep this note]]\n\nMARA\nA signal arrives.\n\n" }.joined()
        var calls = 0
        let cache = ManuscriptAnalysisCache { text, format in
            calls += 1
            return ManuscriptAnalysis.parse(text, format: format)
        }
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        let editor = NSTextView(frame: scroll.bounds)
        editor.string = source
        scroll.documentView = editor
        let native = NativeEditor(text: .constant(source), fontSize: 16, format: .screenplay, preset: .screenplay, analysisCache: cache, onCursor: { _ in })
        let coordinator = native.makeCoordinator()
        let analysis = cache.analysis(for: source, format: .screenplay) // Navigator request.
        XCTAssertEqual(analysis.units, 150)
        coordinator.refreshDisplay(in: editor)
        var styleNotifications = 0
        let observer = NotificationCenter.default.addObserver(forName: NSTextStorage.didProcessEditingNotification, object: editor.textStorage, queue: nil) { _ in styleNotifications += 1 }
        defer { NotificationCenter.default.removeObserver(observer) }
        for _ in 0..<10 {
            coordinator.refreshDisplay(in: editor)
            _ = cache.analysis(for: source, format: .screenplay)
        }
        XCTAssertEqual(calls, 1)
        XCTAssertEqual(styleNotifications, 0)
        let selection = analysis.notes[80].range
        editor.setSelectedRange(selection)
        coordinator.parent.fontSize = 20
        coordinator.refreshDisplay(in: editor)
        XCTAssertEqual(calls, 1, "Font changes require styling, not reparsing")
        XCTAssertGreaterThan(styleNotifications, 0)
        XCTAssertEqual(editor.selectedRange(), selection)
        XCTAssertEqual(editor.string, source)
        editor.string += "An extra sentence."
        coordinator.refreshDisplay(in: editor)
        let updated = cache.analysis(for: editor.string, format: .screenplay)
        XCTAssertEqual(calls, 2)
        XCTAssertGreaterThan(updated.words, analysis.words)
    }
}
