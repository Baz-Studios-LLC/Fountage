import XCTest
import AppKit
import UniformTypeIdentifiers
@testable import Fountage

final class ProjectTests: XCTestCase {
    func testWhitePaperKeepsDarkChromeAndSourceSelection() {
        let chrome = NSView()
        chrome.appearance = NSAppearance(named: .darkAqua)
        let scroll = NSScrollView()
        let editor = NSTextView()
        chrome.addSubview(scroll)
        scroll.documentView = editor
        editor.string = "INT. ROOM - DAY\n\nMARA\nHello. [[A note]]\n"
        let source = editor.string
        let selected = NSRange(location: 21, length: 5)
        editor.setSelectedRange(selected)
        EditorPaperAppearance.white.apply(to: scroll, editor: editor)
        ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
        XCTAssertEqual(chrome.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]), .darkAqua)
        XCTAssertEqual(editor.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]), .aqua)
        XCTAssertEqual(editor.backgroundColor, .white)
        XCTAssertEqual(editor.string, source)
        XCTAssertEqual(editor.selectedRange(), selected)
        editor.effectiveAppearance.performAsCurrentDrawingAppearance {
            let textColor = (editor.textStorage?.attribute(.foregroundColor, at: 21, effectiveRange: nil) as? NSColor)?.usingColorSpace(.deviceRGB)
            XCTAssertLessThan(textColor?.redComponent ?? 1, 0.3)
            XCTAssertLessThan(editor.insertionPointColor.usingColorSpace(.deviceRGB)?.greenComponent ?? 1, 0.6)
        }
        EditorPaperAppearance.matchApp.apply(to: scroll, editor: editor)
        XCTAssertNil(scroll.appearance)
        XCTAssertNil(editor.appearance)
        XCTAssertEqual(editor.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]), .darkAqua)
        XCTAssertEqual(editor.string, source)
    }
    func testNavigationUsesUTF16AndClampsStaleRanges() {
        let source = "Zoë 🌙\r\n[[A note]]\r\n"
        XCTAssertEqual(EditorJump(line: 1).selection(in: source), NSRange(location: 8, length: 0))
        let note = ManuscriptAnalysis.parse(source, format: .screenplay).notes[0]
        XCTAssertEqual((source as NSString).substring(with: EditorJump(line: note.line, range: note.range).selection(in: source)), "[[A note]]")
        XCTAssertEqual(EditorJump(line: 999, range: note.range).selection(in: "x"), NSRange(location: 1, length: 0))
        XCTAssertEqual(EditorJump(line: 999).selection(in: "x"), NSRange(location: 1, length: 0))
    }
    func testScriptNotesUseExactAppKitRangesAndSurviveSaving() throws {
        let source = "Title: Café\r\n\r\n# Act One\r\n\r\n= A quiet arrival.\r\n\r\nEXT. CAFÉ - DAY\r\n\r\nZoë sees 🌙. [[Check the time.]] Then [[ask Éli.]]\r\n\r\n/*\r\nINT. OMITTED - NIGHT\r\n[[Hidden note]]\r\n*/\r\n"
        var document = ScreenplayDocument()
        document.text = source
        let analysis = ManuscriptAnalysis.parse(source, format: .screenplay)
        XCTAssertEqual(analysis.units, 1)
        XCTAssertEqual(analysis.headings.map(\.kind), [.section, .synopsis, .scene])
        XCTAssertEqual(analysis.notes.count, 2)
        XCTAssertNotEqual(analysis.notes[0].id, analysis.notes[1].id)
        for note in analysis.notes {
            XCTAssertEqual((source as NSString).substring(with: note.range), "[[\(note.text)]]")
        }
        let restored = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(restored.text, source)
        XCTAssertEqual(ManuscriptAnalysis.parse(restored.text, format: restored.format).notes.map(\.text), ["Check the time.", "ask Éli."])
    }
    func testAnnotationStylesPreserveSourceAndClearWhenRemoved() {
        let editor = NSTextView()
        let source = "EXT. CAFÉ - DAY\n\nZoë 🌙 [[Check this.]] walks on. /*omitted*/\n"
        editor.string = source
        ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
        XCTAssertEqual(editor.string, source)
        let note = ManuscriptAnalysis.parse(source, format: .screenplay).notes[0]
        XCTAssertNotNil(editor.textStorage?.attribute(.backgroundColor, at: note.start, effectiveRange: nil))
        XCTAssertNil(editor.textStorage?.attribute(.backgroundColor, at: 0, effectiveRange: nil))
        editor.string = source.replacingOccurrences(of: "[[", with: "").replacingOccurrences(of: "]]", with: "")
        ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
        XCTAssertNil(editor.textStorage?.attribute(.backgroundColor, at: note.start, effectiveRange: nil))
    }
    func testCenteredTextAndLyricsHaveDistinctDisplayStyles() {
        let centered = ManuscriptStyle.attributes(kind: .centered, format: .screenplay, preset: .screenplay, size: 16, width: 600)
        XCTAssertEqual((centered[.paragraphStyle] as? NSParagraphStyle)?.alignment, .center)
        let lyric = ManuscriptStyle.attributes(kind: .lyric, format: .screenplay, preset: .screenplay, size: 16, width: 600)
        XCTAssertTrue(NSFontManager.shared.traits(of: lyric[.font] as! NSFont).contains(.italicFontMask))
    }
    func testProjectRoundTripPreservesMetadataAssetsAndUnknownEntries() throws {
        var original = ScreenplayDocument()
        original.text = "Title: Café\n\nINT. ROOM - DAY\n\n@Zoë\nHello, 世界.\n"
        original.metadata["another.editor"] = ["version": 7, "futureField": ["keep": true]]
        var own = original.metadata[ScreenplayDocument.namespace] as! [String: Any]
        own["unknownFutureSetting"] = "keep me"
        original.metadata[ScreenplayDocument.namespace] = own
        let asset = Data([0, 1, 2, 255])
        original.retainedEntries["assets"] = .directory(["image.bin": .file(asset)])
        original.retainedEntries["foreign"] = .directory(["opaque.dat": .file(asset)])
        let wrapper = try original.makeWrapper(contentType: .fountage)
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(reopened.text, original.text)
        XCTAssertEqual((reopened.metadata[ScreenplayDocument.namespace] as? [String: Any])?["projectID"] as? String, own["projectID"] as? String)
        XCTAssertEqual((reopened.metadata[ScreenplayDocument.namespace] as? [String: Any])?["unknownFutureSetting"] as? String, "keep me")
        XCTAssertNotNil(reopened.metadata["another.editor"])
        XCTAssertEqual(reopened.retainedEntries["assets"]?.fileWrapper().fileWrappers?["image.bin"]?.regularFileContents, asset)
        XCTAssertEqual(reopened.retainedEntries["foreign"]?.fileWrapper().fileWrappers?["opaque.dat"]?.regularFileContents, asset)
    }
    func testPlainFountainExportIsExact() throws {
        var document = ScreenplayDocument()
        document.text = "EXT. CAFÉ - DAY\r\n\r\nUnicode 🎬\r\n"
        let wrapper = try document.makeWrapper(contentType: .fountain)
        XCTAssertEqual(wrapper.regularFileContents, Data(document.text.utf8))
        XCTAssertEqual(try ScreenplayDocument(wrapper: wrapper, contentType: .fountain).text, document.text)
    }
    func testFutureVersionAndBrokenProjectAreRejected() throws {
        let valid = try ScreenplayDocument().makeWrapper(contentType: .fountage)
        let future = try replacingMetadata(valid) { info in
            var own = info[ScreenplayDocument.namespace] as! [String: Any]
            own["version"] = 999
            info[ScreenplayDocument.namespace] = own
        }
        XCTAssertThrowsError(try ScreenplayDocument(wrapper: future, contentType: .fountage))
        XCTAssertThrowsError(try ScreenplayDocument(wrapper: FileWrapper(directoryWithFileWrappers: [:]), contentType: .fountage))
    }
    func testDirectoryMasqueradingAsTextIsRejected() throws {
        let valid = try ScreenplayDocument().makeWrapper(contentType: .fountage)
        var files = valid.fileWrappers!
        files["text.fountain"] = FileWrapper(directoryWithFileWrappers: [:])
        XCTAssertThrowsError(try ScreenplayDocument(wrapper: FileWrapper(directoryWithFileWrappers: files), contentType: .fountage))
        XCTAssertThrowsError(try ScreenplayDocument(wrapper: FileWrapper(directoryWithFileWrappers: [:]), contentType: .fountain))
    }
    func testRustBridgeHandlesUnicode() {
        let outline = Outline.parse("INT. CAFÉ - DAY\n\n@Zoë\nHello, 世界.")
        XCTAssertEqual(outline.scenes.count, 1)
        XCTAssertEqual(outline.characters["Zoë"], 1)
    }
    func testEveryFormatAndPresetRoundTripsWithoutChangingText() throws {
        for format in DocumentFormat.allCases {
            for preset in format.presets {
                var document = ScreenplayDocument()
                document.text = "Title: Café\r\n\r\nA story with 世界 and 🎬.\r\n"
                let source = document.text
                document.format = format
                document.preset = preset
                let wrapper = try document.makeWrapper(contentType: .fountage)
                let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
                XCTAssertEqual(reopened.text, source)
                XCTAssertEqual(reopened.format, format)
                XCTAssertEqual(reopened.preset, preset)
                XCTAssertEqual(wrapper.fileWrappers?[format.sourceFilename]?.regularFileContents, Data(source.utf8))
                XCTAssertEqual(try reopened.makeWrapper(contentType: .plainText).regularFileContents, Data(source.utf8))
                if format != .screenplay { XCTAssertThrowsError(try reopened.makeWrapper(contentType: .fountain)) }
            }
        }
    }
    func testSchemaOneMigrationAndFormatSwitchPreserveIdentity() throws {
        let valid = try ScreenplayDocument().makeWrapper(contentType: .fountage)
        let legacy = try replacingMetadata(valid) { info in
            var own = info[ScreenplayDocument.namespace] as! [String: Any]
            own["version"] = 1
            for key in ["documentFormat", "layoutPreset", "sourceFile"] { own.removeValue(forKey: key) }
            info[ScreenplayDocument.namespace] = own
        }
        var document = try ScreenplayDocument(wrapper: legacy, contentType: .fountage)
        let identity = (document.metadata[ScreenplayDocument.namespace] as! [String: Any])["projectID"] as! String
        XCTAssertEqual(document.format, .screenplay)
        document.text = "The same source."
        document.format = .comic
        let wrapper = try document.makeWrapper(contentType: .fountage)
        XCTAssertNil(wrapper.fileWrappers?["text.fountain"])
        let reopened = try ScreenplayDocument(wrapper: wrapper, contentType: .fountage)
        XCTAssertEqual(reopened.format, .comic)
        XCTAssertEqual(reopened.text, "The same source.")
        let own = reopened.metadata[ScreenplayDocument.namespace] as! [String: Any]
        XCTAssertEqual(own["version"] as? Int, 2)
        XCTAssertEqual(own["projectID"] as? String, identity)
    }
    func testStyleEditionsAndUnrecognizedStandardsArePreserved() throws {
        for style in AcademicStyle.choices + [AcademicStyle(family: "apa", edition: 6, variant: "student")] {
            var document = ScreenplayDocument()
            document.format = .academic
            document.text = "# Introduction\n\nEvidence (Smith, 2020)."
            document.academicStyle = style
            document.format = .novel
            document.format = .academic
            let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
            XCTAssertEqual(reopened.academicStyle, style)
            XCTAssertEqual(reopened.text, document.text)
        }
    }
    func testInvalidPresetAndSourcePathAreRejected() throws {
        let valid = try ScreenplayDocument().makeWrapper(contentType: .fountage)
        for change in [["layoutPreset": "novel-reading"], ["sourceFile": "../text.fountain"]] {
            let altered = try replacingMetadata(valid) { info in
                var own = info[ScreenplayDocument.namespace] as! [String: Any]
                for (key, value) in change { own[key] = value }
                info[ScreenplayDocument.namespace] = own
            }
            XCTAssertThrowsError(try ScreenplayDocument(wrapper: altered, contentType: .fountage))
        }
    }
    func testChangingFormatCannotOverwriteAnExistingAttachment() throws {
        var document = ScreenplayDocument()
        document.retainedEntries["text.txt"] = .file(Data("keep this attachment".utf8))
        var reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        reopened.format = .novel
        XCTAssertThrowsError(try reopened.makeWrapper(contentType: .fountage))
    }
    @MainActor
    func testScreenplayStylesIndentWithoutModifyingUnicodeSource() {
        let editor = NSTextView()
        let source = "INT. CAFÉ - DAY\n\n@Zoë\n(quietly)\nHello, 世界.\n\nA new action."
        editor.string = source
        editor.setSelectedRange(NSRange(location: 7, length: 0))
        ManuscriptStyle.apply(to: editor, format: .screenplay, preset: .screenplay, size: 16)
        XCTAssertEqual(editor.string, source)
        XCTAssertEqual(editor.selectedRange().location, 7)
        func style(_ text: String) -> NSParagraphStyle {
            let offset = (source as NSString).range(of: text).location
            return editor.textStorage!.attribute(.paragraphStyle, at: offset, effectiveRange: nil) as! NSParagraphStyle
        }
        XCTAssertGreaterThan(style("@Zoë").headIndent, style("Hello").headIndent)
        XCTAssertGreaterThan(style("Hello").headIndent, 0)
        XCTAssertLessThan(style("Hello").tailIndent, 0)
        XCTAssertEqual(style("A new action").headIndent, 0)
        ManuscriptStyle.apply(to: editor, format: .novel, preset: .novelReading, size: 18)
        XCTAssertEqual(editor.string, source)
    }
    func testFormatAwareRustBridge() {
        let comic = ManuscriptAnalysis.parse("PAGE 1\n\nPanel 1\n\nMARA: Hello.", format: .comic)
        XCTAssertEqual(comic.units, 1)
        XCTAssertEqual(comic.panels, 1)
        XCTAssertEqual(comic.characters["MARA"], 1)
        let paper = ManuscriptAnalysis.parse("# Introduction\n\n## Context", format: .academic)
        XCTAssertEqual(paper.headings.map(\.heading), ["Introduction", "Context"])
        XCTAssertEqual(paper.headings.map(\.level), [0, 1])
    }

    @MainActor
    func testPoetrySpacingSurvivesDisplayAndProjectRoundTrip() throws {
        var document = ScreenplayDocument()
        document.format = .poetry
        document.preset = .lyrics
        document.text = "Title: A Song\n\nVERSE 1\n  café\n    世界\n\nCHORUS\nAgain.\n"
        let editor = NSTextView()
        editor.string = document.text
        ManuscriptStyle.apply(to: editor, format: document.format, preset: document.preset, size: 16)
        XCTAssertEqual(editor.string, document.text)
        let offset = (document.text as NSString).range(of: "café").location
        let paragraph = editor.textStorage!.attribute(.paragraphStyle, at: offset, effectiveRange: nil) as! NSParagraphStyle
        XCTAssertEqual(paragraph.firstLineHeadIndent, 0)
        let reopened = try ScreenplayDocument(wrapper: document.makeWrapper(contentType: .fountage), contentType: .fountage)
        XCTAssertEqual(reopened.text, document.text)
        XCTAssertEqual(reopened.preset, .lyrics)
        XCTAssertEqual(ManuscriptAnalysis.parse(reopened.text, format: reopened.format).units, 2)
    }
    func testAdditionalStartersReachTheirOwnRustAnalysis() {
        let play = ManuscriptAnalysis.parse(DocumentFormat.stageplay.starter, format: .stageplay)
        XCTAssertEqual(play.units, 1)
        XCTAssertEqual(play.characters["CHARACTER"], 2)
        let podcast = ManuscriptAnalysis.parse(DocumentFormat.audio.starter(for: .podcast), format: .audio)
        XCTAssertEqual(podcast.units, 2)
        XCTAssertEqual(podcast.characters.keys.sorted(), ["GUEST", "HOST"])
        let outline = ManuscriptAnalysis.parse(DocumentFormat.general.starter(for: .generalOutline), format: .general)
        XCTAssertEqual(outline.headings.map(\.level), [0, 1, 0])
        let lyrics = ManuscriptAnalysis.parse(DocumentFormat.poetry.starter(for: .lyrics), format: .poetry)
        XCTAssertEqual(lyrics.units, 4)
        XCTAssertEqual(lyrics.headings.count, 4)
    }
    private func replacingMetadata(_ wrapper: FileWrapper, edit: (inout [String: Any]) -> Void) throws -> FileWrapper {
        var files = wrapper.fileWrappers!
        var info = try JSONSerialization.jsonObject(with: files["info.json"]!.regularFileContents!) as! [String: Any]
        edit(&info)
        files["info.json"] = FileWrapper(regularFileWithContents: try JSONSerialization.data(withJSONObject: info))
        return FileWrapper(directoryWithFileWrappers: files)
    }

}
