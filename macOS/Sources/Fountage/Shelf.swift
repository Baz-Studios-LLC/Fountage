import SwiftUI
import AppKit

struct ShelfItem: Identifiable, Codable {
    enum Kind: String, Codable, CaseIterable { case note, snippet, checklist
        var title: String { switch self { case .note: "Note"; case .snippet: "Snippet"; case .checklist: "Checklist" } }
        var icon: String { switch self { case .note: "note.text"; case .snippet: "text.quote"; case .checklist: "checklist" } }
    }
    var id = UUID().uuidString
    var kind: Kind
    var title: String
    var content = ""
    var pinned = false
    func matches(_ other: ShelfItem) -> Bool {
        id.utf8.elementsEqual(other.id.utf8) && kind == other.kind && title.utf8.elementsEqual(other.title.utf8) && content.utf8.elementsEqual(other.content.utf8) && pinned == other.pinned
    }
}

enum ShelfError: LocalizedError {
    case unsupported, changed, invalid, selection, composing
    var errorDescription: String? { switch self {
        case .unsupported: "This project’s Shelf uses an unsupported format. Its data has been preserved and editing is unavailable."
        case .changed: "This Shelf item changed while the action was being prepared. No changes were saved. Reopen it to review the current version."
        case .invalid: "Enter a title, and remove any null characters before saving."
        case .composing: "Finish composing text before adding the note."
        case .selection: "The manuscript or selection changed. Select the manuscript text again and retry the Shelf action."
    } }
}

