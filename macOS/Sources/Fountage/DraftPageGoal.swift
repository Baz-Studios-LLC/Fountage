import SwiftUI

struct PageGoalRequest: Equatable {
    let source: ExactSource
    let options: ScreenplayPrintOptions
    let enabled: Bool
}

/// Uses the same body pagination as PDF output; no word-count estimate or PDF allocation.
@MainActor final class DraftPageCount: ObservableObject {
    @Published private(set) var count: Int?
    @Published private(set) var error: String?
    @Published private(set) var resolvedRequest: PageGoalRequest?
    private var requestID = UUID()

    static func calculate(source: String, options: ScreenplayPrintOptions) throws -> Int {
        try calculate(output: ScreenplayOutput.parse(source), options: options)
    }
    static func calculate(output: ScreenplayOutput, options: ScreenplayPrintOptions) throws -> Int {
        var bodyOptions = options
        // Long or disabled front matter must not affect manuscript page progress.
        bodyOptions.includeTitlePage = false
        let layout = try ScreenplayPagination(output: output, options: bodyOptions)
        // The preview supplies an empty writing page even for an empty document.
        // A manuscript goal starts at zero until there is printable content.
        guard layout.pages.contains(where: { page in
            page.fragments.contains(where: { !$0.text.string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty })
        }) else { return 0 }
        return layout.pages.filter { $0.number != nil }.count
    }

    func refresh(_ request: PageGoalRequest, delay: Duration = .milliseconds(250)) async {
        let id = UUID(); requestID = id
        count = nil; error = nil; resolvedRequest = nil
        guard request.enabled else { return }
        do {
            try await Task.sleep(for: delay)
            try Task.checkCancellation()
            guard requestID == id else { return }
            count = try Self.calculate(source: request.source.text, options: request.options)
            resolvedRequest = request
        } catch is CancellationError { return }
        catch { if requestID == id { self.error = error.localizedDescription; resolvedRequest = request } }
    }
}

struct DraftPageGoalView: View {
    @Binding var target: Int?
    let source: String
    let options: ScreenplayPrintOptions
    @StateObject private var pages = DraftPageCount()
    private var request: PageGoalRequest { PageGoalRequest(source: ExactSource(source), options: options, enabled: target != nil) }
    var body: some View {
        let current = pages.resolvedRequest == request
        DraftGoalView(target: $target, words: current ? (pages.count ?? 0) : 0, unit: "pages",
                      unavailable: current && pages.count != nil ? nil : (current ? pages.error : nil) ?? "Counting pages…")
            .task(id: request) { await pages.refresh(request) }
    }
}
