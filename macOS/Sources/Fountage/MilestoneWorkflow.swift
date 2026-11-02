import Foundation

/// Keep text and newly reached history in one document assignment, so automatic
/// document Undo never gets a second metadata-only edit after a keystroke.
@MainActor enum MilestoneWorkflow {
    static func counts(in document: ScreenplayDocument, cache: ManuscriptAnalysisCache, includePages: Bool) -> WritingMilestoneCounts {
        guard !document.text.utf8.contains(0) else { return WritingMilestoneCounts(words: 0, characters: 0, pages: nil) }
        if includePages && document.format == .screenplay,
           let snapshot = try? cache.screenplay(for: document.text) {
            let pages = try? DraftPageCount.calculate(output: snapshot.output, options: document.outputOptions())
            return WritingMilestoneCounts(words: snapshot.analysis.words, characters: snapshot.analysis.character_count, pages: pages)
        }
        let analysis = cache.analysis(for: document.text, format: document.format)
        return WritingMilestoneCounts(words: analysis.words, characters: analysis.character_count, pages: nil)
    }
    static func recordProgress(in document: inout ScreenplayDocument, cache: ManuscriptAnalysisCache, now: Date = Date()) {
        guard !document.text.utf8.contains(0), let milestones = try? document.writingMilestones(), milestones.contains(where: { !$0.completed }) else { return }
        let counts = counts(in: document, cache: cache, includePages: milestones.contains { !$0.completed && $0.unit == .pages })
        // Unsupported records must never prevent manuscript editing or lose data.
        // The milestone panel reports their read error without replacing the record.
        try? document.advanceMilestones(counts: counts, now: now)
    }
    struct FormatChangeError: LocalizedError {
        var errorDescription: String? {
            "This project’s application metadata uses an unsupported format. The document type has not changed, and the original metadata has been preserved."
        }
    }
    /// Category and newly reached milestones belong to one native document edit.
    @discardableResult
    static func changeFormat(_ format: DocumentFormat, in document: inout ScreenplayDocument,
                             cache: ManuscriptAnalysisCache, now: Date = Date()) throws -> Bool {
        guard format != document.format else { return false }
        guard document.metadata[ScreenplayDocument.namespace] == nil || document.metadata[ScreenplayDocument.namespace] is [String: Any] else {
            throw FormatChangeError()
        }
        document.format = format
        recordProgress(in: &document, cache: cache, now: now)
        return true
    }
    static func replaceText(_ source: String, in document: inout ScreenplayDocument, cache: ManuscriptAnalysisCache, now: Date = Date()) {
        guard !document.text.utf8.elementsEqual(source.utf8) else { return }
        document.text = source
        recordProgress(in: &document, cache: cache, now: now)
    }
}
