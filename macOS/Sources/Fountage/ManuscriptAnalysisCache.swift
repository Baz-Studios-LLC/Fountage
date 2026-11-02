import Foundation

struct ExactSource: Equatable {
    let text: String
    init(_ text: String) { self.text = text }
    static func == (lhs: Self, rhs: Self) -> Bool { lhs.text.utf8.elementsEqual(rhs.text.utf8) }
}

// One snapshot per workspace, shared by the navigator and the display styler.
// Never retain a history of the writer's text. Byte equality matters: Swift String
// equality considers Unicode-normalized strings equal, but their UTF-16 ranges differ.
final class ManuscriptAnalysisCache {
    private let lock = NSLock()
    private let parse: (String, DocumentFormat) -> ManuscriptAnalysis
    private let parseSnapshot: (String) throws -> ScreenplaySnapshot
    private var output: ScreenplayOutput?
    private var source: String?
    private var format: DocumentFormat?
    private var result = ManuscriptAnalysis()

    init(parse: @escaping (String, DocumentFormat) -> ManuscriptAnalysis = { ManuscriptAnalysis.parse($0, format: $1) }) {
        self.parse = parse
        self.parseSnapshot = ScreenplaySnapshot.parse
    }
    init(parse: @escaping (String, DocumentFormat) -> ManuscriptAnalysis,
         snapshot: @escaping (String) throws -> ScreenplaySnapshot) {
        self.parse = parse; self.parseSnapshot = snapshot
    }

    func screenplay(for text: String) throws -> ScreenplaySnapshot {
        lock.lock()
        defer { lock.unlock() }
        if let source, format == .screenplay, source.utf8.elementsEqual(text.utf8), let output {
            return ScreenplaySnapshot(analysis: result, output: output)
        }
        let snapshot = try parseSnapshot(text)
        result = snapshot.analysis; output = snapshot.output; source = text; format = .screenplay
        return snapshot
    }

    func analysis(for text: String, format: DocumentFormat) -> ManuscriptAnalysis {
        lock.lock()
        defer { lock.unlock() }
        if let source, self.format == format, source.utf8.elementsEqual(text.utf8) { return result }
        result = parse(text, format)
        output = nil
        source = text
        self.format = format
        return result
    }
}