extension ScreenplayDocument {
    private var shelfRecord: [String: Any]? { (metadata[Self.namespace] as? [String: Any])?["shelf"] as? [String: Any] }
    var canEditShelf: Bool {
        guard metadata[Self.namespace] == nil || metadata[Self.namespace] is [String: Any] else { return false }
        guard let value = (metadata[Self.namespace] as? [String: Any])?["shelf"] else { return true }
        guard let record = value as? [String: Any], let version = record["version"],
              let data = try? JSONSerialization.data(withJSONObject: [version]),
              (try? JSONDecoder().decode([Int].self, from: data)) == [1],
              record["items"] is [String: Any], let order = record["order"] as? [String], Set(order).count == order.count else { return false }
        if let value = record["pinnedOrder"] {
            guard let order = value as? [String], Set(order).count == order.count else { return false }
        }
        if let value = record["visibility"] {
            guard let visibility = value as? [String: Any] else { return false }
            for kind in ShelfItem.Kind.allCases {
                if let value = visibility[kind.rawValue], Self.shelfBoolean(value) == nil { return false }
            }
        }
        if let value = record["quickEntry"] {
            guard let quick = value as? [String: Any] else { return false }
            if let text = quick["text"] { guard let text = text as? String, !text.utf8.contains(0) else { return false } }
            if let visible = quick["visible"], Self.shelfBoolean(visible) == nil { return false }
        }
        return true
    }
    // Foundation bridges numeric 0/1 through `as? Bool`; JSON decoding keeps
    // malformed numeric flags read-only, matching the Rust format validator.
    private static func shelfBoolean(_ value: Any) -> Bool? {
        guard let data = try? JSONSerialization.data(withJSONObject: [value]) else { return nil }
        return (try? JSONDecoder().decode([Bool].self, from: data))?.first
    }
    func shelfTypeVisible(_ kind: ShelfItem.Kind) -> Bool {
        guard canEditShelf, let visibility = shelfRecord?["visibility"] as? [String: Any],
              let value = visibility[kind.rawValue] else { return true }
        return Self.shelfBoolean(value) ?? true
    }
    var visibleShelfItems: [ShelfItem] {
        let shown = ShelfItem.Kind.allCases.filter { shelfTypeVisible($0) }
        return shelfItems.filter { shown.contains($0.kind) }
    }
    @discardableResult mutating func setShelfVisibility(_ visible: Bool, kind: ShelfItem.Kind? = nil, expected: [String: Any]) throws -> Bool {
        guard canEditShelf else { throw ShelfError.unsupported }
        guard let previous = try? JSONSerialization.data(withJSONObject: expected, options: [.sortedKeys]),
              let current = try? JSONSerialization.data(withJSONObject: metadata, options: [.sortedKeys]),
              previous == current else { throw ShelfError.changed }
        let kinds = kind.map { [$0] } ?? ShelfItem.Kind.allCases
        guard kinds.contains(where: { shelfTypeVisible($0) != visible }) else { return false }
        var own = metadata[Self.namespace] as? [String: Any] ?? [:]
        var shelf = shelfRecord ?? ["version": 1, "items": [String: Any](), "order": [String]()]
        var visibility = shelf["visibility"] as? [String: Any] ?? [:]
        for kind in kinds { visibility[kind.rawValue] = visible }
        shelf["visibility"] = visibility; own["shelf"] = shelf
        var updated = metadata; updated[Self.namespace] = own; metadata = updated
        return true
    }
    var quickNoteText: String { (shelfRecord?["quickEntry"] as? [String: Any])?["text"] as? String ?? "" }
    var quickNoteVisible: Bool {
        guard let value = (shelfRecord?["quickEntry"] as? [String: Any])?["visible"] else { return true }
        return Self.shelfBoolean(value) ?? true
    }
    @discardableResult mutating func setQuickNoteText(_ text: String, replacing expected: String) throws -> Bool {
        guard canEditShelf else { throw ShelfError.unsupported }
        guard quickNoteText.utf8.elementsEqual(expected.utf8) else { throw ShelfError.changed }
        guard !text.utf8.contains(0) else { throw ShelfError.invalid }
        guard !text.utf8.elementsEqual(expected.utf8) else { return false }
        updateQuickNote(key: "text", value: text); return true
    }
    @discardableResult mutating func setQuickNoteVisible(_ shown: Bool) throws -> Bool {
        guard canEditShelf else { throw ShelfError.unsupported }
        guard shown != quickNoteVisible else { return false }
        updateQuickNote(key: "visible", value: shown); return true
    }
    private mutating func updateQuickNote(key: String, value: Any) {
        var info = metadata; var own = info[Self.namespace] as? [String: Any] ?? [:]
        var shelf = shelfRecord ?? ["version": 1, "items": [String: Any](), "order": [String]()]
        var quick = shelf["quickEntry"] as? [String: Any] ?? [:]
        quick[key] = value; shelf["quickEntry"] = quick; own["shelf"] = shelf; info[Self.namespace] = own; metadata = info
    }
    @discardableResult mutating func addQuickNote(replacing expected: String, composing: Bool = false) throws -> String? {
        guard !composing else { throw ShelfError.composing }
        guard canEditShelf else { throw ShelfError.unsupported }
        guard quickNoteText.utf8.elementsEqual(expected.utf8) else { throw ShelfError.changed }
        guard !expected.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        var updated = self
        let item = ShelfItem(kind: .note, title: "Note", content: expected)
        try updated.saveShelfItem(item)
        try updated.setQuickNoteText("", replacing: expected)
        var info = updated.metadata; var own = info[Self.namespace] as! [String: Any]; var shelf = own["shelf"] as! [String: Any]
        let order = shelf["order"] as! [String]
        shelf["order"] = [item.id] + order.filter { $0 != item.id }
        own["shelf"] = shelf; info[Self.namespace] = own; updated.metadata = info
        self = updated; return item.id
    }
    var shelfItems: [ShelfItem] {
        guard canEditShelf else { return [] }
        let items = shelfRecord?["items"] as? [String: Any] ?? [:]
        let order = shelfRecord?["order"] as? [String] ?? []
        let ids = order + items.keys.filter { !order.contains($0) }.sorted()
        let decoded: [ShelfItem] = ids.compactMap { id in
            guard var record = items[id] as? [String: Any] else { return nil }
            record["id"] = id
            guard let data = try? JSONSerialization.data(withJSONObject: record) else { return nil }
            return try? JSONDecoder().decode(ShelfItem.self, from: data)
        }
        let pins = decoded.filter(\.pinned)
        let pinnedOrder = shelfRecord?["pinnedOrder"] as? [String] ?? []
        let ranked = pinnedOrder.compactMap { id in pins.first { $0.id == id } }
        return ranked + pins.filter { !pinnedOrder.contains($0.id) } + decoded.filter { !$0.pinned }
    }
    @discardableResult mutating func saveShelfItem(_ item: ShelfItem, replacing expected: ShelfItem? = nil) throws -> Bool {
        guard canEditShelf else { throw ShelfError.unsupported }
        guard !item.id.isEmpty, !item.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !item.title.utf8.contains(0), !item.content.utf8.contains(0) else { throw ShelfError.invalid }
        var own = metadata[Self.namespace] as? [String: Any] ?? [:]
        var shelf = shelfRecord ?? ["version": 1, "order": [String](), "items": [String: Any]()]
        var records = shelf["items"] as? [String: Any] ?? [:]
        var order = shelf["order"] as? [String] ?? []
        if let expected {
            guard item.id.utf8.elementsEqual(expected.id.utf8), let current = shelfItems.first(where: { $0.id == expected.id }), current.matches(expected) else { throw ShelfError.changed }
            if current.matches(item) { return false }
        } else {
            guard records[item.id] == nil else { throw ShelfError.changed }
            // Reserve any future/dangling order identities too.
            guard !order.contains(item.id), !(shelf["pinnedOrder"] as? [String] ?? []).contains(item.id) else { throw ShelfError.changed }
            order.append(item.id)
        }
        var record = records[item.id] as? [String: Any] ?? [:]
        record["kind"] = item.kind.rawValue; record["title"] = item.title; record["content"] = item.content; record["pinned"] = item.pinned
        records[item.id] = record; shelf["items"] = records; shelf["order"] = order
        own["shelf"] = shelf; var info = metadata; info[Self.namespace] = own; metadata = info
        return true
    }
    mutating func removeShelfItem(_ expected: ShelfItem) throws {
        guard canEditShelf else { throw ShelfError.unsupported }
        guard let current = shelfItems.first(where: { $0.id == expected.id }), current.matches(expected) else { throw ShelfError.changed }
        var own = metadata[Self.namespace] as? [String: Any] ?? [:]; var shelf = shelfRecord!
        var records = shelf["items"] as! [String: Any]; var order = shelf["order"] as! [String]
        records.removeValue(forKey: expected.id); order.removeAll { $0 == expected.id }
        if var pins = shelf["pinnedOrder"] as? [String] { pins.removeAll { $0 == expected.id }; shelf["pinnedOrder"] = pins }
        shelf["items"] = records; shelf["order"] = order; own["shelf"] = shelf
        var info = metadata; info[Self.namespace] = own; metadata = info
    }
    @discardableResult mutating func moveShelfItem(_ expected: ShelfItem, down: Bool, expectedOrder: [String]) throws -> Bool {
        guard canEditShelf else { throw ShelfError.unsupported }
        let items = shelfItems
        guard items.map(\.id) == expectedOrder, let current = items.first(where: { $0.id == expected.id }), current.matches(expected) else { throw ShelfError.changed }
        guard shelfTypeVisible(current.kind) else { throw ShelfError.changed }
        let shown = ShelfItem.Kind.allCases.filter { shelfTypeVisible($0) }
        let group = items.filter { $0.pinned == current.pinned && shown.contains($0.kind) }
        guard let index = group.firstIndex(where: { $0.id == current.id }), group.indices.contains(index + (down ? 1 : -1)) else { return false }
        let neighbor = group[index + (down ? 1 : -1)]
        var own = metadata[Self.namespace] as? [String: Any] ?? [:]; var shelf = shelfRecord!
        let records = shelf["items"] as! [String: Any]
        let orderKey = current.pinned ? "pinnedOrder" : "order"
        var order = shelf[orderKey] as? [String] ?? items.filter(\.pinned).map(\.id)
        let remaining = current.pinned ? items.filter(\.pinned).map(\.id) : records.keys.sorted()
        order += remaining.filter { !order.contains($0) }
        guard let first = order.firstIndex(of: current.id), let second = order.firstIndex(of: neighbor.id) else { throw ShelfError.changed }
        order.swapAt(first, second); shelf[orderKey] = order; own["shelf"] = shelf
        var info = metadata; info[Self.namespace] = own; metadata = info; return true
    }
}

