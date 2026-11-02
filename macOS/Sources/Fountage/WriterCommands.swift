import SwiftUI
import AppKit

struct WriterMenuState {
    var editable: Bool
    var screenplay: Bool
    var hasSelection: Bool
}
private struct WriterMenuKey: FocusedValueKey { typealias Value = WriterMenuState }
extension FocusedValues {
    var writerMenuState: WriterMenuState? {
        get { self[WriterMenuKey.self] }
        set { self[WriterMenuKey.self] = newValue }
    }
}
enum WriterMenuAction {
    @MainActor @discardableResult static func perform(_ selector: Selector, editor: ManuscriptTextView? = nil) -> Bool {
        guard let editor = editor ?? NSApp.keyWindow?.firstResponder as? ManuscriptTextView,
              editor.validateUserInterfaceItem(NSMenuItem(title: "", action: selector, keyEquivalent: "")) else { return false }
        return NSApp.sendAction(selector, to: editor, from: nil)
    }
}
struct WriterCommands: Commands {
    @FocusedValue(\.writerMenuState) private var state
    private var editable: Bool { state?.editable == true }
    private var screenplay: Bool { editable && state?.screenplay == true }
    var body: some Commands {
        CommandMenu("Format") {
            Button("Bold") { WriterMenuAction.perform(#selector(ManuscriptTextView.fountainBold(_:))) }.keyboardShortcut("b").disabled(!screenplay)
            Button("Italic") { WriterMenuAction.perform(#selector(ManuscriptTextView.fountainItalic(_:))) }.keyboardShortcut("i").disabled(!screenplay)
            Button("Underline") { WriterMenuAction.perform(#selector(ManuscriptTextView.fountainUnderline(_:))) }.keyboardShortcut("u").disabled(!screenplay)
            Menu("Convert Lines To") {
                Button("To Do List") { WriterMenuAction.perform(#selector(ManuscriptTextView.convertToTodoList(_:))) }
            }.disabled(!editable)
            Divider()
            Button("Insert Page Break") { WriterMenuAction.perform(#selector(ManuscriptTextView.insertFountainPageBreak(_:))) }.keyboardShortcut(.return).disabled(!screenplay)
            Divider()
            Button("Title Page…") { WriterMenuAction.perform(#selector(ManuscriptTextView.editTitlePage(_:))) }.disabled(!screenplay)
        }
        CommandMenu("Tools") {
            Button("Add Scene Numbers") { WriterMenuAction.perform(#selector(ManuscriptTextView.addSceneNumbers(_:))) }.disabled(!screenplay)
            Button("Number Scenes…") { WriterMenuAction.perform(#selector(ManuscriptTextView.configureSceneNumbers(_:))) }.disabled(!screenplay)
            Button("Remove Scene Numbers in Selection") { WriterMenuAction.perform(#selector(ManuscriptTextView.removeSelectedSceneNumbers(_:))) }.disabled(!screenplay || state?.hasSelection != true)
            Button("Remove Scene Numbers") { WriterMenuAction.perform(#selector(ManuscriptTextView.removeSceneNumbers(_:))) }.disabled(!screenplay)
            Divider()
            Menu("Markers") {
                Button("Insert Marker") { WriterMenuAction.perform(#selector(ManuscriptTextView.insertScriptMarker(_:))) }.keyboardShortcut("m", modifiers: [.control])
                Button("Insert Named Marker") { WriterMenuAction.perform(#selector(ManuscriptTextView.insertNamedScriptMarker(_:))) }
                Button("Go to Next Marker") { WriterMenuAction.perform(#selector(ManuscriptTextView.goToNextScriptMarker(_:))) }.keyboardShortcut("m", modifiers: [.control, .option])
                Button("Remove Markers") { WriterMenuAction.perform(#selector(ManuscriptTextView.removeScriptMarkers(_:))) }
            }.disabled(!editable)
        }
    }
}
