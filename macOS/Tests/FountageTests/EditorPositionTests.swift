import XCTest
import AppKit
import SwiftUI
@testable import Fountage

final class EditorPositionTests: XCTestCase {
    @MainActor
    func testNativeSelectionReportsCRLFLineToWorkspace() async {
        let source = "Zoë 🌙\r\n\r\nINT. ROOM - DAY\r\n"
        let reported = expectation(description: "Workspace cursor update")
        let native = NativeEditor(text: .constant(source), fontSize: 16, format: .screenplay, preset: .screenplay) { line in
            XCTAssertEqual(line, 2)
            reported.fulfill()
        }
        let coordinator = native.makeCoordinator()
        let editor = NSTextView()
        editor.string = source
        editor.setSelectedRange((source as NSString).range(of: "INT."))
        coordinator.textViewDidChangeSelection(Notification(name: NSTextView.didChangeSelectionNotification, object: editor))
        await fulfillment(of: [reported], timeout: 2)
    }

    func testCRLFAndMixedEndingsReportCorrectLinesAtUTF16Offsets() {
        let source = "Zoë 🌙\r\nCafe\u{301}\n世界\r\n"
        let ns = source as NSString
        XCTAssertEqual(EditorPosition.line(in: source, at: 0), 0)
        XCTAssertEqual(EditorPosition.line(in: source, at: ns.range(of: "Cafe\u{301}").location), 1)
        XCTAssertEqual(EditorPosition.line(in: source, at: ns.range(of: "世界").location), 2)
        XCTAssertEqual(EditorPosition.line(in: source, at: ns.length), 3)
        XCTAssertEqual(EditorPosition.line(in: source, at: -1), 0)
        XCTAssertEqual(EditorPosition.line(in: source, at: Int.max), 3)
        XCTAssertEqual(EditorPosition.line(in: "", at: 20), 0)
    }

    func testOutlineJumpsAndCursorLinesAgreeForEveryWritingFormat() {
        for format in DocumentFormat.allCases {
            let source = format.starter.replacingOccurrences(of: "\n", with: "\r\n")
            let analysis = ManuscriptAnalysis.parse(source, format: format)
            for heading in analysis.headings {
                let range = EditorJump(line: heading.line).selection(in: source)
                XCTAssertEqual(EditorPosition.line(in: source, at: range.location), heading.line, "\(format): \(heading.heading)")
            }
        }
    }
}
