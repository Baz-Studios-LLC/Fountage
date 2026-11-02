import SwiftUI
import PDFKit
import UniformTypeIdentifiers

/// A synchronous option change replaces the complete export snapshot before it
/// returns. Menu actions cannot export the preceding SwiftUI render's State value.
@MainActor final class ScreenplayPreviewModel: ObservableObject {
    static let continuationKey = "screenplayDialogueContinuations"
    @Published private(set) var pdf: PDFDocument?
    private(set) var data: Data?
    @Published private(set) var error: String?
    @Published private(set) var options: ScreenplayPrintOptions
    var continuations: Bool { options.dialogueContinuations }
    private(set) var exportTitle = ""
    private let source: String
    private let title: String
    private let defaults: UserDefaults
    private let saveOptions: ((ScreenplayPrintOptions, ScreenplayPrintOptions) throws -> Void)?
    init(source: String, title: String, defaults: UserDefaults = .standard, options: ScreenplayPrintOptions? = nil, saveOptions: ((ScreenplayPrintOptions, ScreenplayPrintOptions) throws -> Void)? = nil) {
        self.source = source; self.title = title; self.defaults = defaults
        self.options = options ?? ScreenplayPrintOptions(defaults: defaults)
        self.saveOptions = saveOptions
    }
    func setContinuations(_ value: Bool) {
        updateOptions { $0.dialogueContinuations = value }
    }
    func updateOptions(_ change: (inout ScreenplayPrintOptions) -> Void) {
        var changed = options; change(&changed)
        guard changed != options else { return }
        do {
            if let saveOptions { try saveOptions(changed, options) }
            else { changed.save(to: defaults) }
            options = changed; render()
        } catch { self.error = error.localizedDescription }
    }
    func receiveOptions(_ updated: ScreenplayPrintOptions) {
        guard updated != options else { return }
        options = updated; render()
    }
    func render() {
        pdf = nil; data = nil; error = nil
        do {
            let output = try ScreenplayOutput.parse(source)
            exportTitle = output.title_fields.first(where: { $0.key == "title" })?.values.map(\.text).joined(separator: " ") ?? title
            let rendered = try ScreenplayPagination(output: output, options: options).pdfData()
            guard let document = PDFDocument(data: rendered) else { throw ScreenplayPagination.LayoutError.pdfFailure }
            data = rendered; pdf = document
        } catch { self.error = error.localizedDescription }
    }
    func printInfo() -> NSPrintInfo {
        let info = (NSPrintInfo.shared.copy() as? NSPrintInfo) ?? NSPrintInfo()
        info.orientation = .portrait
        info.paperSize = options.paper.size
        info.topMargin = 0; info.bottomMargin = 0; info.leftMargin = 0; info.rightMargin = 0
        return info
    }
    func showError(_ message: String?) { error = message }
}

struct ScreenplayPreview: View {
    @Environment(\.dismiss) private var dismiss
    @StateObject private var model: ScreenplayPreviewModel
    private let storedOptions: ScreenplayPrintOptions?
    init(source: String, title: String, options: ScreenplayPrintOptions? = nil,
         saveOptions: ((ScreenplayPrintOptions, ScreenplayPrintOptions) throws -> Void)? = nil) {
        storedOptions = options
        _model = StateObject(wrappedValue: ScreenplayPreviewModel(source: source, title: title, options: options, saveOptions: saveOptions))
    }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Print Preview").font(.headline)
                    Text(model.pdf.map { "\($0.pageCount) \($0.pageCount == 1 ? "page" : "pages") · \(model.options.paper.name)" } ?? "Screenplay")
                        .font(.caption).foregroundStyle(.secondary)
                    Text(storedOptions == nil ? "Uses app output defaults" : "Output settings saved with this project")
                        .font(.caption2).foregroundStyle(.secondary)
                }
                Spacer()
                ScreenplayOutputMenu(model: model).fixedSize()
                Button("Print…", systemImage: "printer") { printPDF() }.disabled(model.pdf == nil)
                Button("Save PDF…", systemImage: "square.and.arrow.down") { savePDF() }.disabled(model.data == nil)
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }.padding(16)
            Divider()
            if let pdf = model.pdf { PDFPreviewSurface(document: pdf) }
            else if let error = model.error { ContentUnavailableView("Preview Unavailable", systemImage: "doc.badge.exclamationmark", description: Text(error)) }
            else { ProgressView("Formatting manuscript…").frame(maxWidth: .infinity, maxHeight: .infinity) }
        }
        .frame(minWidth: 680, idealWidth: 900, minHeight: 560, idealHeight: 760)
        .task { model.render() }
        .onChange(of: storedOptions) { _, options in if let options { model.receiveOptions(options) } }
        .onReceive(NotificationCenter.default.publisher(for: UserDefaults.didChangeNotification).receive(on: RunLoop.main)) { _ in
            if storedOptions == nil { model.receiveOptions(ScreenplayPrintOptions(defaults: .standard)) }
        }
        .alert("PDF Error", isPresented: Binding(get: { model.error != nil && model.pdf != nil }, set: { if !$0 { model.showError(nil) } })) {
            Button("OK") { model.showError(nil) }
        } message: { Text(model.error ?? "") }
    }
    private func savePDF() {
        guard model.data != nil else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        panel.nameFieldStringValue = (model.exportTitle.isEmpty ? "Screenplay" : model.exportTitle.replacingOccurrences(of: "/", with: "-")) + ".pdf"
        panel.canCreateDirectories = true
        let snapshot = model
        // Do not nest a modal run loop inside a SwiftUI menu action. Native menu
        // bindings can be delivered on the next normal event-loop pass.
        panel.begin { response in
            guard response == .OK, let url = panel.url, let data = snapshot.data else { return }
            do { try data.write(to: url, options: .atomic) }
            catch { snapshot.showError(error.localizedDescription) }
        }
    }

    private func printPDF() {
        let snapshot = model
        // Leave the native menu event before taking the print snapshot.
        DispatchQueue.main.async {
            guard let operation = snapshot.pdf?.printOperation(for: snapshot.printInfo(), scalingMode: .pageScaleNone, autoRotate: false) else { return }
            operation.showsPrintPanel = true
            operation.run()
        }
    }

}
private struct PDFPreviewSurface: NSViewRepresentable {
    let document: PDFDocument
    func makeNSView(context: Context) -> PDFView {
        let view = PDFView()
        view.displayMode = .singlePageContinuous
        view.autoScales = true
        view.displaysPageBreaks = true
        view.backgroundColor = .windowBackgroundColor
        view.document = document
        return view
    }
    func updateNSView(_ view: PDFView, context: Context) {
        if view.document !== document { view.document = document }
    }
}
