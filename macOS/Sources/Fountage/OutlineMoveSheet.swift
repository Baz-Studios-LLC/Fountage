import SwiftUI

struct OutlineMoveRequest: Identifiable {
    let id = UUID()
    let line: Int
    let source: String
    let format: DocumentFormat
    let index: OutlineIndex
    init(line: Int, source: String, format: DocumentFormat) {
        self.line = line; self.source = source; self.format = format
        index = OutlineIndex(ManuscriptAnalysis.parse(source, format: format).headings)
    }
    func destinations(after: Bool) -> [OutlineIndex.Row] {
        index.rows.filter { index.canMove(line: line, to: $0.heading.line, after: after) }
    }
    func edit(target: Int, after: Bool, currentSource: String, currentFormat: DocumentFormat) -> ManuscriptEdit? {
        guard currentFormat == format, currentSource.utf8.elementsEqual(source.utf8) else { return nil }
        return index.move(line: line, to: target, after: after, source: source)
    }
}

struct OutlineMoveSheet: View {
    let request: OutlineMoveRequest
    let perform: (Int, Bool) -> Bool
    @Environment(\.dismiss) private var dismiss
    @State private var after = false
    @State private var target: Int
    @State private var error: String?
    init(request: OutlineMoveRequest, perform: @escaping (Int, Bool) -> Bool) {
        self.request = request; self.perform = perform
        _target = State(initialValue: request.destinations(after: false).first?.heading.line ?? -1)
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Move Outline Item").font(.system(size: 25, design: .serif))
            Text("Scenes keep their text and notes. Sections move with all their contents; heading levels stay the same.")
                .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            Picker("Position", selection: $after) {
                Text("Before").tag(false); Text("After").tag(true)
            }.pickerStyle(.segmented)
            Picker("Destination", selection: $target) {
                if request.destinations(after: after).isEmpty { Text("No available destination").tag(-1) }
                ForEach(request.destinations(after: after)) { row in
                    Text(String(repeating: "  ", count: min(row.depth, 4)) + row.heading.heading).tag(row.heading.line)
                }
            }
            if let error { Text(error).font(.callout).foregroundStyle(.red) }
            Divider()
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Move") {
                    if perform(target, after) { dismiss() }
                    else { error = "The manuscript changed or this move is no longer available. Cancel and try again." }
                }.keyboardShortcut(.defaultAction).disabled(!request.destinations(after: after).contains { $0.heading.line == target })
            }
        }.padding(24).frame(width: 470)
        .onChange(of: after) { _, value in
            if !request.destinations(after: value).contains(where: { $0.heading.line == target }) {
                target = request.destinations(after: value).first?.heading.line ?? -1
            }
            error = nil
        }
    }
}
