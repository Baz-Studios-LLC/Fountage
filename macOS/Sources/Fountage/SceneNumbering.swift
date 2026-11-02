import Foundation
import FountageCore

struct SceneNumbering {
    struct Patch: Decodable { let start: Int; let length: Int; let replacement: String }
    struct Change: Decodable { let start: Int; let length: Int; let replacement: String; let patches: [Patch] }
    struct Response: Decodable { let change: Change?; let error: String? }
    struct NumberingError: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }
    static func edit(source: String, selection: NSRange, remove: Bool, firstNumber: UInt32 = 1, selectedOnly: Bool = false) throws -> ManuscriptEdit? {
        guard !source.contains("\0"), firstNumber > 0, Range(selection, in: source) != nil else {
            throw NumberingError(message: "The source, selection or starting number is invalid. No text was changed.")
        }
        if selectedOnly && selection.length == 0 { throw NumberingError(message: "Select scene headings to number first. No text was changed.") }
        let response = try source.withCString { input -> Response in
            let pointer = selectedOnly
                ? fountage_scene_numbers_range(input, firstNumber, remove ? 1 : 0, selection.location, selection.length)
                : fountage_scene_numbers(input, firstNumber, remove ? 1 : 0)
            guard let result = pointer else {
                throw NumberingError(message: "Fountage could not number this screenplay. No text was changed.")
            }
            defer { fountage_string_free(result) }
            return try JSONDecoder().decode(Response.self, from: Data(String(cString: result).utf8))
        }
        if let error = response.error { throw NumberingError(message: error) }
        guard let change = response.change else { return nil }
        func mapped(_ position: Int) -> Int {
            var shift = 0
            for patch in change.patches {
                if position < patch.start { break }
                let size = patch.replacement.utf16.count
                if position < patch.start + patch.length { return patch.start + shift + min(position - patch.start, size) }
                shift += size - patch.length
            }
            return position + shift
        }
        let start = mapped(selection.location), end = mapped(NSMaxRange(selection))
        return ManuscriptEdit(source: source, range: NSRange(location: change.start, length: change.length), replacement: change.replacement,
                              selection: NSRange(location: start, length: max(0, end - start)), actionName: remove ? "Remove Scene Numbers" : "Add Scene Numbers")
    }
}
