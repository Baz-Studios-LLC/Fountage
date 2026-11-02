import Foundation
import CoreGraphics
import AppKit

struct ScreenplayPrintOptions: Equatable {
    enum Paper: String, CaseIterable, Identifiable {
        case letter, a4
        var id: String { rawValue }
        var name: String { self == .letter ? "US Letter" : "A4" }
        var size: CGSize { self == .letter ? CGSize(width: 612, height: 792) : CGSize(width: 595.2756, height: 841.8898) }
    }
    static let fontFamilies: [String] = Array(Set(NSFontManager.shared.availableFontFamilies + ["Courier"])).sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    private static let installedFamilies = Set(fontFamilies)
    static func availableFamily(_ family: String?) -> String {
        guard let family, installedFamilies.contains(family) else { return "Courier" }
        return family
    }
    var bodyFontFamily = "Courier"
    var headingFontFamily = "Courier"
    var paper = Paper.letter
    var includeTitlePage = true
    var boldSceneHeadings = true
    var underlineSceneHeadings = false
    var doubleSpaceSceneHeadings = false
    var dialogueContinuations = true
    var includeSynopses = false
    var includeTodos = true
    var includeCompletedTodos = true
    var visibleSectionLevels: Set<Int> = []

    init() {}
    init(defaults: UserDefaults) {
        bodyFontFamily = Self.availableFamily(defaults.string(forKey: "screenplayBodyFontFamily"))
        headingFontFamily = Self.availableFamily(defaults.string(forKey: "screenplayHeadingFontFamily"))
        paper = Paper(rawValue: defaults.string(forKey: "screenplayPaper") ?? "") ?? .letter
        func bool(_ key: String, fallback: Bool) -> Bool { defaults.object(forKey: key) as? Bool ?? fallback }
        includeSynopses = bool("screenplayIncludeSynopses", fallback: false)
        includeTodos = bool("screenplayIncludeTodos", fallback: true)
        includeCompletedTodos = bool("screenplayIncludeCompletedTodos", fallback: true)
        visibleSectionLevels = Set((defaults.array(forKey: "screenplayVisibleSectionLevels") as? [Int] ?? []).filter { (1...6).contains($0) })
        includeTitlePage = bool("screenplayIncludeTitlePage", fallback: true)
        boldSceneHeadings = bool("screenplayBoldSceneHeadings", fallback: true)
        underlineSceneHeadings = bool("screenplayUnderlineSceneHeadings", fallback: false)
        doubleSpaceSceneHeadings = bool("screenplayDoubleSpaceSceneHeadings", fallback: false)
        dialogueContinuations = bool("screenplayDialogueContinuations", fallback: true)
    }
    var fields: [String: Any] {
        ["paper": paper.rawValue, "bodyFontFamily": bodyFontFamily, "headingFontFamily": headingFontFamily,
         "includeTitlePage": includeTitlePage, "boldSceneHeadings": boldSceneHeadings,
         "underlineSceneHeadings": underlineSceneHeadings, "doubleSpaceSceneHeadings": doubleSpaceSceneHeadings,
         "dialogueContinuations": dialogueContinuations, "includeSynopses": includeSynopses,
         "visibleSectionLevels": visibleSectionLevels.sorted(), "includeTodos": includeTodos, "includeCompletedTodos": includeCompletedTodos]
    }
    init(fields: [String: Any], fallback: ScreenplayPrintOptions) {
        self = fallback
        if let value = fields["paper"] as? String, let parsed = Paper(rawValue: value) { paper = parsed }
        if let value = fields["bodyFontFamily"] as? String { bodyFontFamily = Self.availableFamily(value) }
        if let value = fields["headingFontFamily"] as? String { headingFontFamily = Self.availableFamily(value) }
        func bool(_ key: String, _ fallback: Bool) -> Bool {
            guard let value = fields[key], let data = try? JSONSerialization.data(withJSONObject: [value]),
                  let parsed = try? JSONDecoder().decode([Bool].self, from: data).first else { return fallback }
            return parsed
        }
        includeTitlePage = bool("includeTitlePage", includeTitlePage)
        boldSceneHeadings = bool("boldSceneHeadings", boldSceneHeadings)
        underlineSceneHeadings = bool("underlineSceneHeadings", underlineSceneHeadings)
        doubleSpaceSceneHeadings = bool("doubleSpaceSceneHeadings", doubleSpaceSceneHeadings)
        dialogueContinuations = bool("dialogueContinuations", dialogueContinuations)
        includeSynopses = bool("includeSynopses", includeSynopses)
        includeTodos = bool("includeTodos", includeTodos)
        includeCompletedTodos = bool("includeCompletedTodos", includeCompletedTodos)
        if let value = fields["visibleSectionLevels"], let data = try? JSONSerialization.data(withJSONObject: value),
           let parsed = try? JSONDecoder().decode([Int].self, from: data) {
            visibleSectionLevels = Set(parsed.filter { (1...6).contains($0) })
        }
    }
    func save(to defaults: UserDefaults) {
        defaults.set(bodyFontFamily, forKey: "screenplayBodyFontFamily")
        defaults.set(headingFontFamily, forKey: "screenplayHeadingFontFamily")
        defaults.set(includeSynopses, forKey: "screenplayIncludeSynopses")
        defaults.set(includeTodos, forKey: "screenplayIncludeTodos")
        defaults.set(includeCompletedTodos, forKey: "screenplayIncludeCompletedTodos")
        defaults.set(visibleSectionLevels.sorted(), forKey: "screenplayVisibleSectionLevels")
        defaults.set(paper.rawValue, forKey: "screenplayPaper")
        defaults.set(includeTitlePage, forKey: "screenplayIncludeTitlePage")
        defaults.set(boldSceneHeadings, forKey: "screenplayBoldSceneHeadings")
        defaults.set(underlineSceneHeadings, forKey: "screenplayUnderlineSceneHeadings")
        defaults.set(doubleSpaceSceneHeadings, forKey: "screenplayDoubleSpaceSceneHeadings")
        defaults.set(dialogueContinuations, forKey: "screenplayDialogueContinuations")
    }
}
