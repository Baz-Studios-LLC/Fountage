import SwiftUI
import AppKit

struct OutlineFilterMenu: NSViewRepresentable {
    let kinds: [ElementKind]
    let format: DocumentFormat
    let hidden: Set<ElementKind>
    let change: (Set<ElementKind>) -> Bool
    func makeCoordinator() -> Coordinator { Coordinator(kinds: kinds, format: format, hidden: hidden, change: change) }
    func makeNSView(context: Context) -> NSPopUpButton {
        let button = NSPopUpButton(frame: .zero, pullsDown: true)
        button.isBordered = false; button.controlSize = .small
        button.imagePosition = .imageOnly
        (button.cell as? NSPopUpButtonCell)?.arrowPosition = .noArrow
        button.setAccessibilityLabel("Filter Outline")
        button.toolTip = "Choose which entry types appear in the outline"
        button.menu = context.coordinator.makeMenu(); button.sizeToFit()
        return button
    }
    func updateNSView(_ button: NSPopUpButton, context: Context) {
        let coordinator = context.coordinator
        coordinator.hidden = hidden; coordinator.change = change
        if coordinator.kinds != kinds || coordinator.format != format {
            coordinator.kinds = kinds; coordinator.format = format; button.menu = coordinator.makeMenu()
        } else if let menu = button.menu { coordinator.refresh(menu) }
    }
    @MainActor final class Coordinator: NSObject, NSMenuDelegate {
        var kinds: [ElementKind]
        var format: DocumentFormat
        var hidden: Set<ElementKind>
        var change: (Set<ElementKind>) -> Bool
        init(kinds: [ElementKind], format: DocumentFormat, hidden: Set<ElementKind>, change: @escaping (Set<ElementKind>) -> Bool) {
            self.kinds = kinds; self.format = format; self.hidden = hidden; self.change = change
        }
        func makeMenu() -> NSMenu {
            let menu = NSMenu(); menu.autoenablesItems = false; menu.delegate = self
            menu.addItem(NSMenuItem(title: "Filter Outline", action: nil, keyEquivalent: ""))
            for (index, kind) in kinds.enumerated() {
                let item = NSMenuItem(title: NavigatorFilters.label(kind, format: format), action: #selector(choose(_:)), keyEquivalent: "")
                item.target = self; item.tag = index + 1; menu.addItem(item)
            }
            menu.addItem(.separator())
            for (title, tag) in [("Show All", -1), ("Hide All", -2)] {
                let item = NSMenuItem(title: title, action: #selector(choose(_:)), keyEquivalent: "")
                item.target = self; item.tag = tag; menu.addItem(item)
            }
            refresh(menu); return menu
        }
        func menuNeedsUpdate(_ menu: NSMenu) { refresh(menu) }
        func refresh(_ menu: NSMenu) {
            menu.items.first?.image = NSImage(systemSymbolName: hidden.intersection(kinds).isEmpty ? "line.3.horizontal.decrease.circle" : "line.3.horizontal.decrease.circle.fill", accessibilityDescription: nil)
            for item in menu.items {
                if kinds.indices.contains(item.tag - 1) { item.state = hidden.contains(kinds[item.tag - 1]) ? .off : .on }
                if item.tag == -1 { item.isEnabled = !hidden.intersection(kinds).isEmpty }
                if item.tag == -2 { item.isEnabled = !Set(kinds).isSubset(of: hidden) }
            }
        }
        @objc func choose(_ sender: NSMenuItem) {
            var desired = hidden
            if sender.tag == -1 { desired.subtract(kinds) }
            else if sender.tag == -2 { desired.formUnion(kinds) }
            else if kinds.indices.contains(sender.tag - 1) {
                let kind = kinds[sender.tag - 1]
                if desired.contains(kind) { desired.remove(kind) } else { desired.insert(kind) }
            } else { return }
            if desired != hidden && change(desired) { hidden = desired }
            if let menu = sender.menu { refresh(menu) }
        }
    }
}
