import AppKit

// Display styles only. Source text never contains spaces inserted to simulate indentation.
enum ManuscriptStyle {
    static func attributes(kind: ElementKind, format: DocumentFormat, preset: LayoutPreset, size: CGFloat, width: CGFloat) -> [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineSpacing = 5
        var font = NSFont(name: [DocumentFormat.novel, .academic, .general].contains(format) ? "Georgia" : "Courier", size: size) ?? .monospacedSystemFont(ofSize: size, weight: .regular)
        var color = NSColor.textColor
        let usable = max(180, width)
        if format == .screenplay {
            switch kind {
            case .character: paragraph.headIndent = usable * 0.36; paragraph.firstLineHeadIndent = paragraph.headIndent
            case .dialogue: paragraph.headIndent = usable * 0.20; paragraph.firstLineHeadIndent = paragraph.headIndent; paragraph.tailIndent = -usable * 0.18
            case .parenthetical: paragraph.headIndent = usable * 0.29; paragraph.firstLineHeadIndent = paragraph.headIndent; paragraph.tailIndent = -usable * 0.23
            case .transition: paragraph.alignment = .right
            case .scene: font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            case .section:
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
                color = StudioTheme.accent
                paragraph.paragraphSpacingBefore = 8
            case .synopsis:
                font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
                color = .secondaryLabelColor
            case .centered, .pageBreak: paragraph.alignment = .center
            case .lyric: font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
            default: break
            }
        } else if format == .comic {
            switch kind {
            case .page:
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
                paragraph.paragraphSpacingBefore = 20; paragraph.paragraphSpacing = 8
            case .panel:
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
                paragraph.paragraphSpacingBefore = 8
            case .dialogue:
                if preset == .comicFull { paragraph.headIndent = 22; paragraph.firstLineHeadIndent = 22 }
            case .balloon, .caption, .soundEffect:
                if preset == .comicFull { paragraph.headIndent = 22; paragraph.firstLineHeadIndent = 22 }
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            default: break
            }
        } else if format == .stageplay {
            switch kind {
            case .act, .scene:
                paragraph.alignment = .center
                paragraph.paragraphSpacingBefore = 20
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            case .character:
                paragraph.alignment = .center
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            case .parenthetical:
                paragraph.headIndent = usable * 0.16; paragraph.firstLineHeadIndent = paragraph.headIndent
                paragraph.tailIndent = -usable * 0.10
                font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
            default: break
            }
        } else if format == .audio {
            switch kind {
            case .scene, .heading:
                paragraph.paragraphSpacingBefore = 16
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            case .character:
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            case .dialogue: paragraph.headIndent = 18; paragraph.firstLineHeadIndent = 18
            case .soundEffect:
                color = .secondaryLabelColor
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            case .parenthetical: font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
            default: break
            }
        } else if format == .poetry {
            paragraph.lineSpacing = 4
            if kind == .heading || kind == .verse {
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
                paragraph.paragraphSpacingBefore = 12
                paragraph.paragraphSpacing = 5
            }
        } else if format == .general {
            paragraph.lineHeightMultiple = 1.3
            if kind == .heading {
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
                paragraph.paragraphSpacingBefore = 12
                paragraph.paragraphSpacing = 6
            }
        } else if format == .academic {
            paragraph.lineHeightMultiple = 1.6
            if kind == .heading {
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
                paragraph.paragraphSpacingBefore = 16
                paragraph.paragraphSpacing = 8
            }
        } else {
            paragraph.lineHeightMultiple = preset == .novelManuscript ? 1.8 : 1.3
            switch kind {
            case .chapter:
                paragraph.alignment = .center; paragraph.paragraphSpacingBefore = 28; paragraph.paragraphSpacing = 16
                font = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
            case .sceneBreak: paragraph.alignment = .center; paragraph.paragraphSpacingBefore = 12; paragraph.paragraphSpacing = 12
            case .body: paragraph.firstLineHeadIndent = size * 1.6
            default: break
            }
        }
        if kind == .title || kind == .note || kind == .omitted { color = .secondaryLabelColor }
        return [.font: font, .paragraphStyle: paragraph, .foregroundColor: color]
    }
    static func apply(to editor: NSTextView, format: DocumentFormat, preset: LayoutPreset, size: CGFloat, analysisCache: ManuscriptAnalysisCache? = nil) {
        guard let storage = editor.textStorage, !editor.hasMarkedText() else { return }
        let source = editor.string as NSString
        let analysis = analysisCache?.analysis(for: editor.string, format: format) ?? ManuscriptAnalysis.parse(editor.string, format: format)
        let kinds = Dictionary(uniqueKeysWithValues: analysis.elements.map { ($0.line, $0.kind) })
        let width = max(180, (editor.enclosingScrollView?.contentSize.width ?? 700) - 2 * editor.textContainerInset.width - 10)
        storage.beginEditing()
        // Match the engine's LF/CRLF source lines. NSString.lineRange also splits
        // Unicode separators and would shift every subsequent screenplay role.
        for (line, range) in EditorPosition.lineRanges(in: editor.string).enumerated() {
            storage.setAttributes(attributes(kind: kinds[line] ?? .body, format: format, preset: preset, size: size, width: width), range: range)
        }
        for span in analysis.emphasis {
            let range = NSIntersectionRange(NSRange(location: span.start, length: span.length), NSRange(location: 0, length: source.length))
            var updates: [(NSRange, NSFont)] = []
            storage.enumerateAttribute(.font, in: range) { value, run, _ in
                guard let font = value as? NSFont else { return }
                var traits: NSFontTraitMask = []
                if span.bold { traits.insert(.boldFontMask) }
                if span.italic { traits.insert(.italicFontMask) }
                updates.append((run, NSFontManager.shared.convert(font, toHaveTrait: traits)))
            }
            for (run, font) in updates { storage.addAttribute(.font, value: font, range: run) }
            if span.underline { storage.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: range) }
        }
        for note in analysis.notes {
            let range = NSIntersectionRange(note.range, NSRange(location: 0, length: source.length))
            storage.addAttributes([.foregroundColor: NSColor.secondaryLabelColor, .backgroundColor: StudioTheme.accent.withAlphaComponent(0.08)], range: range)
        }
        for omission in analysis.omissions {
            let range = NSIntersectionRange(omission.range, NSRange(location: 0, length: source.length))
            storage.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: range)
        }
        storage.endEditing()
        let position = min(editor.selectedRange().location, source.length)
        let currentLine = EditorPosition.line(in: editor.string, at: position)
        editor.typingAttributes = attributes(kind: kinds[currentLine] ?? .body, format: format, preset: preset, size: size, width: width)
    }
}
