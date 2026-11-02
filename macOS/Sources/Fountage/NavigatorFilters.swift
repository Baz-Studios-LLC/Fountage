import Foundation

struct NavigatorFilters {
    static let known: Set<ElementKind> = [.todo, .note, .marker, .section, .scene, .synopsis, .pageBreak, .page, .panel, .chapter, .heading, .act, .verse]
    static func kinds(for format: DocumentFormat) -> [ElementKind] {
        let structural: [ElementKind] = switch format {
        case .screenplay: [.section, .scene, .synopsis, .note, .pageBreak]
        case .comic: [.page, .panel]
        case .novel: [.chapter]
        case .academic, .general: [.heading]
        case .stageplay: [.act, .scene]
        case .audio: [.scene, .heading]
        case .poetry: [.heading, .verse]
        }
        return structural + [.marker, .todo]
    }
    static func label(_ kind: ElementKind, format: DocumentFormat) -> String {
        switch kind {
        case .section, .heading: "Sections"
        case .scene: format == .audio ? "Scenes & Segments" : "Scenes"
        case .synopsis: "Synopses"
        case .pageBreak: "Page Breaks"
        case .marker: "Markers"
        case .note: "Notes"
        case .todo: "Checklists"
        case .page: "Pages"
        case .panel: "Panels"
        case .chapter: "Chapters"
        case .act: "Acts"
        case .verse: "Parts"
        default: kind.rawValue.capitalized
        }
    }
    struct SettingsError: LocalizedError {
        var errorDescription: String? { "This project contains unsupported Navigator settings. They have been preserved; no filters were changed." }
    }
}

extension ScreenplayDocument {
    var navigatorHiddenKinds: Set<ElementKind> {
        let own = metadata[Self.namespace] as? [String: Any] ?? [:]
        let settings = own["navigatorFilters"] as? [String: Any] ?? [:]
        return Set((settings["hiddenKinds"] as? [String] ?? []).compactMap(ElementKind.init(rawValue:))).intersection(NavigatorFilters.known)
    }
    mutating func setNavigatorHiddenKinds(_ kinds: Set<ElementKind>) throws {
        var info = metadata, own = info[Self.namespace] as? [String: Any] ?? [:]
        let existing = own["navigatorFilters"]
        guard existing == nil || existing is [String: Any] else { throw NavigatorFilters.SettingsError() }
        var settings = existing as? [String: Any] ?? [:]
        if let version = settings["version"] {
            guard let data = try? JSONSerialization.data(withJSONObject: [version]),
                  let versions = try? JSONDecoder().decode([Int].self, from: data), versions == [1] else { throw NavigatorFilters.SettingsError() }
        }
        guard settings["hiddenKinds"] == nil || settings["hiddenKinds"] is [String] else { throw NavigatorFilters.SettingsError() }
        let old = settings["hiddenKinds"] as? [String] ?? []
        let future = old.filter { ElementKind(rawValue: $0).map { !NavigatorFilters.known.contains($0) } ?? true }
        let wanted = Set(future).union(kinds.intersection(NavigatorFilters.known).map(\.rawValue)).sorted()
        guard Set(old) != Set(wanted) else { return }
        settings["version"] = 1; settings["hiddenKinds"] = wanted
        own["navigatorFilters"] = settings; info[Self.namespace] = own; metadata = info
    }
}
