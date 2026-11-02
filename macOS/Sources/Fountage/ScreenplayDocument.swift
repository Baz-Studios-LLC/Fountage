import SwiftUI
import UniformTypeIdentifiers

extension UTType {
    static let fountain = UTType(importedAs: "com.fountain.screenplay", conformingTo: .plainText)
    static let fountage = UTType(exportedAs: "com.bazstudios.fountage.project", conformingTo: .package)
}

enum ProjectError: LocalizedError {
    case invalid(String)
    case invalidManuscript
    var errorDescription: String? {
        switch self {
        case .invalid(let reason): "This project could not be opened: \(reason)"
        case .invalidManuscript: "The manuscript contains a null character and could not be saved."
        }
    }
    var recoverySuggestion: String? {
        switch self {
        case .invalid: nil
        case .invalidManuscript: "Undo the recent paste or remove the null character, then save again. Your previously saved file has not been replaced."
        }
    }
}

struct ScreenplayDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.fountage, .fountain, .plainText] }
    static var writableContentTypes: [UTType] { [.fountage, .fountain, .plainText] }
    static let namespace = "com.bazstudios.fountage"
    var text = ""
    // Session-only: existing documents never reopen in the new-document chooser.
    var needsTypeSelection = true
    // Preserve foreign metadata and entries verbatim where possible. Never rebuild a project from only known fields.
    private var metadataData: Data = Data()
    var metadata: [String: Any] {
        get { (try? JSONSerialization.jsonObject(with: metadataData) as? [String: Any]) ?? [:] }
        set {
            precondition(JSONSerialization.isValidJSONObject(newValue), "Metadata must contain only JSON values")
            metadataData = try! JSONSerialization.data(withJSONObject: newValue, options: [.sortedKeys])
        }
    }
    var retainedEntries: [String: ProjectEntry] = [:]
    private var storedSourceFilename: String?
    var format: DocumentFormat {
        get {
            let own = metadata[Self.namespace] as? [String: Any] ?? [:]
            return DocumentFormat(rawValue: own["documentFormat"] as? String ?? "screenplay") ?? .screenplay
        }
        set {
            var info = metadata
            var own = info[Self.namespace] as? [String: Any] ?? [:]
            own["documentFormat"] = newValue.rawValue
            own["layoutPreset"] = newValue.presets[0].rawValue
            info[Self.namespace] = own
            metadata = info
        }
    }
    var preset: LayoutPreset {
        get {
            let own = metadata[Self.namespace] as? [String: Any] ?? [:]
            guard let value = own["layoutPreset"] as? String, let preset = LayoutPreset(rawValue: value), format.presets.contains(preset) else { return format.presets[0] }
            return preset
        }
        set {
            guard format.presets.contains(newValue) else { return }
            var info = metadata
            var own = info[Self.namespace] as? [String: Any] ?? [:]
            own["layoutPreset"] = newValue.rawValue
            info[Self.namespace] = own
            metadata = info
        }
    }

    var draftWordGoal: Int? {
        get { draftGoal(for: "draftWords") }
        set { setDraftGoal(newValue, for: "draftWords") }
    }
    var draftCharacterGoal: Int? {
        get { draftGoal(for: "draftCharacters") }
        set { setDraftGoal(newValue, for: "draftCharacters") }
    }
    var draftPageGoal: Int? {
        get { draftGoal(for: "draftPages") }
        set { setDraftGoal(newValue, for: "draftPages") }
    }
    private func draftGoal(for key: String) -> Int? {
        let own = metadata[Self.namespace] as? [String: Any] ?? [:]
        guard let goals = own["goals"] as? [String: Any], let value = goals[key],
              let data = try? JSONSerialization.data(withJSONObject: [value]),
              let target = try? JSONDecoder().decode([Int].self, from: data).first,
              DraftGoal.validTargets.contains(target) else { return nil }
        return target
    }
    private mutating func setDraftGoal(_ target: Int?, for key: String) {
        guard target == nil || DraftGoal.validTargets.contains(target!) else { return }
        var info = metadata
        var own = info[Self.namespace] as? [String: Any] ?? [:]
        // Preserve unsupported future records rather than replacing them.
        guard own["goals"] == nil || own["goals"] is [String: Any] else { return }
        var goals = own["goals"] as? [String: Any] ?? [:]
        if let target { goals[key] = target } else { goals.removeValue(forKey: key) }
        own["goals"] = goals; info[Self.namespace] = own; metadata = info
    }

    var academicStyle: AcademicStyle? {
        get {
            let own = metadata[Self.namespace] as? [String: Any] ?? [:]
            guard let style = own["academicStyle"] as? [String: Any],
                  let family = style["family"] as? String, let edition = style["edition"] as? Int,
                  let variant = style["variant"] as? String else { return nil }
            return AcademicStyle(family: family, edition: edition, variant: variant)
        }
        set {
            var info = metadata
            var own = info[Self.namespace] as? [String: Any] ?? [:]
            if let style = newValue {
                var fields = own["academicStyle"] as? [String: Any] ?? [:]
                fields["family"] = style.family
                fields["edition"] = style.edition
                fields["variant"] = style.variant
                own["academicStyle"] = fields
            } else { own.removeValue(forKey: "academicStyle") }
            info[Self.namespace] = own
            metadata = info
        }
    }

    init() {
        metadata = [
            "version": 2, "type": "com.fountain.screenplay", "creatorIdentifier": Self.namespace,
            Self.namespace: [
                "version": 2, "documentFormat": "screenplay", "layoutPreset": "screenplay-standard", "sourceFile": "text.fountain", "projectID": UUID().uuidString,
                "createdAt": ISO8601DateFormatter().string(from: Date()),
                "authors": [String](), "settings": [String: String](),
                "characters": [String: String](), "notes": [String](),
                "revisions": [String](), "snippets": [String](), "goals": [String: String]()
            ] as [String: Any]
        ]
    }
    init(configuration: ReadConfiguration) throws {
        try self.init(wrapper: configuration.file, contentType: configuration.contentType)
    }
    init(wrapper: FileWrapper, contentType: UTType) throws {
        self.init()
        needsTypeSelection = false
        if contentType.conforms(to: .fountage) {
            guard wrapper.isDirectory, let files = wrapper.fileWrappers,
                  files["info.json"]?.isRegularFile == true,
                  let infoData = files["info.json"]?.regularFileContents,
                  let info = try JSONSerialization.jsonObject(with: infoData) as? [String: Any],
                  let own = info[Self.namespace] as? [String: Any] else {
                throw ProjectError.invalid("the project metadata is missing.")
            }
            guard let version = own["version"] as? Int, [1, 2].contains(version) else {
                throw ProjectError.invalid("this project uses an unsupported metadata version. Open it with a compatible version of Fountage.")
            }
            let format: DocumentFormat
            if version == 1 { format = .screenplay }
            else {
                guard let value = own["documentFormat"] as? String, let parsed = DocumentFormat(rawValue: value),
                      let presetValue = own["layoutPreset"] as? String,
                      let preset = LayoutPreset(rawValue: presetValue), parsed.presets.contains(preset) else {
                    throw ProjectError.invalid("the document format or layout preset is not supported.")
                }
                format = parsed
            }
            let filename = version == 1 ? "text.fountain" : (own["sourceFile"] as? String ?? "")
            guard filename == format.sourceFilename, files[filename]?.isRegularFile == true,
                  let textData = files[filename]?.regularFileContents else {
                throw ProjectError.invalid("the project's manuscript is missing or invalid.")
            }
            guard info["version"] as? Int == 2, info["type"] as? String == format.sourceType,
                  let identifier = own["projectID"] as? String, UUID(uuidString: identifier) != nil else {
                throw ProjectError.invalid("the project identity or container metadata is invalid.")
            }
            text = try Self.decode(textData)
            metadata = info
            storedSourceFilename = filename
            retainedEntries = try files.mapValues { try ProjectEntry(wrapper: $0) }
        } else {
            guard wrapper.isRegularFile, let data = wrapper.regularFileContents else { throw CocoaError(.fileReadCorruptFile) }
            text = try Self.decode(data)
            if !contentType.conforms(to: .fountain) { format = .novel }
        }
    }
    private static func decode(_ data: Data) throws -> String {
        guard let text = String(data: data, encoding: .utf8), !text.contains("\0") else {
            throw CocoaError(.fileReadInapplicableStringEncoding)
        }
        return text
    }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        try makeWrapper(contentType: configuration.contentType)
    }
    func makeWrapper(contentType: UTType) throws -> FileWrapper {
        // The reader rejects NUL. Reject it before producing a save snapshot too,
        // so pasted binary content cannot replace a readable manuscript on disk.
        guard !text.contains("\0") else { throw ProjectError.invalidManuscript }
        if !contentType.conforms(to: .fountage) {
            if contentType.conforms(to: .fountain) && format != .screenplay {
                throw ProjectError.invalid("this document type should be saved as a Fountage project or plain text.")
            }
            return FileWrapper(regularFileWithContents: Data(text.utf8))
        }
        var info = metadata
        var own = info[Self.namespace] as? [String: Any] ?? [:]
        own["version"] = 2
        own["documentFormat"] = format.rawValue
        own["layoutPreset"] = preset.rawValue
        own["sourceFile"] = format.sourceFilename
        info["type"] = format.sourceType
        own["modifiedAt"] = ISO8601DateFormatter().string(from: Date())
        info[Self.namespace] = own
        var files = retainedEntries.mapValues { $0.fileWrapper() }
        if let previous = storedSourceFilename, previous != format.sourceFilename { files.removeValue(forKey: previous) }
        if storedSourceFilename != format.sourceFilename && retainedEntries[format.sourceFilename] != nil {
            throw ProjectError.invalid("an existing attachment uses the new manuscript filename. Rename that attachment before changing document type.")
        }
        files[format.sourceFilename] = FileWrapper(regularFileWithContents: Data(text.utf8))
        files["info.json"] = FileWrapper(regularFileWithContents: try JSONSerialization.data(withJSONObject: info, options: [.sortedKeys, .prettyPrinted]))
        if files["assets"] == nil { files["assets"] = FileWrapper(directoryWithFileWrappers: [:]) }
        return FileWrapper(directoryWithFileWrappers: files)
    }
}

// A value snapshot lets document writes occur off the main thread without sharing mutable FileWrappers.
indirect enum ProjectEntry: Sendable {
    case file(Data)
    case directory([String: ProjectEntry])
    case symbolicLink(URL)
    init(wrapper: FileWrapper) throws {
        if wrapper.isRegularFile, let data = wrapper.regularFileContents { self = .file(data) }
        else if wrapper.isDirectory, let entries = wrapper.fileWrappers { self = .directory(try entries.mapValues { try ProjectEntry(wrapper: $0) }) }
        else if wrapper.isSymbolicLink, let destination = wrapper.symbolicLinkDestinationURL { self = .symbolicLink(destination) }
        else { throw ProjectError.invalid("an attachment could not be preserved.") }
    }
    func fileWrapper() -> FileWrapper {
        switch self {
        case .file(let data): FileWrapper(regularFileWithContents: data)
        case .directory(let entries): FileWrapper(directoryWithFileWrappers: entries.mapValues { $0.fileWrapper() })
        case .symbolicLink(let destination): FileWrapper(symbolicLinkWithDestinationURL: destination)
        }
    }
}
