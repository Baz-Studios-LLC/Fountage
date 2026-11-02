import SwiftUI
import AppKit
import UniformTypeIdentifiers

struct ShelfDragSession {
    static let type = UTType(exportedAs: "com.bazstudios.fountage.shelf-move", conformingTo: .data)
    let id = UUID()
    let item: ShelfItem
    let order: [String]
    var token: Data { Data(id.uuidString.utf8) }
    func pasteboardItem() -> NSPasteboardItem {
        let item = NSPasteboardItem()
        item.setString(self.item.content, forType: .string)
        item.setData(token, forType: NSPasteboard.PasteboardType(Self.type.identifier))
        return item
    }
}
struct ShelfDropReceipt {
    let id = UUID()
    let drag: ShelfDragSession
}
@MainActor final class ShelfDragStore: ObservableObject {
    private(set) var session: ShelfDragSession?
    private var pending: ShelfDropReceipt?
    func begin(_ drag: ShelfDragSession) { session = drag; pending = nil }
    func end(id: UUID) { if session?.id == id { session = nil } }
    func invalidate() { session = nil; pending = nil }
    func claim() -> ShelfDropReceipt? {
        guard let session else { return nil }
        let receipt = ShelfDropReceipt(drag: session); self.session = nil; pending = receipt
        return receipt
    }
    func fail(_ receipt: ShelfDropReceipt) { if pending?.id == receipt.id { pending = nil } }
    @discardableResult func commit(_ receipt: ShelfDropReceipt, token: Data, to document: inout ScreenplayDocument, before target: String?) throws -> Bool {
        guard pending?.id == receipt.id else { throw ShelfError.changed }
        pending = nil
        guard token == receipt.drag.token else { throw ShelfError.changed }
        return try document.reorderShelfItem(receipt.drag.item, before: target, expectedOrder: receipt.drag.order)
    }
}

extension ScreenplayDocument {
    @discardableResult mutating func reorderShelfItem(_ expected: ShelfItem, before target: String?, expectedOrder: [String]) throws -> Bool {
        guard canEditShelf else { throw ShelfError.unsupported }
        let current = shelfItems
        guard current.map(\.id) == expectedOrder, let item = current.first(where: { $0.id == expected.id }), item.matches(expected) else { throw ShelfError.changed }
        guard shelfTypeVisible(item.kind) else { throw ShelfError.changed }
        if target == item.id { return false }
        if let target { guard let destination = current.first(where: { $0.id == target }), destination.pinned == item.pinned, shelfTypeVisible(destination.kind) else { throw ShelfError.changed } }
        let shown = ShelfItem.Kind.allCases.filter { shelfTypeVisible($0) }
        let group = current.filter { $0.pinned == item.pinned && shown.contains($0.kind) }.map(\.id)
        var reordered = group.filter { $0 != item.id }
        let destination = target.flatMap { reordered.firstIndex(of: $0) } ?? reordered.count
        reordered.insert(item.id, at: destination)
        guard reordered != group else { return false }
        var info = metadata; var own = info[Self.namespace] as! [String: Any]; var shelf = own["shelf"] as! [String: Any]
        let records = shelf["items"] as! [String: Any]
        let key = item.pinned ? "pinnedOrder" : "order"
        var order = shelf[key] as? [String] ?? current.filter(\.pinned).map(\.id)
        let remaining = item.pinned ? current.filter(\.pinned).map(\.id) : records.keys.sorted()
        order += remaining.filter { !order.contains($0) }
        var cursor = 0
        for index in order.indices where group.contains(order[index]) { order[index] = reordered[cursor]; cursor += 1 }
        shelf[key] = order; own["shelf"] = shelf; info[Self.namespace] = own; metadata = info
        return true
    }
    mutating func addDroppedShelfText(_ values: [String]) throws {
        guard !values.isEmpty else { throw ShelfError.invalid }
        var updated = self
        for content in values {
            guard !content.isEmpty, !content.utf8.contains(0) else { throw ShelfError.invalid }
            let title = content.components(separatedBy: .newlines).first(where: { !$0.trimmingCharacters(in: .whitespaces).isEmpty })?.trimmingCharacters(in: .whitespaces) ?? "Dropped text"
            try updated.saveShelfItem(ShelfItem(kind: .snippet, title: String(title.prefix(60)), content: content))
        }
        self = updated
    }
}

