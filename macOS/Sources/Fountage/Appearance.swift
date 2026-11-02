import SwiftUI
import AppKit

enum StudioAppearance: String, CaseIterable, Identifiable {
    case system, light, dark
    var id: String { rawValue }
    var name: String { switch self { case .system: "System"; case .light: "Light"; case .dark: "Dark" } }
    var colorScheme: ColorScheme? { switch self { case .system: nil; case .light: .light; case .dark: .dark } }
    var native: NSAppearance? { switch self { case .system: nil; case .light: NSAppearance(named: .aqua); case .dark: NSAppearance(named: .darkAqua) } }
}

// Paper is a reading preference, independent of the surrounding app's appearance.
enum EditorPaperAppearance: String, CaseIterable, Identifiable {
    case matchApp, white
    var id: String { rawValue }
    var name: String { self == .white ? "White Paper" : "Match App" }
    var native: NSAppearance? { self == .white ? NSAppearance(named: .aqua) : nil }
    func apply(to scroll: NSScrollView, editor: NSTextView) {
        scroll.appearance = native
        editor.appearance = native
        scroll.scrollerKnobStyle = self == .white ? .dark : .default
        editor.backgroundColor = self == .white ? .white : .textBackgroundColor
        editor.textColor = .textColor
        editor.insertionPointColor = StudioTheme.accent
    }
}

enum StudioLayout {
    static let manuscriptInset: CGFloat = 28
    static let sidebarInset: CGFloat = 16
}

enum StudioTheme {
    static let accent = NSColor(name: nil) { appearance in
        if appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua {
            return NSColor(srgbRed: 0.65, green: 0.79, blue: 0.69, alpha: 1)
        }
        return NSColor(srgbRed: 0.27, green: 0.39, blue: 0.32, alpha: 1)
    }
}
