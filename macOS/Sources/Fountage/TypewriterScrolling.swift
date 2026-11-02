import AppKit

// Keep AppKit's ordinary selection and drag/autoscroll behavior until the mouse is released.
final class ManuscriptTextView: NSTextView {
    var supportsFountainFormatting = false
    var manuscriptFormat: DocumentFormat = .screenplay
    var onTitlePage: (() -> Void)?
    var onShelfCapture: ((String, NSRange, Bool) throws -> Void)?
    override func insertNewline(_ sender: Any?) {
        if isEditable, !hasMarkedText(), selectedRanges.count == 1,
           let edit = TodoListEdit.newline(source: string, selection: selectedRange(), format: manuscriptFormat), edit.apply(to: self) { return }
        super.insertNewline(sender)
    }
    @objc func editTitlePage(_ sender: Any?) {
        guard supportsFountainFormatting, isEditable, !hasMarkedText() else { return }
        onTitlePage?()
    }
    override func validateUserInterfaceItem(_ item: NSValidatedUserInterfaceItem) -> Bool {
        if [#selector(copySelectionToShelf(_:)), #selector(cutSelectionToShelf(_:))].contains(item.action) {
            return isEditable && !hasMarkedText() && selectedRanges.count == 1 && selectedRange().length > 0 && onShelfCapture != nil
        }
        if item.action == #selector(convertToTodoList(_:)) { return isEditable && !hasMarkedText() && selectedRanges.count == 1 }
        if [#selector(insertScriptMarker(_:)), #selector(insertNamedScriptMarker(_:)), #selector(removeScriptMarkers(_:)), #selector(goToNextScriptMarker(_:))].contains(item.action) {
            return isEditable && !hasMarkedText() && selectedRanges.count == 1
        }
        if item.action == #selector(editTitlePage(_:)) { return supportsFountainFormatting && isEditable && !hasMarkedText() && onTitlePage != nil }
        if item.action == #selector(removeSelectedSceneNumbers(_:)) { return supportsFountainFormatting && isEditable && !hasMarkedText() && selectedRanges.count == 1 && selectedRange().length > 0 }
        if item.action == #selector(configureSceneNumbers(_:)) || item.action == #selector(addSceneNumbers(_:)) || item.action == #selector(removeSceneNumbers(_:)) || item.action == #selector(insertFountainPageBreak(_:)) || FountainFormatting.Style.allCases.contains(where: { $0.selector == item.action }) {
            return supportsFountainFormatting && isEditable && !hasMarkedText() && selectedRanges.count == 1
        }
        return super.validateUserInterfaceItem(item)
    }

    private(set) var isSelectingWithMouse = false
    override func mouseDown(with event: NSEvent) {
        isSelectingWithMouse = true
        super.mouseDown(with: event)
        isSelectingWithMouse = false
        (enclosingScrollView as? ManuscriptScrollView)?.scheduleCentering()
    }
}

final class ManuscriptScrollView: NSScrollView {
    var onWidthChange: (() -> Void)?
    var typewriterScrolling = false {
        didSet {
            guard oldValue != typewriterScrolling else { return }
            updateWritingInsets()
            if typewriterScrolling { scheduleCentering(requiresEditorFocus: false) }
        }
    }
    private var previousSize = NSSize(width: -1, height: -1)
    private var updatingLayout = false
    private var centeringScheduled = false
    private var needsEditorFocus = true

    override func layout() {
        super.layout()
        guard !updatingLayout, contentSize != previousSize else { return }
        updatingLayout = true
        defer { updatingLayout = false }
        let widthChanged = abs(contentSize.width - previousSize.width) > 0.5
        previousSize = contentSize
        if widthChanged { onWidthChange?() }
        updateWritingInsets()
        scheduleCentering()
    }

    // Insets provide scroll room even for the first and last line. They are never source text.
    private func updateWritingInsets() {
        guard let editor = documentView as? NSTextView else { return }
        let oldInset = editor.textContainerInset.height
        let inset = typewriterScrolling ? max(42, contentSize.height / 2) : 42
        let oldOrigin = contentView.bounds.origin
        editor.textContainerInset.height = inset
        editor.minSize.height = contentSize.height
        editor.sizeToFit()
        if abs(inset - oldInset) > 0.5 {
            scroll(to: NSPoint(x: oldOrigin.x, y: oldOrigin.y + inset - oldInset))
        }
    }

    // AppKit completes its normal caret scrolling first. Coalesce selection/style callbacks.
    // Manual wheel/trackpad scrolling never schedules a recenter.
    func scheduleCentering(requiresEditorFocus: Bool = true) {
        guard typewriterScrolling else { return }
        needsEditorFocus = centeringScheduled ? needsEditorFocus && requiresEditorFocus : requiresEditorFocus
        guard !centeringScheduled else { return }
        centeringScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            let requiresFocus = self.needsEditorFocus
            self.centeringScheduled = false
            self.centerInsertionPoint(requiresEditorFocus: requiresFocus)
        }
    }

    func centerInsertionPoint(requiresEditorFocus: Bool = true) {
        guard typewriterScrolling, let editor = documentView as? NSTextView,
              !requiresEditorFocus || editor.window?.firstResponder === editor,
              editor.selectedRanges.count == 1, editor.selectedRange().length == 0,
              !editor.hasMarkedText(),
              (editor as? ManuscriptTextView)?.isSelectingWithMouse != true,
              let center = insertionLineMidY(in: editor) else { return }
        editor.sizeToFit()
        scroll(to: NSPoint(x: contentView.bounds.minX, y: center - contentView.bounds.height / 2))
    }

    private func scroll(to origin: NSPoint) {
        var bounds = contentView.bounds
        bounds.origin = origin
        contentView.scroll(to: contentView.constrainBoundsRect(bounds).origin)
        reflectScrolledClipView(contentView)
    }

    private func insertionLineMidY(in editor: NSTextView) -> CGFloat? {
        guard let layout = editor.layoutManager, let container = editor.textContainer else { return nil }
        let location = min(editor.selectedRange().location, (editor.string as NSString).length)
        layout.ensureLayout(for: container)
        let rect: NSRect
        if location == (editor.string as NSString).length, layout.extraLineFragmentTextContainer === container {
            rect = layout.extraLineFragmentRect
        } else if layout.numberOfGlyphs > 0 {
            let glyph = location == (editor.string as NSString).length
                ? layout.numberOfGlyphs - 1 : layout.glyphIndexForCharacter(at: location)
            rect = layout.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        } else { return nil }
        return editor.textContainerOrigin.y + rect.midY
    }
}