private struct ShelfDrop: DropDelegate {
    let store: ShelfDragStore
    let target: String?
    let enabled: Bool
    let highlight: (Bool) -> Void
    let reorder: (ShelfDropReceipt, Result<Data, Error>, String?) -> Void
    let add: ([NSItemProvider]) -> Void
    func validateDrop(info: DropInfo) -> Bool { enabled && info.hasItemsConforming(to: [ShelfDragSession.type, .text]) }
    func dropEntered(info: DropInfo) { highlight(validateDrop(info: info)) }
    func dropExited(info: DropInfo) { highlight(false) }
    func dropUpdated(info: DropInfo) -> DropProposal? {
        guard validateDrop(info: info) else { return DropProposal(operation: .forbidden) }
        return DropProposal(operation: store.session != nil && info.hasItemsConforming(to: [ShelfDragSession.type]) ? .move : .copy)
    }
    func performDrop(info: DropInfo) -> Bool {
        highlight(false)
        guard validateDrop(info: info) else { return false }
        if let session = store.session, info.hasItemsConforming(to: [ShelfDragSession.type]) {
            let providers = info.itemProviders(for: [ShelfDragSession.type])
            guard providers.count == 1, let receipt = store.claim(), receipt.drag.id == session.id else { return false }
            providers[0].loadDataRepresentation(forTypeIdentifier: ShelfDragSession.type.identifier) { data, error in
                let result: Result<Data, Error> = data.map { .success($0) } ?? .failure(error ?? ShelfError.changed)
                DispatchQueue.main.async { reorder(receipt, result, target) }
            }
        } else { add(info.itemProviders(for: [.text])) }
        return true
    }
}

struct ShelfInspector: View {
    @Binding var document: ScreenplayDocument
    let manage: () -> Void
    let prepareEdit: () -> Void
    let insert: (String) throws -> Void
    @StateObject private var drag = ShelfDragStore()
    @State private var query = ""
    @State private var editing: ShelfItem?
    @State private var error: String?
    @State private var highlighted: String?
    @State private var loading: ShelfTextDrop?
    @State private var loadID: UUID?
    private var items: [ShelfItem] { document.shelfItems }
    private var visibleItems: [ShelfItem] { document.visibleShelfItems }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack { Text("Shelf").font(.system(size: 23, design: .serif)); Spacer(); Button("Manage…", action: manage).buttonStyle(.link) }
            Text("Drag text here to keep it. Drag a snippet onto the page to reuse it.").font(.caption).foregroundStyle(.secondary)
            if document.canEditShelf && document.quickNoteVisible { QuickNoteEntry(document: $document) }
            TextField("Search Shelf", text: $query).textFieldStyle(.roundedBorder)
            if !query.isEmpty { Text("Clear search to add or reorder by dragging.").font(.caption).foregroundStyle(.secondary) }
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    ForEach(visibleItems.filter { query.isEmpty || $0.title.localizedStandardContains(query) || $0.content.localizedStandardContains(query) }) { item in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Image(systemName: item.kind.icon).foregroundStyle(.secondary)
                                Text(item.title).font(.headline).lineLimit(2)
                                Spacer(minLength: 0)
                                if item.pinned { Image(systemName: "pin.fill").font(.caption).foregroundStyle(.secondary) }
                                ShelfDragHandle(begin: {
                                    let session = ShelfDragSession(item: item, order: items.map(\.id)); drag.begin(session); return session
                                }, cancel: { id in drag.end(id: id) }).frame(width: 16, height: 20)
                                Button { prepareEdit(); editing = item } label: { Image(systemName: "pencil") }.buttonStyle(.borderless).accessibilityLabel("Edit \(item.title)")
                            }
                            if item.kind == .checklist {
                                ShelfChecklistControls(document: $document, item: item)
                            } else {
                                Text(item.content.isEmpty ? "Empty " + item.kind.title.lowercased() : item.content).font(.caption).foregroundStyle(.secondary).lineLimit(4)
                            }
                        }.padding(12).frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
                        .overlay(alignment: .top) { if highlighted == item.id { Rectangle().fill(Color.accentColor).frame(height: 2) } }
                        .onDrop(of: [ShelfDragSession.type, .text], delegate: drop(target: item.id))
                    }
                    Text(visibleItems.isEmpty && !items.isEmpty ? "Items are hidden. Choose Manage → Shelf Options → Show All." : items.isEmpty ? "Drop text to begin" : "Drop text or move an item to the end")
                        .font(.caption).foregroundStyle(.secondary).frame(maxWidth: .infinity, minHeight: 70)
                        .background(highlighted == "end" ? Color.accentColor.opacity(0.12) : Color.clear, in: RoundedRectangle(cornerRadius: 8))
                        .onDrop(of: [ShelfDragSession.type, .text], delegate: drop(target: nil))
                }
            }
            if loading != nil { ProgressView("Adding text…").controlSize(.small) }
        }.padding(18)
        .onDisappear { loading?.cancel(); loading = nil; loadID = nil; drag.invalidate() }
        .onChange(of: visibleItems.map(\.id)) { drag.invalidate(); highlighted = nil }
        .sheet(item: $editing) { item in
            ShelfEditor(item: item, existing: true, save: { changed in var updated = document; if try updated.saveShelfItem(changed, replacing: item) { document = updated } }, remove: { var updated = document; try updated.removeShelfItem(item); document = updated }, insert: insert)
        }
        .alert("Shelf Not Changed", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) { Button("OK") { error = nil } } message: { Text(error ?? "") }
    }
    private func drop(target: String?) -> ShelfDrop {
        ShelfDrop(store: drag, target: target, enabled: document.canEditShelf && loading == nil && query.isEmpty, highlight: { highlighted = $0 ? (target ?? "end") : nil }, reorder: { receipt, result, target in
            do {
                let token = try result.get()
                var updated = document
                if try drag.commit(receipt, token: token, to: &updated, before: target) { document = updated }
            } catch { drag.fail(receipt); self.error = error.localizedDescription }
        }, add: receive)
    }
    private func receive(_ providers: [NSItemProvider]) {
        guard !providers.isEmpty, loading == nil else { return }
        let operation = UUID(); loadID = operation
        // Starts all loadObject requests now, before performDrop returns.
        loading = ShelfTextDrop.start(providers) { result in
            guard loadID == operation else { return }
            defer { loading = nil; loadID = nil }
            do {
                let values = try result.get()
                var updated = document; try updated.addDroppedShelfText(values); document = updated
            } catch { self.error = error.localizedDescription }
        }
    }
}

