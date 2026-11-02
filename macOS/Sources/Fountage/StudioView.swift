import SwiftUI
import AppKit

private enum NavigatorSection { case outline, notes, markers }

struct StudioView: View {
    @Binding var document: ScreenplayDocument
    let fileURL: URL?
    @Environment(\.newDocument) private var newDocument
    @State private var navigatorError: String?
    @State private var formatError: String?
    @Environment(\.undoManager) private var undoManager
    @StateObject private var outputSettings = ProjectOutputSettings()
    @Environment(\.colorScheme) private var colorScheme
    @AppStorage("editorPaperAppearance") private var paper = EditorPaperAppearance.matchApp
    @AppStorage("typewriterScrolling") private var typewriterScrolling = false
    @State private var outline = ManuscriptAnalysis()
    @State private var analysisCache = ManuscriptAnalysisCache()
    @State private var outlineIndex = OutlineIndex()
    @State private var collapsed: Set<OutlineIndex.Key> = []
    @State private var selection: Int?
    @State private var jump: EditorJump?
    @State private var outlineMoveRequest: OutlineMoveRequest?
    @StateObject private var outlineDrag = OutlineDragStore()
    @State private var showingReferences = false
    @State private var showingTodos = false
    @State private var showingShelf = false
    @State private var inspectorSection = "Draft"
    @State private var editorSelectionSource = ""
    @State private var editorSelection = NSRange(location: NSNotFound, length: 0)
    @State private var shelfSource = ""
    @State private var shelfSelection = NSRange(location: NSNotFound, length: 0)
    @State private var showingMilestones = false
    @State private var showingPreview = false
    @State private var showingTitlePage = false
    @State private var edit: ManuscriptEdit?
    @State private var cursorLine = 0
    @State private var selectedWords: Int?
    @State private var showInspector = true
    @State private var columns: NavigationSplitViewVisibility = .all
    @State private var focus = false
    @State private var layoutBeforeFocus: (columns: NavigationSplitViewVisibility, inspector: Bool)?
    @AppStorage("editorFontSize") private var storedFontSize = 16.0
    private var fontSize: CGFloat { CGFloat(min(26, max(12, storedFontSize.isFinite ? storedFontSize : 16))) }
    @State private var headingQuery = ""
    @State private var navigator = NavigatorSection.outline
    @State private var noteSelection: Int?
    private let accent = Color(nsColor: StudioTheme.accent)
    private var isProject: Bool { fileURL == nil || fileURL?.pathExtension.lowercased() == "fountage" }
    var title: String {
        document.text.components(separatedBy: .newlines).first(where: { $0.hasPrefix("Title:") })?
            .dropFirst(6).trimmingCharacters(in: .whitespaces)
            .nonEmpty ?? fileURL?.deletingPathExtension().lastPathComponent ?? "Untitled \(document.format.name.lowercased())"
    }
    var headings: [OutlineIndex.Row] {
        navigatorEntries.compactMap(\.row)
    }
    private var navigatorEntries: [NavigatorOutline.Entry] {
        NavigatorOutline.entries(index: outlineIndex, markers: outline.markers, notes: outline.notes, todos: outline.todos, collapsed: collapsed, query: headingQuery, hiddenKinds: document.navigatorHiddenKinds)
    }
    private var visibleExpandableKeys: Set<OutlineIndex.Key> {
        Set(NavigatorOutline.entries(index: outlineIndex, markers: outline.markers, notes: outline.notes, todos: outline.todos, collapsed: [], query: "", hiddenKinds: document.navigatorHiddenKinds).compactMap(\.row).filter(\.hasChildren).map(\.key))
    }
    private var navigatorCount: String {
        if navigator != .outline { return headingQuery.isEmpty ? "\(navigator == .markers ? outline.markers.count : outline.notes.count)" : "\(notes.count) shown" }
        if !headingQuery.isEmpty || !document.navigatorHiddenKinds.intersection(NavigatorFilters.kinds(for: document.format)).isEmpty { return "\(navigatorEntries.count) shown" }
        return "\(outline.units)"
    }
    var notes: [ManuscriptNote] {
        (navigator == .markers ? outline.markers : outline.notes).filter { headingQuery.isEmpty || $0.text.localizedCaseInsensitiveContains(headingQuery) }
    }
    var body: some View {
        if document.needsTypeSelection {
            NewDocumentChooser { format, preset, academicStyle, includeStarter in
                document.format = format
                document.preset = preset
                document.academicStyle = academicStyle
                if includeStarter { document.text = format.starter(for: preset) }
                document.needsTypeSelection = false
            }
        } else {
            workspace
        }
    }
    private var workspaceChrome: some View {
        NavigationSplitView(columnVisibility: $columns) { navigatorSidebar } detail: { manuscriptPane }
        .inspector(isPresented: $showInspector) { inspector.inspectorColumnWidth(min: 200, ideal: 225, max: 310) }
        .overlay(alignment: .top) {
            // One continuous separator below the toolbar, independent of pane widths.
            Divider().allowsHitTesting(false).accessibilityHidden(true)
        }
        .toolbarBackground(Color(nsColor: .windowBackgroundColor), for: .windowToolbar)
        .toolbarBackground(.visible, for: .windowToolbar)
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    documentFormatMenu
                    if document.format == .screenplay {
                        Button { showingPreview = true } label: { Label("Print Preview", systemImage: "doc.text.magnifyingglass") }
                            .help("Preview, print or save a screenplay PDF").keyboardShortcut("p", modifiers: [.command, .shift])
                    }
                    Button { setFocus(!focus) } label: { Label(focus ? "Leave Focus" : "Focus", systemImage: "viewfinder") }
                        .help("Show only the manuscript").keyboardShortcut("f", modifiers: [.command, .shift])
                    Menu {
                        Text("Editor Type: \(Int(fontSize)) pt")
                        Divider()
                        Button("Smaller Type") { storedFontSize = Double(fontSize - 1) }
                            .disabled(fontSize <= 12)
                        Button("Larger Type") { storedFontSize = Double(fontSize + 1) }
                            .disabled(fontSize >= 26)
                        Button("Actual Size") { storedFontSize = 16 }
                            .disabled(fontSize == 16)
                    } label: { Label("Text Size", systemImage: "textformat.size") }.help("Editor text size")
                    Button { showInspector.toggle() } label: { Label("Manuscript Inspector", systemImage: "sidebar.right") }.help("Show or hide manuscript inspector")
                }
            }
    }
    private var workspacePresentation: some View {
        workspaceChrome
        .sheet(isPresented: $showingTitlePage) {
            TitlePageEditor(source: document.text) { proposed in
                guard proposed.source.utf8.elementsEqual(document.text.utf8) else { throw TitlePageEdit.EditError.staleSource }
                edit = proposed
            }
        }
        .sheet(isPresented: $showingPreview) { screenplayPreview }
        .sheet(isPresented: $showingShelf) {
            ShelfPanel(document: $document, insert: { content in
                edit = try ShelfInsertion.edit(source: document.text, expectedSource: shelfSource, selection: shelfSelection, content: content)
            }, canCapture: shelfSource.utf8.elementsEqual(document.text.utf8) && shelfSelection.location != NSNotFound && shelfSelection.length > 0,
                capture: { cut in try captureToShelf(source: shelfSource, range: shelfSelection, cut: cut) })
        }
        .sheet(isPresented: $showingMilestones) { MilestonePanel(document: $document) }
        .sheet(item: $outlineMoveRequest) { request in
            OutlineMoveSheet(request: request) { target, after in
                guard let change = request.edit(target: target, after: after, currentSource: document.text, currentFormat: document.format) else { return false }
                edit = change; return true
            }
        }
        .alert("Document Type Unavailable", isPresented: Binding(get: { formatError != nil }, set: { if !$0 { formatError = nil } })) {
            Button("OK") { formatError = nil }
        } message: { Text(formatError ?? "") }
        .alert("Navigator Unavailable", isPresented: Binding(get: { navigatorError != nil }, set: { if !$0 { navigatorError = nil } })) {
            Button("OK") { navigatorError = nil }
        } message: { Text(navigatorError ?? "") }
        .focusedSceneValue(\.writerMenuState, WriterMenuState(editable: !showingShelf && !showingReferences && !showingTodos && !showingPreview && !showingTitlePage && !showingMilestones && editorSelection.location != NSNotFound && editorSelectionSource.utf8.elementsEqual(document.text.utf8), screenplay: document.format == .screenplay, hasSelection: editorSelection.length > 0))
        .focusedSceneValue(\.canCaptureShelfSelection, isProject && document.canEditShelf && !showingShelf && !showingReferences && !showingTodos && !showingPreview && !showingTitlePage && !showingMilestones && editorSelectionSource.utf8.elementsEqual(document.text.utf8) && editorSelection.location != NSNotFound && editorSelection.length > 0)
        .tint(accent)
    }
    private var workspace: some View {
        workspacePresentation
        .onAppear { analyze() }
        .onChange(of: ExactSource(document.text)) { _, _ in outlineDrag.cancel(); selection = nil; noteSelection = nil; analyze() }
        .onChange(of: document.format) { _, _ in outlineDrag.cancel(); selection = nil; noteSelection = nil; navigator = .outline; headingQuery = ""; analyze() }
        .onChange(of: navigator) { _, _ in headingQuery = ""; selection = nil; noteSelection = nil }
        .onChange(of: document.navigatorHiddenKinds) { _, _ in outlineDrag.cancel(); clearHiddenSelection() }
        .onChange(of: collapsed) { _, _ in clearHiddenSelection() }
        .onChange(of: headingQuery) { _, _ in outlineDrag.cancel(); clearHiddenSelection() }
        .onChange(of: selection) { _, id in
            if let entry = navigatorEntries.first(where: { $0.id == id }) {
                jump = EditorJump(line: entry.line, range: entry.annotation?.range, focusEditor: false)
            }
        }
        .onChange(of: noteSelection) { _, id in
            if let note = notes.first(where: { $0.id == id }) { jump = EditorJump(line: note.line, range: note.range, focusEditor: false) }
        }
    }
    private func outlineRow(_ row: OutlineIndex.Row) -> some View {
        let isCollapsed = collapsed.contains(row.key) && headingQuery.isEmpty
        return HStack(alignment: .center, spacing: 5) {
            if row.hasChildren {
                Button { toggle(row) } label: {
                    Image(systemName: isCollapsed ? "chevron.right" : "chevron.down")
                        .font(.system(size: 10, weight: .semibold)).foregroundStyle(.secondary)
                        .frame(width: 16, height: 32).contentShape(Rectangle())
                }.buttonStyle(.plain)
                    .disabled(!headingQuery.isEmpty)
                    .accessibilityLabel("\(isCollapsed ? "Expand" : "Collapse") \(row.heading.heading)")
                    .accessibilityValue(isCollapsed ? "Collapsed" : "Expanded")
                    .help(headingQuery.isEmpty ? "\(isCollapsed ? "Expand" : "Collapse") group" : "Groups stay expanded while searching")
            } else { Color.clear.frame(width: 16, height: 1).accessibilityHidden(true) }
            VStack(alignment: .leading, spacing: 6) {
                Text(headingLabel(row.heading))
                    .font(.system(size: 9, weight: .medium)).tracking(1).foregroundStyle(.secondary)
                Text(row.heading.heading).font(.system(size: 12, weight: .medium)).lineLimit(3).lineSpacing(3)
            }
        }.padding(.vertical, 8).padding(.leading, CGFloat(min(row.depth, 4)) * 10)
            .contextMenu {
                Button("Move Up") { moveOutline(row, down: false) }
                    .disabled(!headingQuery.isEmpty || outlineIndex.neighbor(of: row.heading.line, down: false) == nil)
                Button("Move Down") { moveOutline(row, down: true) }
                    .disabled(!headingQuery.isEmpty || outlineIndex.neighbor(of: row.heading.line, down: true) == nil)
                Button("Move…") { outlineMoveRequest = OutlineMoveRequest(line: row.heading.line, source: document.text, format: document.format) }
                    .disabled(!headingQuery.isEmpty || [ElementKind.synopsis, .pageBreak].contains(row.heading.kind))
            }
    }
    private func annotationRow(_ entry: NavigatorOutline.Entry) -> some View {
        let snapshot = document.text
        let annotation = entry.annotation!
        let label = entry.kind == .todo ? (entry.todo?.completed == true ? "COMPLETED TO-DO" : "TO-DO") : entry.kind == .note ? "NOTE" : "MARKER"
        let icon = entry.kind == .todo ? (entry.todo?.completed == true ? "checkmark.square" : "square") : entry.kind == .note ? "text.bubble" : "bookmark"
        return VStack(alignment: .leading, spacing: 6) {
            Label(label, systemImage: icon).font(.system(size: 9, weight: .medium)).foregroundStyle(.secondary)
            Text(annotation.text.isEmpty ? (entry.kind == .todo ? "Untitled to-do" : entry.kind == .note ? "Empty note" : "Marker") : annotation.text)
                .font(.system(size: 12, weight: .medium)).lineLimit(3).strikethrough(entry.todo?.completed == true)
        }.padding(.vertical, 8).padding(.leading, 21 + CGFloat(min(entry.depth, 4)) * 10)
        .contextMenu {
            if let item = entry.todo {
                Button(item.completed ? "Reopen To-Do" : "Complete To-Do") {
                    do { edit = try TodoListEdit.toggle(source: document.text, expectedSource: snapshot, item: item, format: document.format) }
                    catch { navigatorError = error.localizedDescription }
                }
            }
            Button("Show in Manuscript") { jump = EditorJump(line: entry.line, range: annotation.range) }
        }
    }
    private var navigatorSidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 5) {
                Text("Fountage").font(.system(size: 27, weight: .regular, design: .serif))
                Text("by Baz Studios").font(.caption).foregroundStyle(.secondary)
            }.padding(.horizontal, StudioLayout.sidebarInset).padding(.top, 25).padding(.bottom, 28)
            Picker("Navigator", selection: $navigator) {
                    Text("Outline").tag(NavigatorSection.outline)
                    if document.format == .screenplay { Text("Notes").tag(NavigatorSection.notes) }
                    Text("Markers").tag(NavigatorSection.markers)
                }.pickerStyle(.segmented).labelsHidden()
                    .padding(.horizontal, StudioLayout.sidebarInset).padding(.bottom, 18)
            navigatorHeader
            if navigator != .outline {
                noteNavigator
            } else if outline.headings.isEmpty && outline.markers.isEmpty && outline.notes.isEmpty && outline.todos.isEmpty {
                ContentUnavailableView("Room for an idea", systemImage: "text.book.closed", description: Text(document.format.structureHint))
                    .controlSize(.small)
            } else {
                List(selection: $selection) {
                    ForEach(navigatorEntries) { entry in
                        if let row = entry.row {
                        OutlineDraggableRow(line: row.heading.line,
                            canDrag: headingQuery.isEmpty && ![ElementKind.synopsis, .pageBreak].contains(row.heading.kind),
                            source: document.text, format: document.format, store: outlineDrag, enabled: headingQuery.isEmpty,
                            start: { outlineDrag.begin($0) }, perform: { session, token, after in
                                guard headingQuery.isEmpty else { return }
                                edit = outlineDrag.resolve(session, token: token, source: document.text, format: document.format,
                                                           target: row.heading.line, after: after)
                                outlineDrag.cancel()
                            }) { outlineRow(row) }
                            .tag(entry.id)
                        } else {
                            annotationRow(entry).tag(entry.id)
                        }
                    }
                }.listStyle(.sidebar)
                    .onKeyPress(.rightArrow) { foldSelected(expand: true) }
                    .onKeyPress(.leftArrow) { foldSelected(expand: false) }
                    .onKeyPress(.return) {
                        guard let entry = navigatorEntries.first(where: { $0.id == selection }) else { return .ignored }
                        jump = EditorJump(line: entry.line, range: entry.annotation?.range)
                        return .handled
                    }
                    .searchable(text: $headingQuery, placement: .sidebar, prompt: "Find an outline item")
                    .overlay { navigatorEmptyState }
            }
            Spacer(minLength: 0)
            HStack {
                Image(systemName: "doc.plaintext").foregroundStyle(accent)
                Text(isProject ? "Fountage project" : "Plain text document").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button(action: exportVisibleNavigator) { Image(systemName: "square.and.arrow.up") }
                    .buttonStyle(.borderless)
                    .accessibilityLabel(navigator == .markers ? "Export Visible Markers" : navigator == .notes ? "Export Visible Notes" : "Export Visible Outline")
                    .help("Create a new project from visible Navigator entries (⌥⌘E)")
                    .keyboardShortcut("e", modifiers: [.command, .option])
                    .disabled(navigator != .outline ? notes.isEmpty : navigatorEntries.isEmpty)
            }.padding(.horizontal, StudioLayout.sidebarInset).padding(.vertical, 20)
        }.navigationSplitViewColumnWidth(min: 200, ideal: 235, max: 340)
    }
    private var documentFormatMenu: some View {
        Menu {
            Picker("Document Type", selection: manuscriptFormat) {
                ForEach(DocumentFormat.allCases) { Text($0.name).tag($0) }
            }.disabled(!isProject)
            Picker("Layout", selection: $document.preset) {
                ForEach(document.format.presets) { Text($0.name).tag($0) }
            }.disabled(!isProject)
            if document.format == .academic { academicStyleMenu }
            Divider()
            if document.format == .screenplay {
                Button("Title Page…") { showingTitlePage = true }
            }
            Button("Insert Starter") { manuscriptText.wrappedValue = document.format.starter(for: document.preset) }
                .disabled(!document.text.isEmpty)
            if !isProject { Text("Save as a Fountage project to keep format settings.") }
        } label: { Label("Document Format", systemImage: "doc.text") }.help("Document type and layout").accessibilityLabel("Document Format")
    }
    private var manuscriptFormat: Binding<DocumentFormat> {
        Binding(get: { document.format }, set: { format in
            do {
                var updated = document
                if try MilestoneWorkflow.changeFormat(format, in: &updated, cache: analysisCache) { document = updated }
            } catch { formatError = error.localizedDescription }
        })
    }
    private var manuscriptText: Binding<String> {
        Binding(get: { document.text }, set: { source in
            var updated = document
            MilestoneWorkflow.replaceText(source, in: &updated, cache: analysisCache)
            document = updated
        })
    }
    private var manuscriptPane: some View {
        VStack(spacing: 0) {
            HStack(alignment: .center) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("THE MANUSCRIPT").font(.system(size: 9, weight: .medium)).tracking(2).foregroundStyle(.secondary)
                    Text(title).font(.system(size: 26, weight: .regular, design: .serif)).lineLimit(1)
                }
                Spacer()
                Text(document.format.name.uppercased()).font(.system(size: 9, weight: .medium)).tracking(1.3).foregroundStyle(.tertiary)
            }.padding(.horizontal, StudioLayout.manuscriptInset).padding(.top, 25).padding(.bottom, 22)
            NativeEditor(text: manuscriptText, jump: jump, edit: edit, fontSize: fontSize, format: document.format, preset: document.preset, paper: paper, typewriterScrolling: typewriterScrolling, analysisCache: analysisCache, onTitlePage: { showingTitlePage = true }, onSelectedWords: { selectedWords = $0 }, onShelfCapture: isProject && document.canEditShelf ? { source, range, cut in try captureToShelf(source: source, range: range, cut: cut) } : nil, onSourceSelection: { source, range in editorSelectionSource = source; editorSelection = range }) { line in cursorLine = line }
                .background(paper == .white ? Color.white : Color(nsColor: .textBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 4))
                .overlay(RoundedRectangle(cornerRadius: 4).stroke(Color.primary.opacity(0.07), lineWidth: 1))
                .shadow(color: .black.opacity(0.04), radius: 12, y: 5)
                .padding(.horizontal, StudioLayout.manuscriptInset).padding(.bottom, 20)
                .overlay {
                    if document.text.isEmpty && !typewriterScrolling {
                        VStack(spacing: 12) {
                            Image(systemName: "pencil.line").font(.system(size: 27, weight: .ultraLight)).foregroundStyle(accent)
                            Text("Make room for an idea.").font(.system(size: 26, weight: .regular, design: .serif))
                            Text("Click anywhere on the page and write your first line.").font(.callout).foregroundStyle(.secondary)
                            Text(document.format.prompt).font(.system(size: 11, design: .monospaced)).foregroundStyle(.tertiary).padding(.top, 8)
                        }.allowsHitTesting(false)
                    }
                }
                .environment(\.colorScheme, paper == .white ? .light : colorScheme)
            HStack(spacing: 16) {
                Text(selectedWords.map { "\($0.formatted()) selected / \(outline.words.formatted()) words" } ?? "\(outline.words.formatted()) words")
                    .monospacedDigit()
                    .help(document.format == .screenplay ? "Draft counts exclude title metadata, outline sections, synopses, notes, omitted text and scene numbers. Selections use the same manuscript counts; a partially selected word counts once." : "Word counts include source markup. Select text to count a passage.")
                Text("\(outline.units) \(document.format.unit)\(outline.units == 1 ? "" : "s")").monospacedDigit()
                if document.format == .comic { Text("\(outline.panels) panels").monospacedDigit() }
                Spacer()
                Text("Line \(cursorLine + 1)").monospacedDigit()
            }.font(.system(size: 11)).foregroundStyle(.secondary).padding(.horizontal, StudioLayout.manuscriptInset).padding(.bottom, 16)
        }.background(Color(nsColor: .windowBackgroundColor))
    }
    private var screenplayPreview: some View {
        let options: ScreenplayPrintOptions? = isProject ? document.outputOptions() : nil
        let save: ((ScreenplayPrintOptions, ScreenplayPrintOptions) throws -> Void)? = isProject ? saveProjectOutputOptions : nil
        return ScreenplayPreview(source: document.text, title: title, options: options, saveOptions: save)
    }
    private func saveProjectOutputOptions(_ options: ScreenplayPrintOptions, _ previous: ScreenplayPrintOptions) throws {
        try outputSettings.change(options, previous: previous, document: $document, undoManager: undoManager)
    }
    private var navigatorHeader: some View {
        HStack {
            Text(navigator == .markers ? "MARKERS" : navigator == .notes ? "SCRIPT NOTES" : "MANUSCRIPT").font(.system(size: 10, weight: .semibold)).tracking(1.5).foregroundStyle(.secondary)
            Spacer()
            Text(navigatorCount).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            if navigator == .outline {
                OutlineFilterMenu(kinds: NavigatorFilters.kinds(for: document.format), format: document.format,
                    hidden: document.navigatorHiddenKinds, change: changeNavigatorFilters)
                    .frame(width: 20, height: 20)
            }
            if navigator == .outline && !visibleExpandableKeys.isEmpty {
                Menu {
                    Button("Expand All") { collapsed.subtract(visibleExpandableKeys) }.disabled(collapsed.intersection(visibleExpandableKeys).isEmpty)
                    Button("Collapse All") { collapsed.formUnion(visibleExpandableKeys) }
                } label: { Image(systemName: "ellipsis.circle") }
                    .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                    .accessibilityLabel("Outline Options").help("Expand or collapse outline groups")
                    .disabled(!headingQuery.isEmpty)
            }
        }.padding(.horizontal, StudioLayout.sidebarInset).padding(.bottom, 10)
    }
    @ViewBuilder private var navigatorEmptyState: some View {
        if navigatorEntries.isEmpty {
            if headingQuery.isEmpty {
                ContentUnavailableView {
                    Label("Outline Filtered", systemImage: "line.3.horizontal.decrease.circle")
                } description: { Text("Show more entry types to see your outline.") }
                actions: { Button("Show All") { _ = changeNavigatorFilters(document.navigatorHiddenKinds.subtracting(NavigatorFilters.kinds(for: document.format))) } }
                .controlSize(.small)
            } else { ContentUnavailableView.search(text: headingQuery) }
        }
    }
    private func changeNavigatorFilters(_ hidden: Set<ElementKind>) -> Bool {
        do { try document.setNavigatorHiddenKinds(hidden); return true }
        catch { navigatorError = error.localizedDescription; return false }
    }
    private func exportVisibleNavigator() {
        do {
            let exported = try OutlineExport.make(from: document, collapsed: collapsed, query: headingQuery, notesOnly: navigator == .notes, markersOnly: navigator == .markers, hiddenKinds: document.navigatorHiddenKinds)
            newDocument(exported)
        } catch { navigatorError = error.localizedDescription }
    }
    private func moveOutline(_ row: OutlineIndex.Row, down: Bool) {
        guard headingQuery.isEmpty else { return }
        edit = outlineIndex.move(line: row.heading.line, down: down, source: document.text)
    }
    private func toggle(_ row: OutlineIndex.Row) {
        if collapsed.contains(row.key) { collapsed.remove(row.key) }
        else { collapsed.insert(row.key) }
    }
    private func foldSelected(expand: Bool) -> KeyPress.Result {
        guard headingQuery.isEmpty, let selection,
              let row = headings.first(where: { $0.heading.line == selection }), row.hasChildren else { return .ignored }
        if expand { collapsed.remove(row.key) } else { collapsed.insert(row.key) }
        return .handled
    }
    private func clearHiddenSelection() {
        if let selection, !navigatorEntries.contains(where: { $0.id == selection }) { self.selection = nil }
        if let noteSelection, !notes.contains(where: { $0.id == noteSelection }) { self.noteSelection = nil }
    }
    private var noteNavigator: some View {
        Group {
            if (navigator == .markers ? outline.markers : outline.notes).isEmpty {
                ContentUnavailableView(navigator == .markers ? "Find your place" : "A place for your notes", systemImage: navigator == .markers ? "bookmark" : "note.text", description: Text(navigator == .markers ? "Choose Tools → Markers to bookmark a place in your manuscript." : "Add [[a note]] anywhere in your screenplay to find it here."))
                    .controlSize(.small)
            } else {
                List(selection: $noteSelection) {
                    ForEach(notes) { note in
                        VStack(alignment: .leading, spacing: 7) {
                            Label("LINE \(note.line + 1)", systemImage: navigator == .markers ? "bookmark" : "text.bubble")
                                .font(.system(size: 9, weight: .medium)).foregroundStyle(.secondary)
                            Text(note.text.isEmpty ? (navigator == .markers ? "Marker" : "Empty note") : note.text)
                                .font(.system(size: 12)).lineLimit(4).lineSpacing(3)
                        }.padding(.vertical, 9).tag(note.id)
                    }
                }.listStyle(.sidebar)
                    .onKeyPress(.return) {
                        guard let note = notes.first(where: { $0.id == noteSelection }) else { return .ignored }
                        jump = EditorJump(line: note.line, range: note.range)
                        return .handled
                    }
                    .help(navigator == .markers ? "Use arrow keys to browse markers. Press Return to reveal the marker in the manuscript." : "Use arrow keys to browse notes. Press Return to edit the selected note.")
                    .searchable(text: $headingQuery, placement: .sidebar, prompt: navigator == .markers ? "Find a marker" : "Find a note")
                    .overlay { if notes.isEmpty { ContentUnavailableView.search(text: headingQuery) } }
            }
        }
    }
    private var academicStyleMenu: some View {
        Menu("Required Style: \(document.academicStyle?.name ?? "Not selected")") {
            Button("Not selected") { document.academicStyle = nil }
            ForEach(AcademicStyle.choices.indices, id: \.self) { index in
                let style = AcademicStyle.choices[index]
                Button { document.academicStyle = style } label: {
                    if document.academicStyle == style { Label(style.name, systemImage: "checkmark") }
                    else { Text(style.name) }
                }
            }
        }.disabled(!isProject)
    }
    private func captureToShelf(source: String, range: NSRange, cut: Bool) throws {
        var updated = document
        try ShelfCapture.store(in: &updated, expectedSource: source, selection: range, cut: cut, cache: analysisCache)
        document = updated
        undoManager?.setActionName(cut ? "Cut to Shelf" : "Copy to Shelf")
        if cut {
            let caret = NSRange(location: range.location, length: 0)
            shelfSource = document.text; shelfSelection = caret
            jump = EditorJump(line: 0, range: caret, focusEditor: !showingShelf)
        }
    }
    private var inspector: some View {
        VStack(spacing: 0) {
            if isProject {
                Picker("Inspector", selection: $inspectorSection) {
                    Text("Draft").tag("Draft"); Text("Shelf").tag("Shelf")
                }.pickerStyle(.segmented).labelsHidden().padding(14)
                Divider()
            }
            if isProject && inspectorSection == "Shelf" {
                ShelfInspector(document: $document, manage: {
                    shelfSource = editorSelectionSource; shelfSelection = editorSelection; showingShelf = true
                }, prepareEdit: {
                    shelfSource = editorSelectionSource; shelfSelection = editorSelection
                }, insert: { content in
                    edit = try ShelfInsertion.edit(source: document.text, expectedSource: shelfSource, selection: shelfSelection, content: content)
                })
            } else { draftInspector }
        }.background(.regularMaterial)
    }
    private var draftInspector: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 25) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("DRAFT AT A GLANCE").font(.system(size: 9, weight: .semibold)).tracking(1.3).foregroundStyle(.secondary)
                    Text(outline.words.formatted()).font(.system(size: 43, weight: .light, design: .serif)).monospacedDigit()
                    Text("words in your draft").font(.caption).foregroundStyle(.secondary)
                }
                if isProject {
                    VStack(alignment: .leading, spacing: 16) {
                        Button { showingMilestones = true } label: {
                            Label("Writing Milestones…", systemImage: "flag.checkered")
                        }.buttonStyle(.link).font(.caption)
                        DraftGoalView(target: $document.draftWordGoal, words: outline.words)
                        DraftGoalView(target: $document.draftCharacterGoal, words: outline.character_count, unit: "characters")
                        if document.format == .screenplay {
                            DraftPageGoalView(target: $document.draftPageGoal, source: document.text, options: document.outputOptions())
                        }
                    }
                }
                if isProject {
                    Button {
                        shelfSource = editorSelectionSource; shelfSelection = editorSelection; showingShelf = true
                    } label: { Label("Shelf…", systemImage: "tray") }.buttonStyle(.link).font(.caption)
                }
                Button { showingTodos = true } label: { Label("To-Do List…", systemImage: "checklist") }.buttonStyle(.link).font(.caption)
                    .sheet(isPresented: $showingTodos) {
                        TodoListPanel(text: manuscriptText, format: document.format) { item in jump = EditorJump(line: item.line, range: item.range) }
                    }
                Divider()
                if document.format.hasCast { cast }
                else {
                    VStack(alignment: .leading, spacing: 12) {
                        Text(document.format.name).font(.system(size: 20, design: .serif))
                        Text(document.preset.name).font(.callout).foregroundStyle(.secondary)
                        if document.format == .academic {
                            Text("Required style").font(.caption).foregroundStyle(.secondary)
                            Text(document.academicStyle?.name ?? "Not selected").font(.callout)
                            Text("Style and edition are saved with this project. Citation formatting and submission layout are not yet available.")
                                .font(.caption).foregroundStyle(.secondary)
                        } else {
                            Text(document.format.structureHint).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                if isProject {
                    Button("Manage Cast & Locations…") { showingReferences = true }
                        .buttonStyle(.link).font(.caption)
                        .sheet(isPresented: $showingReferences) {
                            ReferenceLibrary(document: $document, analysis: outline)
                        }
                }
                if document.format == .screenplay {
                    Divider()
                    locations
                }
                Divider()
                VStack(alignment: .leading, spacing: 9) {
                    Image(systemName: "leaf").font(.system(size: 19, weight: .light)).foregroundStyle(accent)
                    Text("One line at a time.").font(.system(size: 19, design: .serif))
                    Text("The rest of the world can wait.").font(.caption).foregroundStyle(.secondary)
                }
            }.padding(23)
        }.background(.regularMaterial)
    }
    private var locations: some View {
        VStack(alignment: .leading, spacing: 15) {
            HStack {
                Text("Locations").font(.system(size: 20, design: .serif))
                Spacer()
                Text("\(outline.locations.count)").font(.caption).foregroundStyle(.secondary)
            }
            if outline.locations.isEmpty {
                Text("Locations will appear as you add scene headings.").font(.callout).foregroundStyle(.secondary)
            }
            ForEach(outline.locations.keys.sorted(), id: \.self) { name in
                Menu {
                    ForEach(outline.locations[name] ?? [], id: \.self) { line in
                        Button(outline.headings.first(where: { $0.line == line })?.heading ?? name) {
                            jump = EditorJump(line: line, focusEditor: true)
                        }
                    }
                } label: {
                    HStack {
                        Image(systemName: "mappin.and.ellipse").foregroundStyle(accent)
                        Text("\(name) · \(outline.locations[name]?.count ?? 0)").font(.system(size: 11, weight: .medium)).lineLimit(2)
                        Spacer()
                    }
                }
                .menuStyle(.borderlessButton)
                .help("Jump to a scene at this location")
            }
            if !outline.locations.isEmpty {
                Text("Scenes per location · choose a location to navigate").font(.system(size: 10)).foregroundStyle(.secondary)
            }
        }
    }
    private var cast: some View {
        VStack(alignment: .leading, spacing: 15) {
            HStack { Text("Characters").font(.system(size: 20, design: .serif)); Spacer(); Text("\(outline.characters.count)").font(.caption).foregroundStyle(.secondary) }
            if outline.characters.isEmpty { Text("Your cast will appear as you add dialogue.").font(.callout).foregroundStyle(.secondary) }
            ForEach(outline.characters.keys.sorted(), id: \.self) { name in
                HStack(spacing: 10) {
                    Text(String(name.prefix(1))).font(.system(size: 11, weight: .medium)).foregroundStyle(accent)
                        .frame(width: 27, height: 27).background(accent.opacity(0.09), in: Circle())
                    Text(name).font(.system(size: 11, weight: .medium)).lineLimit(2)
                    Spacer()
                    Text("\(outline.characters[name] ?? 0)").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }.accessibilityElement(children: .combine)
            }
            if !outline.characters.isEmpty { Text("Dialogue cues per character").font(.system(size: 10)).foregroundStyle(.tertiary) }
        }
    }
    private func headingLabel(_ heading: ManuscriptHeading) -> String {
        switch heading.kind {
        case .act: "ACT"
        case .panel: "PANEL"
        case .verse: "PART"
        case .heading, .section: "SECTION"
        case .synopsis: "SYNOPSIS"
        case .pageBreak: "PAGE BREAK"
        default: document.format.unit.uppercased()
        }
    }
    private func analyze() {
        outline = analysisCache.analysis(for: document.text, format: document.format)
        let updated = OutlineIndex(outline.headings)
        // Body edits can shift source lines without changing a group's identity. Structural
        // edits reopen groups so duplicated or moved headings cannot inherit a stale fold.
        if updated.keys != outlineIndex.keys { collapsed.removeAll() }
        outlineIndex = updated
    }
    private func setFocus(_ value: Bool) {
        guard value != focus else { return }
        withAnimation(.easeInOut(duration: 0.2)) {
            if value {
                layoutBeforeFocus = (columns, showInspector)
                columns = .detailOnly
                showInspector = false
            } else if let saved = layoutBeforeFocus {
                columns = saved.columns
                showInspector = saved.inspector
                layoutBeforeFocus = nil
            }
            focus = value
        }
    }
}
private extension String { var nonEmpty: String? { isEmpty ? nil : self } }
