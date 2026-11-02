import SwiftUI

struct OutputSettingsError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

extension ScreenplayDocument {
    var outputSettingsRecord: Data? {
        let own = metadata[Self.namespace] as? [String: Any] ?? [:]
        guard let value = own["screenplayOutput"] else { return nil }
        return try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed])
    }
    func outputOptions() -> ScreenplayPrintOptions {
        let own = metadata[Self.namespace] as? [String: Any] ?? [:]
        guard let fields = own["screenplayOutput"] as? [String: Any] else { return ScreenplayPrintOptions() }
        return ScreenplayPrintOptions(fields: fields, fallback: .init())
    }
    mutating func setOutputOptions(_ options: ScreenplayPrintOptions, previous: ScreenplayPrintOptions) throws {
        var own = metadata[Self.namespace] as? [String: Any] ?? [:]
        let existing = own["screenplayOutput"]
        guard existing == nil || existing is [String: Any] else {
            throw OutputSettingsError(message: "This project contains output settings in an unsupported format. They have been preserved. No settings were changed.")
        }
        var fields = existing as? [String: Any] ?? [:]
        if let version = fields["version"] {
            guard let data = try? JSONSerialization.data(withJSONObject: [version]),
                  let decoded = try? JSONDecoder().decode([Int].self, from: data), decoded == [1] else {
                throw OutputSettingsError(message: "This project uses a newer or unsupported output-settings version. No settings were changed.")
            }
        }
        for (key, value) in options.fields {
            // Keep unknown or malformed unedited fields verbatim.
            if existing == nil || !NSDictionary(dictionary: [key: value]).isEqual(to: [key: previous.fields[key]!]) { fields[key] = value }
        }
        fields["version"] = 1
        own["screenplayOutput"] = fields
        var info = metadata; info[Self.namespace] = own; metadata = info
    }
    mutating func restoreOutputSettings(_ record: Data?) throws {
        var info = metadata, own = info[Self.namespace] as? [String: Any] ?? [:]
        if let record { own["screenplayOutput"] = try JSONSerialization.jsonObject(with: record, options: .fragmentsAllowed) }
        else { own.removeValue(forKey: "screenplayOutput") }
        info[Self.namespace] = own; metadata = info
    }
}

/// Undo only the output-settings record, never a snapshot of the manuscript.
@MainActor final class ProjectOutputSettings: ObservableObject {
    func change(_ options: ScreenplayPrintOptions, previous: ScreenplayPrintOptions,
                document: Binding<ScreenplayDocument>, undoManager: UndoManager?) throws {
        var updated = document.wrappedValue
        let before = updated.outputSettingsRecord
        try updated.setOutputOptions(options, previous: previous)
        let after = updated.outputSettingsRecord
        guard before != after else { return }
        MilestoneWorkflow.recordProgress(in: &updated, cache: ManuscriptAnalysisCache())
        document.wrappedValue = updated
        register(record: before, expected: after, document: document, manager: undoManager)
    }
    private func register(record: Data?, expected: Data?, document: Binding<ScreenplayDocument>, manager: UndoManager?) {
        manager?.registerUndo(withTarget: self) { [weak manager] target in
            var updated = document.wrappedValue
            // Do not overwrite a newer setting edit from another owner.
            guard updated.outputSettingsRecord == expected else { NSSound.beep(); return }
            do {
                try updated.restoreOutputSettings(record)
                document.wrappedValue = updated
                target.register(record: expected, expected: record, document: document, manager: manager)
            } catch { NSSound.beep() }
        }
        manager?.setActionName("Change Output Settings")
    }
}