enum ShelfInsertion {
    static func edit(source: String, expectedSource: String, selection: NSRange, content: String) throws -> ManuscriptEdit {
        let ns = source as NSString
        guard source.utf8.elementsEqual(expectedSource.utf8), !source.utf8.contains(0), !content.utf8.contains(0),
              selection.location >= 0, selection.location <= ns.length, selection.length >= 0, selection.length <= ns.length - selection.location else { throw ShelfError.selection }
        for at in [selection.location, NSMaxRange(selection)] where at > 0 && at < ns.length {
            if (0xD800...0xDBFF).contains(ns.character(at: at - 1)) && (0xDC00...0xDFFF).contains(ns.character(at: at)) { throw ShelfError.selection }
        }
        return ManuscriptEdit(source: source, range: selection, replacement: content,
            selection: NSRange(location: selection.location + content.utf16.count, length: 0), actionName: "Insert from Shelf")
    }
}

@MainActor enum ShelfCapture {
    @discardableResult static func store(in document: inout ScreenplayDocument, expectedSource: String, selection: NSRange, cut: Bool,
                                         id: String = UUID().uuidString, cache: ManuscriptAnalysisCache = ManuscriptAnalysisCache()) throws -> ShelfItem {
        // Validate both boundaries before substring extraction or any metadata mutation.
        let removal = try ShelfInsertion.edit(source: document.text, expectedSource: expectedSource, selection: selection, content: "")
        guard selection.length > 0 else { throw ShelfError.selection }
        let content = (document.text as NSString).substring(with: selection)
        let firstLine = content.components(separatedBy: .newlines).first(where: { !$0.trimmingCharacters(in: .whitespaces).isEmpty })?
            .trimmingCharacters(in: .whitespaces) ?? "Selected text"
        let item = ShelfItem(id: id, kind: .snippet, title: String(firstLine.prefix(60)), content: content)
        var updated = document
        try updated.saveShelfItem(item)
        if cut {
            let source = (document.text as NSString).replacingCharacters(in: removal.range, with: "")
            MilestoneWorkflow.replaceText(source, in: &updated, cache: cache)
        }
        document = updated
        return item
    }
}

