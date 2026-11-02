import SwiftUI
import AppKit

/// The draft belongs to the document, so native save, versions and Undo retain it.
struct QuickNoteEntry: View {
    @Binding var document: ScreenplayDocument
    @State private var error: String?
    @State private var addedID: String?
    @FocusState private var focused: Bool
    private var added: Bool { document.quickNoteText.isEmpty && addedID.map { id in document.shelfItems.contains { $0.id == id } } == true }
    @Environment(\.undoManager) private var undoManager
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Quick Note", systemImage: "square.and.pencil").font(.headline)
            TextEditor(text: Binding(get: { document.quickNoteText }, set: { text in
                do {
                    var updated = document
                    if try updated.setQuickNoteText(text, replacing: document.quickNoteText) { document = updated; addedID = nil }
                } catch { self.error = error.localizedDescription }
            }))
            .focused($focused)
            .font(.body).scrollContentBackground(.hidden).frame(height: 76)
            .padding(6).background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.secondary.opacity(0.2)))
            .accessibilityLabel("Quick note draft")
            HStack {
                Text(added ? (document.shelfTypeVisible(.note) ? "Note added." : "Note added. Notes are hidden.") : "Draft stays with this project.")
                    .font(.caption).foregroundStyle(.secondary)
                Spacer(minLength: 4)
                Button("Add Note") {
                    do {
                        var updated = document
                        let composing = (NSApp.keyWindow?.firstResponder as? NSTextView)?.hasMarkedText() ?? false
                        if let id = try updated.addQuickNote(replacing: document.quickNoteText, composing: composing) {
                            document = updated; addedID = id; undoManager?.setActionName("Add Quick Note")
                        }
                    } catch { self.error = error.localizedDescription }
                }.keyboardShortcut(focused ? KeyboardShortcut(.return, modifiers: .command) : nil)
                .disabled(document.quickNoteText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }.padding(12).background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
        .alert("Quick Note Not Changed", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK") { error = nil }
        } message: { Text(error ?? "") }
    }
}
