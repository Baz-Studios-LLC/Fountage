import SwiftUI
import UniformTypeIdentifiers

/// An opaque process-local token; manuscript text is never placed on the drag pasteboard.
struct OutlineDragSession {
    static let type = UTType(exportedAs: "com.bazstudios.fountage.outline-move", conformingTo: .data)
    let id = UUID()
    let line: Int
    let source: String
    let format: DocumentFormat
    private let index: OutlineIndex
    init(line: Int, source: String, format: DocumentFormat) {
        self.line = line; self.source = source; self.format = format
        index = source.contains("\0") ? OutlineIndex() : OutlineIndex(ManuscriptAnalysis.parse(source, format: format).headings)
    }
    func canMove(to targetLine: Int, after: Bool) -> Bool { index.canMove(line: line, to: targetLine, after: after) }
    var token: Data { Data(id.uuidString.utf8) }
    func provider() -> NSItemProvider {
        let provider = NSItemProvider()
        let data = token
        provider.registerDataRepresentation(forTypeIdentifier: Self.type.identifier, visibility: .ownProcess) { completion in
            completion(data, nil); return nil
        }
        return provider
    }
    func edit(token: Data, currentSource: String, currentFormat: DocumentFormat, targetLine: Int, after: Bool) -> ManuscriptEdit? {
        guard token == self.token, format == currentFormat,
              currentSource.utf8.elementsEqual(source.utf8) else { return nil }
        return index.move(line: line, to: targetLine, after: after, source: currentSource)
    }
}

/// Delegates read the live session even before SwiftUI redraws during AppKit drag tracking.
@MainActor final class OutlineDragStore: ObservableObject {
    @Published private(set) var session: OutlineDragSession?
    func begin(_ session: OutlineDragSession) { self.session = session }
    func cancel(id: UUID? = nil) {
        if id == nil || session?.id == id { session = nil }
    }
    func resolve(_ drag: OutlineDragSession, token: Data, source: String, format: DocumentFormat, target: Int, after: Bool) -> ManuscriptEdit? {
        guard session?.id == drag.id else { return nil }
        return drag.edit(token: token, currentSource: source, currentFormat: format, targetLine: target, after: after)
    }
}

private struct OutlineMoveDrop: DropDelegate {
    let store: OutlineDragStore?
    private var session: OutlineDragSession? { store?.session }
    let line: Int
    let height: CGFloat
    let highlight: (Bool?) -> Void
    let perform: (OutlineDragSession, Data, Bool) -> Void
    func validateDrop(info: DropInfo) -> Bool {
        guard let session else { return false }
        return (session.canMove(to: line, after: false) || session.canMove(to: line, after: true))
            && info.hasItemsConforming(to: [OutlineDragSession.type])
    }
    func dropEntered(info: DropInfo) { _ = dropUpdated(info: info) }
    func dropUpdated(info: DropInfo) -> DropProposal? {
        let after = info.location.y >= height / 2
        guard validateDrop(info: info), session?.canMove(to: line, after: after) == true else {
            highlight(nil); return DropProposal(operation: .forbidden)
        }
        highlight(after)
        return DropProposal(operation: .move)
    }
    func dropExited(info: DropInfo) { highlight(nil) }
    func performDrop(info: DropInfo) -> Bool {
        highlight(nil)
        let providers = info.itemProviders(for: [OutlineDragSession.type])
        guard validateDrop(info: info), let session, providers.count == 1 else { return false }
        let after = info.location.y >= height / 2
        guard session.canMove(to: line, after: after) else { return false }
        providers[0].loadDataRepresentation(forTypeIdentifier: OutlineDragSession.type.identifier) { data, _ in
            guard let data else { return }
            DispatchQueue.main.async { perform(session, data, after) }
        }
        return true
    }
}

struct OutlineDraggableRow<Content: View>: View {
    let line: Int
    let canDrag: Bool
    let source: String
    let format: DocumentFormat
    @ObservedObject var store: OutlineDragStore
    let enabled: Bool
    let start: (OutlineDragSession) -> Void
    let perform: (OutlineDragSession, Data, Bool) -> Void
    @ViewBuilder let content: () -> Content
    @State private var height: CGFloat = 40
    @State private var after: Bool?
    var body: some View {
        HStack(spacing: 5) {
            content().frame(maxWidth: .infinity, alignment: .leading)
            if canDrag {
                OutlineDragHandle(begin: {
                    let drag = OutlineDragSession(line: line, source: source, format: format)
                    start(drag); return drag
                }, cancel: { id in
                    if store.session?.id == id { after = nil; store.cancel(id: id) }
                })
                    .frame(width: 14, height: 20)
                    .help("Drag above or below another outline item to move it. Sections move with their contents.")
            }
        }
        .contentShape(Rectangle())
        .onChange(of: store.session?.id) { _, id in if id == nil { after = nil } }
        .background(GeometryReader { proxy in
            Color.clear.onAppear { height = proxy.size.height }
                .onChange(of: proxy.size.height) { _, value in height = value }
        })
        .overlay(alignment: after == true ? .bottom : .top) {
            if after != nil { Rectangle().fill(Color.accentColor).frame(height: 2).allowsHitTesting(false) }
        }
        .onDrop(of: [OutlineDragSession.type], delegate: OutlineMoveDrop(store: enabled ? store : nil, line: line, height: height,
            highlight: { after = $0 }, perform: perform))
    }
}

/// A native drag source alongside the row’s ordinary selection and context menu.
struct OutlineDragHandle: NSViewRepresentable {
    let begin: () -> OutlineDragSession
    let cancel: (UUID) -> Void
    func makeNSView(context: Context) -> OutlineDragHandleView {
        let view = OutlineDragHandleView()
        view.begin = begin; view.cancel = cancel
        return view
    }
    func updateNSView(_ view: OutlineDragHandleView, context: Context) {
        view.begin = begin; view.cancel = cancel
    }
}

final class OutlineDragHandleView: NSImageView, NSDraggingSource {
    var begin: (() -> OutlineDragSession)?
    var cancel: ((UUID) -> Void)?
    private var dragging = false
    private var activeID: UUID?
    override init(frame: NSRect) {
        super.init(frame: frame)
        image = NSImage(systemSymbolName: "line.3.horizontal", accessibilityDescription: "Drag outline item")
        contentTintColor = .tertiaryLabelColor
        imageScaling = .scaleProportionallyDown
        setAccessibilityLabel("Drag outline item")
        setAccessibilityHelp("Drag above or below another outline item. Use the row’s Move menu for a keyboard alternative.")
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) { dragging = false }
    override func mouseDragged(with event: NSEvent) {
        guard !dragging, let session = begin?() else { return }
        dragging = true; activeID = session.id
        let writer = NSPasteboardItem()
        writer.setData(session.token, forType: NSPasteboard.PasteboardType(OutlineDragSession.type.identifier))
        let item = NSDraggingItem(pasteboardWriter: writer)
        item.setDraggingFrame(bounds, contents: image)
        let native = beginDraggingSession(with: [item], event: event, source: self)
        native.animatesToStartingPositionsOnCancelOrFail = true
    }
    func draggingSession(_ session: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation {
        context == .withinApplication ? .move : []
    }
    func draggingSession(_ session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation) {
        dragging = false
        if operation.isEmpty, let activeID { cancel?(activeID) }
        activeID = nil
    }
}
