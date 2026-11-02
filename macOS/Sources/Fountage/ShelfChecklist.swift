import SwiftUI

enum ShelfChecklistEdit {
    private static func validate(_ expected: ShelfItem, in document: ScreenplayDocument) throws {
        guard document.canEditShelf else { throw ShelfError.unsupported }
        guard expected.kind == .checklist, let current = document.shelfItems.first(where: { $0.id == expected.id }), current.matches(expected) else { throw ShelfError.changed }
    }
    @discardableResult static func toggle(in document: inout ScreenplayDocument, item: ShelfItem, task: ManuscriptTodo, completed: Bool) throws -> Bool {
        try validate(item, in: document)
        let edit = try TodoListEdit.toggle(source: item.content, expectedSource: item.content, item: task, format: .general)
        guard completed != task.completed else { return false }
        return try apply(edit, item: item, to: &document)
    }
    @discardableResult static func append(in document: inout ScreenplayDocument, item: ShelfItem, text: String) throws -> Bool {
        try validate(item, in: document)
        let edit = try TodoListEdit.append(source: item.content, expectedSource: item.content, taskText: text, format: .general, separateParagraphs: false)
        return try apply(edit, item: item, to: &document)
    }
    private static func apply(_ edit: ManuscriptEdit, item: ShelfItem, to document: inout ScreenplayDocument) throws -> Bool {
        var changed = item
        changed.content = (item.content as NSString).replacingCharacters(in: edit.range, with: edit.replacement)
        return try document.saveShelfItem(changed, replacing: item)
    }
}

struct ShelfChecklistControls: View {
    @Binding var document: ScreenplayDocument
    let item: ShelfItem
    @Environment(\.undoManager) private var undoManager
    @State private var draft = ""
    @State private var error: String?
    var body: some View {
        let tasks = ManuscriptAnalysis.parse(item.content, format: .general).todos
        VStack(alignment: .leading, spacing: 8) {
            Text("\(tasks.filter { !$0.completed }.count) open · \(tasks.count) total").font(.caption2).foregroundStyle(.secondary)
            ForEach(tasks) { task in
                Toggle(isOn: Binding(get: { task.completed }, set: { completed in
                    change(name: completed ? "Complete Shelf Task" : "Reopen Shelf Task") { try ShelfChecklistEdit.toggle(in: &$0, item: item, task: task, completed: completed) }
                })) {
                    Text(task.text.isEmpty ? "Untitled task" : task.text).font(.caption).strikethrough(task.completed)
                        .foregroundStyle(task.completed ? .secondary : .primary)
                }.toggleStyle(.checkbox)
            }
            HStack(spacing: 6) {
                TextField("New task", text: $draft).textFieldStyle(.roundedBorder).controlSize(.small)
                    .accessibilityLabel("New task in \(item.title)").onSubmit(add)
                Button(action: add) { Image(systemName: "plus") }.buttonStyle(.borderless)
                    .accessibilityLabel("Add task to \(item.title)")
                    .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .disabled(!document.canEditShelf)
        .alert("Shelf Checklist Not Changed", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK") { error = nil }
        } message: { Text(error ?? "") }
    }
    private func add() {
        if change(name: "Add Shelf Task", { try ShelfChecklistEdit.append(in: &$0, item: item, text: draft) }) { draft = "" }
    }
    @discardableResult private func change(name: String, _ operation: (inout ScreenplayDocument) throws -> Bool) -> Bool {
        do {
            var updated = document
            if try operation(&updated) { document = updated; undoManager?.setActionName(name); return true }
        } catch { self.error = error.localizedDescription }
        return false
    }
}
