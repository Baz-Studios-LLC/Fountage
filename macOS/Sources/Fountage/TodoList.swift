import SwiftUI
import AppKit

struct ManuscriptTodo: Decodable, Identifiable {
    let line: Int
    let start: Int
    let length: Int
    let state_start: Int
    let completed: Bool
    let text: String
    var id: Int { start }
    var range: NSRange { NSRange(location: start, length: length) }
}

enum TodoListEdit {
    struct EditError: LocalizedError {
        var message = "The manuscript changed while this checklist action was being prepared. No text was changed. Try again with the updated list."
        var errorDescription: String? { message }
    }
    static func toggle(source: String, expectedSource: String, item: ManuscriptTodo, format: DocumentFormat) throws -> ManuscriptEdit {
        guard source.utf8.elementsEqual(expectedSource.utf8), !source.utf8.contains(0),
              let current = ManuscriptAnalysis.parse(source, format: format).todos.first(where: { $0.id == item.id }),
              current.state_start == item.state_start, current.completed == item.completed,
              current.text.utf8.elementsEqual(item.text.utf8), current.length == item.length else { throw EditError() }
        return ManuscriptEdit(source: source, range: NSRange(location: item.state_start, length: 1), replacement: item.completed ? " " : "x",
            selection: item.range, actionName: item.completed ? "Reopen To-Do" : "Complete To-Do")
    }
    static func append(source: String, expectedSource: String, taskText: String, format: DocumentFormat, separateParagraphs: Bool = true) throws -> ManuscriptEdit {
        guard source.utf8.elementsEqual(expectedSource.utf8), !source.utf8.contains(0) else { throw EditError() }
        let label = taskText.trimmingCharacters(in: .whitespaces)
        guard !label.isEmpty, !label.contains(where: { $0.isNewline }), !label.utf8.contains(0) else {
            throw EditError(message: "Enter a task on one line. No text was changed.")
        }
        let ending = source.first(where: { $0.isNewline }) == "\r\n" ? "\r\n" : "\n"
        let separator = source.isEmpty ? "" : (separateParagraphs ? (source.hasSuffix(ending + ending) ? "" : (source.hasSuffix(ending) ? ending : ending + ending)) : (source.utf8.last == 10 ? "" : ending))
        let insertion = separator + "[ ] " + label + ending
        let start = source.utf16.count + separator.utf16.count
        let candidate = source + insertion
        guard let added = ManuscriptAnalysis.parse(candidate, format: format).todos.first(where: { $0.start == start }),
              added.text.utf8.elementsEqual(label.utf8) else {
            throw EditError(message: "The task would be hidden by manuscript markup. Close any unfinished omitted passage, and enter the task without note or omission delimiters. No text was changed.")
        }
        return ManuscriptEdit(source: source, range: NSRange(location: source.utf16.count, length: 0), replacement: insertion,
            selection: added.range, actionName: "Add To-Do")
    }
    static func taskContent(source: String, item: ManuscriptTodo) -> String {
        let ns = source as NSString
        var start = item.state_start + 2
        guard start >= 0, NSMaxRange(item.range) <= ns.length, start <= NSMaxRange(item.range) else { return "" }
        while start < NSMaxRange(item.range) && [9, 32].contains(Int(ns.character(at: start))) { start += 1 }
        return ns.substring(with: NSRange(location: start, length: NSMaxRange(item.range) - start))
    }
    static func update(source: String, expectedSource: String, item: ManuscriptTodo, content: String, format: DocumentFormat) throws -> ManuscriptEdit? {
        _ = try toggle(source: source, expectedSource: expectedSource, item: item, format: format)
        try checkAnnotationBoundary(source: source, line: item.range)
        guard !content.contains(where: { $0.isNewline }), !content.utf8.contains(0) else {
            throw EditError(message: "Keep the task on one line. No text was changed.")
        }
        let old = taskContent(source: source, item: item)
        guard !old.utf8.elementsEqual(content.utf8) else { return nil }
        let range = NSRange(location: NSMaxRange(item.range) - old.utf16.count, length: old.utf16.count)
        // A prefix-only task needs its first separator before new content.
        let replacement = (range.location == item.state_start + 2 && !content.isEmpty ? " " : "") + content
        let candidate = (source as NSString).replacingCharacters(in: range, with: replacement)
        let analysis = ManuscriptAnalysis.parse(candidate, format: format)
        guard let updated = analysis.todos.first(where: { $0.start == item.start }), updated.completed == item.completed else { throw EditError() }
        try checkAnnotationBoundary(source: candidate, line: updated.range)
        return ManuscriptEdit(source: source, range: range, replacement: replacement, selection: updated.range, actionName: "Edit To-Do")
    }
    private static func checkAnnotationBoundary(source: String, line: NSRange) throws {
        let annotations = ManuscriptAnalysis.parse(source, format: .screenplay)
        let spans = annotations.notes.map(\.range) + annotations.omissions.map(\.range) + annotations.markers.map(\.range)
        guard !spans.contains(where: { NSIntersectionRange($0, line).length > 0 && ($0.location < line.location || NSMaxRange($0) > NSMaxRange(line)) }) else {
            throw EditError(message: "This task shares a note or omitted passage with another line. Edit it in the manuscript so that the surrounding text stays intact. No text was changed.")
        }
    }
    static func remove(source: String, expectedSource: String, item: ManuscriptTodo, format: DocumentFormat) throws -> ManuscriptEdit {
        // Reuse the same identity/state validation as completion actions.
        _ = try toggle(source: source, expectedSource: expectedSource, item: item, format: format)
        let line = item.range
        try checkAnnotationBoundary(source: source, line: line)
        let ns = source as NSString
        var end = NSMaxRange(line)
        if end < ns.length && ns.character(at: end) == 13 { end += 1 }
        if end < ns.length && ns.character(at: end) == 10 { end += 1 }
        return ManuscriptEdit(source: source, range: NSRange(location: line.location, length: end - line.location), replacement: "",
            selection: NSRange(location: line.location, length: 0), actionName: "Remove To-Do")
    }
    static func newline(source: String, selection: NSRange, format: DocumentFormat) -> ManuscriptEdit? {
        let ns = source as NSString
        guard !source.utf8.contains(0), selection.length == 0, selection.location >= 0, selection.location <= ns.length else { return nil }
        let at = selection.location
        if at > 0 && at < ns.length && (0xD800...0xDBFF).contains(ns.character(at: at - 1)) && (0xDC00...0xDFFF).contains(ns.character(at: at)) { return nil }
        let analysis = ManuscriptAnalysis.parse(source, format: format)
        let hidden = analysis.notes.map(\.range) + analysis.omissions.map(\.range) + analysis.markers.map(\.range)
        guard !hidden.contains(where: { at >= $0.location && at < NSMaxRange($0) }),
              let task = analysis.todos.first(where: { at >= $0.state_start + 2 && at <= NSMaxRange($0.range) }) else { return nil }
        let line = ns.substring(with: task.range)
        let indent = String(line.prefix { $0 == " " || $0 == "\t" })
        // Prefer the current physical line ending, including mixed-ending documents.
        let end = NSMaxRange(task.range)
        let ending: String
        if end < ns.length && ns.character(at: end) == 13 && end + 1 < ns.length && ns.character(at: end + 1) == 10 { ending = "\r\n" }
        else if end < ns.length && ns.character(at: end) == 10 { ending = "\n" }
        else { ending = source.first(where: { $0.isNewline }) == "\r\n" ? "\r\n" : "\n" }
        let content = String(line.dropFirst(indent.count + 3))
        if content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return ManuscriptEdit(source: source, range: task.range, replacement: ending,
                selection: NSRange(location: task.start + ending.utf16.count, length: 0), actionName: "End To-Do List")
        }
        let insertion = ending + indent + "[ ] "
        return ManuscriptEdit(source: source, range: selection, replacement: insertion,
            selection: NSRange(location: at + insertion.utf16.count, length: 0), actionName: "Continue To-Do List")
    }
    static func convert(source: String, selection: NSRange) -> ManuscriptEdit? {
        let ns = source as NSString
        guard !source.utf8.contains(0), selection.location >= 0, selection.location <= ns.length,
              selection.length >= 0, selection.length <= ns.length - selection.location else { return nil }
        for at in [selection.location, NSMaxRange(selection)] where at > 0 && at < ns.length {
            if (0xD800...0xDBFF).contains(ns.character(at: at - 1)) && (0xDC00...0xDFFF).contains(ns.character(at: at)) { return nil }
        }
        let first = ns.lineRange(for: NSRange(location: selection.location, length: 0))
        let last = ns.lineRange(for: NSRange(location: selection.length > 0 ? NSMaxRange(selection) - 1 : selection.location, length: 0))
        let range = NSRange(location: first.location, length: NSMaxRange(last) - first.location)
        let original = ns.substring(with: range)
        let replacement = original.components(separatedBy: "\n").map { line -> String in
            let indent = String(line.prefix { $0 == " " || $0 == "\t" })
            let body = String(line.dropFirst(indent.count))
            if body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return line }
            if ["[ ]", "[x]", "[X]"].contains(where: { body == $0 || body.hasPrefix($0 + " ") || body.hasPrefix($0 + "\t") || body == $0 + "\r" }) { return line }
            return indent + "[ ] " + body
        }.joined(separator: "\n")
        guard !original.utf8.elementsEqual(replacement.utf8) else { return nil }
        return ManuscriptEdit(source: source, range: range, replacement: replacement,
            selection: NSRange(location: range.location, length: replacement.utf16.count), actionName: "Convert to To-Do List")
    }
}

