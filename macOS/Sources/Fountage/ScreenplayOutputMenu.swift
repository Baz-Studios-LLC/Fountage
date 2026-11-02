import SwiftUI
import AppKit

/// Native menu targets commit options synchronously before the next app command.
struct ScreenplayOutputMenu: NSViewRepresentable {
    @ObservedObject var model: ScreenplayPreviewModel
    func makeCoordinator() -> Coordinator { Coordinator(model: model) }
    func makeNSView(context: Context) -> NSPopUpButton {
        let button = NSPopUpButton(frame: .zero, pullsDown: true)
        button.bezelStyle = .rounded
        button.setAccessibilityLabel("Output Options")
        button.menu = context.coordinator.makeMenu()
        button.sizeToFit()
        return button
    }
    func updateNSView(_ button: NSPopUpButton, context: Context) {
        context.coordinator.model = model
        if let menu = button.menu { context.coordinator.refresh(menu) }
    }
    @MainActor final class Coordinator: NSObject, NSMenuDelegate {
        enum Command: Int { case letter = 1, a4, title, bold, underline, spacing, continuations, synopses, level1, level2, level3, level4, level5, level6, todos, completedTodos }
        var model: ScreenplayPreviewModel
        init(model: ScreenplayPreviewModel) { self.model = model }
        func item(_ title: String, _ command: Command) -> NSMenuItem {
            let item = NSMenuItem(title: title, action: #selector(choose(_:)), keyEquivalent: "")
            item.target = self; item.tag = command.rawValue
            return item
        }
        func makeMenu() -> NSMenu {
            let menu = NSMenu(); menu.autoenablesItems = false; menu.delegate = self
            let heading = NSMenuItem(title: "Output Options", action: nil, keyEquivalent: "")
            heading.image = NSImage(systemSymbolName: "slider.horizontal.3", accessibilityDescription: nil)
            menu.addItem(heading)
            let paper = NSMenuItem(title: "Paper Size", action: nil, keyEquivalent: "")
            let submenu = NSMenu(); submenu.autoenablesItems = false
            submenu.addItem(item("US Letter", .letter)); submenu.addItem(item("A4", .a4))
            paper.submenu = submenu; menu.addItem(paper)
            for (label, tag) in [("Heading Font", 100), ("Body Font", 101)] {
                let parent = NSMenuItem(title: label, action: nil, keyEquivalent: "")
                let fonts = NSMenu(); fonts.autoenablesItems = false
                for family in ScreenplayPrintOptions.fontFamilies {
                    let font = NSMenuItem(title: family, action: #selector(chooseFont(_:)), keyEquivalent: "")
                    font.target = self; font.tag = tag; font.representedObject = family
                    fonts.addItem(font)
                }
                parent.submenu = fonts; menu.addItem(parent)
            }
            menu.addItem(item("Title Page", .title))
            menu.addItem(item("Synopses", .synopses))
            menu.addItem(item("All To-Do Items", .todos))
            menu.addItem(item("Completed To-Do Items", .completedTodos))
            let sections = NSMenuItem(title: "Section Levels", action: nil, keyEquivalent: "")
            let levels = NSMenu(); levels.autoenablesItems = false
            for level in 1...6 {
                levels.addItem(item("Level \(level) (\(String(repeating: "#", count: level)))", Command(rawValue: Command.level1.rawValue + level - 1)!))
            }
            sections.submenu = levels; menu.addItem(sections); menu.addItem(.separator())
            menu.addItem(item("Bold Scene Headings", .bold))
            menu.addItem(item("Underline Scene Headings", .underline))
            menu.addItem(item("Double-Space Scene Headings", .spacing)); menu.addItem(.separator())
            menu.addItem(item("Dialogue Continuations", .continuations))
            refresh(menu)
            return menu
        }
        func menuNeedsUpdate(_ menu: NSMenu) { refresh(menu) }
        func refresh(_ menu: NSMenu) {
            for item in menu.items {
                if let submenu = item.submenu { refresh(submenu) }
                if let family = item.representedObject as? String, item.tag == 100 || item.tag == 101 {
                    item.state = family == (item.tag == 100 ? model.options.headingFontFamily : model.options.bodyFontFamily) ? .on : .off
                    continue
                }
                guard let command = Command(rawValue: item.tag) else { continue }
                let on: Bool
                switch command {
                case .letter: on = model.options.paper == .letter
                case .a4: on = model.options.paper == .a4
                case .title: on = model.options.includeTitlePage
                case .bold: on = model.options.boldSceneHeadings
                case .underline: on = model.options.underlineSceneHeadings
                case .spacing: on = model.options.doubleSpaceSceneHeadings
                case .continuations: on = model.options.dialogueContinuations
                case .synopses: on = model.options.includeSynopses
                case .todos: on = model.options.includeTodos
                case .completedTodos: on = model.options.includeCompletedTodos
                case .level1, .level2, .level3, .level4, .level5, .level6:
                    on = model.options.visibleSectionLevels.contains(command.rawValue - Command.level1.rawValue + 1)
                }
                item.state = on ? .on : .off
            }
        }
        @objc func chooseFont(_ sender: NSMenuItem) {
            guard let family = sender.representedObject as? String, [100, 101].contains(sender.tag) else { return }
            model.updateOptions {
                if sender.tag == 100 { $0.headingFontFamily = ScreenplayPrintOptions.availableFamily(family) }
                else { $0.bodyFontFamily = ScreenplayPrintOptions.availableFamily(family) }
            }
            if let menu = sender.menu { refresh(menu) }
        }
        @objc func choose(_ sender: NSMenuItem) {
            guard let command = Command(rawValue: sender.tag) else { return }
            model.updateOptions { options in
                switch command {
                case .letter: options.paper = .letter
                case .a4: options.paper = .a4
                case .title: options.includeTitlePage.toggle()
                case .bold: options.boldSceneHeadings.toggle()
                case .underline: options.underlineSceneHeadings.toggle()
                case .spacing: options.doubleSpaceSceneHeadings.toggle()
                case .continuations: options.dialogueContinuations.toggle()
                case .synopses: options.includeSynopses.toggle()
                case .todos: options.includeTodos.toggle()
                case .completedTodos: options.includeCompletedTodos.toggle()
                case .level1, .level2, .level3, .level4, .level5, .level6:
                    let level = command.rawValue - Command.level1.rawValue + 1
                    if options.visibleSectionLevels.contains(level) { options.visibleSectionLevels.remove(level) }
                    else { options.visibleSectionLevels.insert(level) }
                }
            }
            if let menu = sender.menu { refresh(menu) }
        }
    }
}
