import Foundation
import FountageCore

// The same semantic document will feed pagination, preview and exports.
// Output failures must never look like a successful empty export.
struct ScreenplayOutput: Decodable {
    struct Style: Decodable, Hashable { let bold: Bool; let italic: Bool; let underline: Bool }
    struct Run: Decodable, Hashable { let start: Int; let length: Int; let style: Style }
    struct Line: Decodable {
        let source_line: Int
        let kind: ElementKind
        let text: String
        let runs: [Run]
        let scene_number: String?
        let section_level: Int?
        let dual_dialogue: Bool
        var todo_completed: Bool? = nil
    }
    struct TitleField: Decodable {
        enum Placement: String, Decodable { case center, lowerLeft, metadata }
        let key: String
        let placement: Placement
        let values: [Line]
    }
    let title_fields: [TitleField]
    let title: [Line]
    let body: [Line]
    let outline: [Line]
    enum OutputError: LocalizedError {
        case invalidSource, engineFailure
        var errorDescription: String? {
            switch self {
            case .invalidSource: "The manuscript contains a null character and cannot be formatted."
            case .engineFailure: "Fountage could not format this manuscript. Your source has not been changed."
            }
        }
    }
    static func parse(_ source: String) throws -> ScreenplayOutput {
        guard !source.utf8.contains(0) else { throw OutputError.invalidSource }
        return try source.withCString { input in
            guard let output = fountage_screenplay_output(input) else { throw OutputError.engineFailure }
            defer { fountage_string_free(output) }
            return try JSONDecoder().decode(Self.self, from: Data(String(cString: output).utf8))
        }
    }
}

/// The engine shares the exact same parse between editing indexes and pagination.
struct ScreenplaySnapshot: Decodable {
    let analysis: ManuscriptAnalysis
    let output: ScreenplayOutput
    static func parse(_ source: String) throws -> ScreenplaySnapshot {
        guard !source.utf8.contains(0) else { throw ScreenplayOutput.OutputError.invalidSource }
        return try source.withCString { input in
            guard let pointer = fountage_screenplay_snapshot(input) else { throw ScreenplayOutput.OutputError.engineFailure }
            defer { fountage_string_free(pointer) }
            return try JSONDecoder().decode(Self.self, from: Data(String(cString: pointer).utf8))
        }
    }
}