private struct ShelfDragHandle: NSViewRepresentable {
    let begin: () -> ShelfDragSession
    let cancel: (UUID) -> Void
    func makeNSView(context: Context) -> ShelfDragHandleView {
        let view = ShelfDragHandleView(); view.begin = begin; view.cancel = cancel; return view
    }
    func updateNSView(_ view: ShelfDragHandleView, context: Context) { view.begin = begin; view.cancel = cancel }
}
private final class ShelfDragHandleView: NSImageView, NSDraggingSource {
    var begin: (() -> ShelfDragSession)?
    var cancel: ((UUID) -> Void)?
    private var dragging = false
    private var activeID: UUID?
    override init(frame: NSRect) {
        super.init(frame: frame)
        image = NSImage(systemSymbolName: "line.3.horizontal", accessibilityDescription: "Drag Shelf item")
        contentTintColor = .secondaryLabelColor; imageScaling = .scaleProportionallyDown
        setAccessibilityLabel("Drag Shelf item")
        setAccessibilityHelp("Drag onto another Shelf item to reorder, or onto the manuscript to insert a copy. Manage offers Move Up and Move Down alternatives.")
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) { dragging = false }
    override func mouseDragged(with event: NSEvent) { startDrag(with: event) }
    private func startDrag(with event: NSEvent) {
        guard !dragging, let source = begin?() else { return }
        dragging = true; activeID = source.id
        let item = NSDraggingItem(pasteboardWriter: source.pasteboardItem())
        item.setDraggingFrame(bounds, contents: image)
        let native = beginDraggingSession(with: [item], event: event, source: self)
        native.animatesToStartingPositionsOnCancelOrFail = true
    }
    func draggingSession(_ session: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation { context == .withinApplication ? [.copy, .move] : .copy }
    func draggingSession(_ session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation) {
        dragging = false
        if let activeID { cancel?(activeID) }
        activeID = nil
    }
}