extension ManuscriptTextView {
    @objc func copySelectionToShelf(_ sender: Any?) { captureSelection(cut: false) }
    @objc func cutSelectionToShelf(_ sender: Any?) { captureSelection(cut: true) }
    private func captureSelection(cut: Bool) {
        guard isEditable, !hasMarkedText(), selectedRanges.count == 1, selectedRange().length > 0, let onShelfCapture else { return }
        do {
            breakUndoCoalescing()
            try onShelfCapture(string, selectedRange(), cut)
            breakUndoCoalescing()
            undoManager?.setActionName(cut ? "Cut to Shelf" : "Copy to Shelf")
        } catch {
            let alert = NSAlert(error: error)
            if let window { alert.beginSheetModal(for: window) } else { alert.runModal() }
        }
    }
}

private struct ShelfSession: Identifiable { let id = UUID(); let item: ShelfItem; let original: ShelfItem? }
struct ShelfPanel: View {
    @Binding var document: ScreenplayDocument
    let insert: (String) throws -> Void
    var canCapture = false
    var capture: ((Bool) throws -> Void)? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var query = ""
    @State private var selection: String?
    @State private var editing: ShelfSession?
    @State private var error: String?
    private var items: [ShelfItem] { document.shelfItems }
    private var visibleItems: [ShelfItem] { document.visibleShelfItems }
    var body: some View {
        let expected = document.metadata
        VStack(alignment: .leading, spacing: 16) {
            HStack { Text("Shelf").font(.title2.weight(.semibold)); Spacer(); Text("\(items.count) \(items.count == 1 ? "item" : "items")").foregroundStyle(.secondary) }
            Text("Keep notes, snippets and checklists with your project. Shelf text stays separate until you insert it into the manuscript.").foregroundStyle(.secondary)
            HStack {
                Menu("Shelf Options", systemImage: "slider.horizontal.3") {
                    Toggle("Show Quick Note Entry", isOn: Binding(get: { document.quickNoteVisible }, set: { shown in change { try $0.setQuickNoteVisible(shown) } }))
                    Divider()
                    ForEach(ShelfItem.Kind.allCases, id: \.self) { kind in
                        Toggle("Show \(kind.title)s", isOn: Binding(
                            get: { document.shelfTypeVisible(kind) },
                            set: { shown in change { try $0.setShelfVisibility(shown, kind: kind, expected: expected) } }
                        ))
                    }
                    Divider()
                    Button("Show All") { change { try $0.setShelfVisibility(true, expected: expected) } }
                    Button("Hide All") { change { try $0.setShelfVisibility(false, expected: expected) } }
                }.fixedSize().disabled(!document.canEditShelf)
                Spacer()
                Text("\(visibleItems.count) shown · \(items.count) stored").font(.caption).foregroundStyle(.secondary)
            }
            TextField("Search the Shelf", text: $query).textFieldStyle(.roundedBorder)
            List(selection: $selection) {
                ForEach(visibleItems.filter { query.isEmpty || $0.title.localizedStandardContains(query) || $0.content.localizedStandardContains(query) }) { item in
                    HStack(alignment: .top, spacing: 12) {
                        Image(systemName: item.kind.icon).foregroundStyle(.secondary).frame(width: 20)
                        VStack(alignment: .leading, spacing: 4) {
                            HStack { Text(item.title).font(.headline); if item.pinned { Image(systemName: "pin.fill").font(.caption).foregroundStyle(.secondary) } }
                            Text(item.content.isEmpty ? item.kind.title : item.content).lineLimit(2).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button { editing = ShelfSession(item: item, original: item) } label: { Image(systemName: "pencil") }.buttonStyle(.borderless).accessibilityLabel("Edit \(item.title)")
                    }.padding(.vertical, 6).tag(item.id)
                    .contextMenu {
                        Button("Edit…") { editing = ShelfSession(item: item, original: item) }
                        Button(item.pinned ? "Unpin" : "Pin") { change { doc in var changed = item; changed.pinned.toggle(); return try doc.saveShelfItem(changed, replacing: item) } }
                        Button("Move Up") { change { try $0.moveShelfItem(item, down: false, expectedOrder: items.map(\.id)) } }
                        Button("Move Down") { change { try $0.moveShelfItem(item, down: true, expectedOrder: items.map(\.id)) } }
                    }
                }
            }.onKeyPress(.return) {
                guard let item = visibleItems.first(where: { $0.id == selection }) else { return .ignored }
                editing = ShelfSession(item: item, original: item); return .handled
            }.onChange(of: query) { selection = nil }
            .onChange(of: visibleItems.map(\.id)) { selection = nil }
            .help("Use the arrow keys to select an item, then Return to edit it.")
            .overlay {
                if !document.canEditShelf {
                    ContentUnavailableView("Shelf data preserved", systemImage: "tray", description: Text("This Shelf uses an unsupported format. Editing is unavailable."))
                } else if visibleItems.isEmpty && !items.isEmpty {
                    ContentUnavailableView("Shelf items are hidden", systemImage: "eye.slash", description: Text("Your \(items.count) stored items are kept. Choose Shelf Options → Show All to see them again."))
                } else if items.isEmpty {
                    ContentUnavailableView("A place to keep possibilities", systemImage: "tray", description: Text("Add a note, snippet or checklist to get started."))
                }
            }
            HStack {
                Menu("Add Item", systemImage: "plus") { ForEach(ShelfItem.Kind.allCases, id: \.self) { kind in Button(kind.title) { editing = ShelfSession(item: ShelfItem(kind: kind, title: kind.title), original: nil) } }
                    Divider()
                    Button("Copy Selection to Shelf") { captureSelection(cut: false) }.disabled(!canCapture)
                    Button("Cut Selection to Shelf") { captureSelection(cut: true) }.disabled(!canCapture)
                }.fixedSize().disabled(!document.canEditShelf)
                Spacer(); Text("Undo changes after closing the Shelf.").font(.caption).foregroundStyle(.secondary)
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }.padding(24).frame(width: 610, height: 550)
        .sheet(item: $editing) { session in
            ShelfEditor(item: session.item, existing: session.original != nil, save: { item in
                var updated = document
                if try updated.saveShelfItem(item, replacing: session.original) { document = updated }
            }, remove: {
                guard let original = session.original else { return }
                var updated = document; try updated.removeShelfItem(original); document = updated
            }, insert: { content in try insert(content); dismiss() })
        }
        .alert("Shelf Not Changed", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button("OK") { error = nil } } message: { Text(error ?? "") }
    }
    private func captureSelection(cut: Bool) {
        do { try capture?(cut); query = "" }
        catch { self.error = error.localizedDescription }
    }
    private func change(_ operation: (inout ScreenplayDocument) throws -> Bool) {
        do { var updated = document; if try operation(&updated) { document = updated } }
        catch { self.error = error.localizedDescription }
    }
}

struct ShelfEditor: View {
    @State var item: ShelfItem
    let existing: Bool
    let save: (ShelfItem) throws -> Void
    let remove: () throws -> Void
    let insert: (String) throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var error: String?
    @State private var deleting = false
    @State private var checklist = false
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(item.kind.title).font(.title2.weight(.semibold))
            TextField("Title", text: $item.title).textFieldStyle(.roundedBorder)
            Toggle("Pin to the top", isOn: $item.pinned)
            TextEditor(text: $item.content).font(.body).border(Color.secondary.opacity(0.25)).accessibilityLabel("Shelf content")
            if item.kind == .checklist { Button("Edit Checklist…") { checklist = true } }
            Text("Insert replaces the manuscript selection captured when the Shelf opened. Save keeps this item on the Shelf.").font(.caption).foregroundStyle(.secondary)
            HStack {
                if existing { Button("Delete…", role: .destructive) { deleting = true } }
                Button("Insert into Manuscript") { perform { try insert(item.content); dismiss() } }.disabled(item.content.isEmpty)
                Spacer(); Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Save") { perform { try save(item); dismiss() } }.keyboardShortcut(.defaultAction)
            }
        }.padding(24).frame(width: 580, height: 440)
        .sheet(isPresented: $checklist) { TodoListPanel(text: $item.content, format: .general, undoHint: "Changes stay in this draft until you save the Shelf item.", locationName: "checklist draft", reveal: nil) }
        .confirmationDialog("Delete this Shelf item?", isPresented: $deleting, titleVisibility: .visible) {
            Button("Delete", role: .destructive) { perform { try remove(); dismiss() } }
            Button("Cancel", role: .cancel) { }
        } message: { Text("Its manuscript text, if already inserted, will stay. You can undo deletion after closing the Shelf.") }
        .alert("Shelf Not Changed", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button("OK") { error = nil } } message: { Text(error ?? "") }
    }
    private func perform(_ operation: () throws -> Void) { do { try operation() } catch { self.error = error.localizedDescription } }
}

private struct ShelfCaptureAvailability: FocusedValueKey { typealias Value = Bool }
extension FocusedValues {
    var canCaptureShelfSelection: Bool? {
        get { self[ShelfCaptureAvailability.self] }
        set { self[ShelfCaptureAvailability.self] = newValue }
    }
}
struct ShelfCommands: Commands {
    @FocusedValue(\.canCaptureShelfSelection) private var available
    var body: some Commands {
        CommandGroup(after: .pasteboard) {
            Button("Cut to Shelf") { WriterMenuAction.perform(#selector(ManuscriptTextView.cutSelectionToShelf(_:))) }
                .disabled(available != true)
            Button("Copy to Shelf") { WriterMenuAction.perform(#selector(ManuscriptTextView.copySelectionToShelf(_:))) }
                .disabled(available != true)
        }
    }
}
