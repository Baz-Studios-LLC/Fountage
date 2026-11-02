import Foundation
import FountageCore

struct Scene: Decodable, Identifiable, Hashable {
    let line: Int
    let heading: String
    var id: Int { line }
}
struct Outline: Decodable {
    var scenes: [Scene] = []
    var characters: [String: Int] = [:]
    var words: Int = 0
    static func parse(_ source: String) -> Outline {
        source.withCString { input in
            guard let output = fountage_outline(input) else { return Outline() }
            defer { fountage_string_free(output) }
            return (try? JSONDecoder().decode(Outline.self, from: Data(String(cString: output).utf8))) ?? Outline()
        }
    }
}

enum ElementKind: String, Decodable {
    case body, title, scene, character, dialogue, parenthetical, transition, note, section
    case page, panel, balloon, caption, soundEffect, chapter, sceneBreak, heading, act, verse
    case synopsis, centered, lyric, pageBreak, omitted, marker, todo
}
struct ManuscriptNote: Decodable, Identifiable {
    let line: Int
    let start: Int
    let length: Int
    let text: String
    var id: Int { start }
    var range: NSRange { NSRange(location: start, length: length) }
}
struct ManuscriptSourceRange: Decodable {
    let start: Int
    let length: Int
    var range: NSRange { NSRange(location: start, length: length) }
}
struct ManuscriptElement: Decodable { var line: Int; var kind: ElementKind }
struct ManuscriptHeading: Decodable, Identifiable { var line: Int; var heading: String; var kind: ElementKind; var level: Int; var id: Int { line } }
struct ManuscriptEmphasis: Decodable {
    var start: Int
    var length: Int
    var bold: Bool
    var italic: Bool
    var underline: Bool
}
struct ManuscriptAnalysis: Decodable {
    var word_ranges: [ManuscriptSourceRange] = []
    var emphasis: [ManuscriptEmphasis] = []
    var headings: [ManuscriptHeading] = []
    var notes: [ManuscriptNote] = []
    var markers: [ManuscriptNote] = []
    var todos: [ManuscriptTodo] = []
    var omissions: [ManuscriptSourceRange] = []
    var characters: [String: Int] = [:]
    var locations: [String: [Int]] = [:]
    var words = 0
    var character_count = 0
    var elements: [ManuscriptElement] = []
    var units = 0
    var panels = 0
    static func parse(_ source: String, format: DocumentFormat) -> ManuscriptAnalysis {
        source.withCString { input in
            guard let output = fountage_analyze(input, format.code) else { return ManuscriptAnalysis() }
            defer { fountage_string_free(output) }
            return (try? JSONDecoder().decode(Self.self, from: Data(String(cString: output).utf8))) ?? ManuscriptAnalysis()
        }
    }
}