struct TodoListPanel: View {
    @Binding var text: String
    let format: DocumentFormat
    var undoHint = "Changes can be undone after closing this window."
    var locationName = "manuscript"
    let reveal: ((ManuscriptTodo) -> Void)?
    @Environment(\.dismiss) private var dismiss
    @State private var filter = "All"
    @State private var query = ""
    @State private var error: String?
    @State private var newTask = ""
    @State private var pendingRemoval: ManuscriptTodo?
    @State private var removalSnapshot = ""
    @State private var editing: ManuscriptTodo?
    @State private var editingSnapshot = ""
    var body: some View {
        let snapshot = text
        let items = ManuscriptAnalysis.parse(snapshot, format: format).todos
        let visible = items.filter { (filter == "All" || $0.completed == (filter == "Completed")) && (query.isEmpty || $0.text.localizedCaseInsensitiveContains(query)) }
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("To-Do List").font(.title2.weight(.semibold))
                Spacer()
                Text("\(items.filter { !$0.completed }.count) open").foregroundStyle(.secondary)
            }
            Text("Keep your next steps in the \(locationName). Add a task here, or use the pencil to edit its text.").foregroundStyle(.secondary)
            HStack {
                TextField("New to-do", text: $newTask).textFieldStyle(.roundedBorder)
                    .onSubmit { add(snapshot: text) }
                Button("Add") { add(snapshot: text) }
                    .disabled(newTask.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            Picker("Show", selection: $filter) {
                ForEach(["All", "Open", "Completed"], id: \.self) { Text($0).tag($0) }
            }.pickerStyle(.segmented)
            TextField("Find a to-do", text: $query).textFieldStyle(.roundedBorder)
            List(visible) { item in
                HStack(alignment: .top, spacing: 12) {
                    Toggle(isOn: Binding(get: { item.completed }, set: { completed in
                        guard completed != item.completed else { return }
                        do {
                            let edit = try TodoListEdit.toggle(source: text, expectedSource: snapshot, item: item, format: format)
                            text = (text as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
                        } catch { self.error = error.localizedDescription }
                    })) { Text(item.text.isEmpty ? "Untitled to-do" : item.text).strikethrough(item.completed).foregroundStyle(item.completed ? .secondary : .primary) }
                    .toggleStyle(.checkbox)
                    Spacer(minLength: 0)
                    Button { editingSnapshot = snapshot; editing = item } label: { Image(systemName: "pencil") }
                        .buttonStyle(.borderless).accessibilityLabel("Edit to-do on line \(item.line + 1)")
                        .help("Edit task text")
                    if let reveal {
                        Button { reveal(item); dismiss() } label: { Image(systemName: "arrow.up.forward") }
                            .buttonStyle(.borderless).accessibilityLabel("Reveal to-do on line \(item.line + 1)")
                    }
                    Button { removalSnapshot = snapshot; pendingRemoval = item } label: { Image(systemName: "trash") }
                        .buttonStyle(.borderless).accessibilityLabel("Remove to-do on line \(item.line + 1)")
                        .help("Remove this task from the manuscript")
                }.padding(.vertical, 5)
                .contextMenu {
                    Button("Edit To-Do…") { editingSnapshot = snapshot; editing = item }
                    if let reveal { Button("Show in Manuscript") { reveal(item); dismiss() } }
                    Button("Remove To-Do…", role: .destructive) { removalSnapshot = snapshot; pendingRemoval = item }
                }
            }.overlay { if visible.isEmpty { ContentUnavailableView(items.isEmpty ? "Room for a next step" : "No matching to-dos", systemImage: "checklist", description: Text(items.isEmpty ? "Write [ ] followed by a task, or use Format → Convert Lines To → To Do List." : "Choose another filter or search.")) } }
            HStack { Text(undoHint).font(.caption).foregroundStyle(.secondary); Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.cancelAction) }
        }.padding(24).frame(width: 570, height: 550)
        .sheet(item: $editing) { item in
            TodoTextEditor(content: TodoListEdit.taskContent(source: editingSnapshot, item: item)) { content in
                if let edit = try TodoListEdit.update(source: text, expectedSource: editingSnapshot, item: item, content: content, format: format) {
                    text = (text as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
                }
            }
        }
        .confirmationDialog("Remove this to-do from the \(locationName)?", isPresented: Binding(get: { pendingRemoval != nil }, set: { if !$0 { pendingRemoval = nil } }), titleVisibility: .visible) {
            Button("Remove To-Do", role: .destructive) {
                guard let item = pendingRemoval else { return }
                do {
                    let edit = try TodoListEdit.remove(source: text, expectedSource: removalSnapshot, item: item, format: format)
                    text = (text as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
                } catch { self.error = error.localizedDescription }
                pendingRemoval = nil
            }
            Button("Cancel", role: .cancel) { pendingRemoval = nil }
        } message: { Text("The entire task line, including any inline notes, will be removed. \(undoHint)") }
        .alert("To-Do Not Changed", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button("OK") { error = nil } } message: { Text(error ?? "") }
    }
    private func add(snapshot: String) {
        do {
            let edit = try TodoListEdit.append(source: text, expectedSource: snapshot, taskText: newTask, format: format)
            text = (text as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
            newTask = ""; query = ""; filter = "All"
        } catch { self.error = error.localizedDescription }
    }
}

private struct TodoTextEditor: View {
    @State var content: String
    let save: (String) throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var error: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Edit To-Do").font(.title2.weight(.semibold))
            Text("Edit the task’s source text. Inline notes and formatting appear as markup; keep their delimiters to retain them.").foregroundStyle(.secondary)
            TextField("Task text", text: $content).textFieldStyle(.roundedBorder).onSubmit(commit)
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Save", action: commit).keyboardShortcut(.defaultAction)
            }
        }.padding(24).frame(width: 480)
        .alert("To-Do Not Changed", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK") { error = nil }
        } message: { Text(error ?? "") }
    }
    private func commit() {
        do { try save(content); dismiss() }
        catch { self.error = error.localizedDescription }
    }
}

extension ManuscriptTextView {
    @objc func convertToTodoList(_ sender: Any?) {
        guard isEditable, !hasMarkedText(), selectedRanges.count == 1 else { return }
        TodoListEdit.convert(source: string, selection: selectedRange())?.apply(to: self)
    }
}
