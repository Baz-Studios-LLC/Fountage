import SwiftUI
import UniformTypeIdentifiers

final class FountageApplicationDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Isolated test bundles have a different Launch Services identity and can
        // inherit a generic cached Dock icon. Keep production's adaptive asset icon.
        guard Bundle.main.object(forInfoDictionaryKey: "FountageQAApplication") as? Bool == true,
              let url = Bundle.main.url(forResource: "Fountage", withExtension: "icns"),
              let icon = NSImage(contentsOf: url) else { return }
        NSApp.applicationIconImage = icon
    }
}

@main
struct FountageApp: App {
    @NSApplicationDelegateAdaptor(FountageApplicationDelegate.self) private var appDelegate
    @AppStorage("studioAppearance") private var appearance = StudioAppearance.system
    @AppStorage("editorPaperAppearance") private var paper = EditorPaperAppearance.matchApp
    @AppStorage("typewriterScrolling") private var typewriterScrolling = false
    private let releaseVersion = Bundle.main.object(forInfoDictionaryKey: "FountageReleaseVersion") as? String ?? "Development"
    var body: some SwiftUI.Scene {
        DocumentGroup(newDocument: ScreenplayDocument()) { file in
            StudioView(document: file.$document, fileURL: file.fileURL)
                .frame(minWidth: 900, minHeight: 620)
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { NSApp.appearance = appearance.native }
                .onChange(of: appearance) { _, value in NSApp.appearance = value.native }
        }
        .defaultSize(width: 1320, height: 880)
        .windowToolbarStyle(.unified(showsTitle: false))
        .commands {
            SidebarCommands()
            TextEditingCommands()
            ShelfCommands()
            WriterCommands()
            CommandGroup(after: .toolbar) {
                Toggle("Typewriter Scrolling", isOn: $typewriterScrolling)
                    .keyboardShortcut("t", modifiers: [.command, .control])
                Menu("Appearance") {
                    Picker("Appearance", selection: $appearance) {
                        ForEach(StudioAppearance.allCases) { Text($0.name).tag($0) }
                    }
                    Divider()
                    Picker("Paper", selection: $paper) {
                        ForEach(EditorPaperAppearance.allCases) { Text($0.name).tag($0) }
                    }
                }
            }
            CommandGroup(replacing: .appInfo) {
                Button("About Fountage") {
                    NSApp.orderFrontStandardAboutPanel(options: [
                        .applicationName: "Fountage by Baz Studios",
                        .applicationVersion: releaseVersion,
                        .version: ""
                    ])
                }
            }
        }
        Settings {
            Form {
                Section("Appearance") {
                    Picker("Theme", selection: $appearance) {
                        ForEach(StudioAppearance.allCases) { Text($0.name).tag($0) }
                    }.pickerStyle(.segmented)
                    Text("System follows your Mac’s appearance.").font(.caption).foregroundStyle(.secondary)
                    Picker("Paper", selection: $paper) {
                        ForEach(EditorPaperAppearance.allCases) { Text($0.name).tag($0) }
                    }.pickerStyle(.segmented)
                    Text("White Paper keeps the writing surface white in any app theme.").font(.caption).foregroundStyle(.secondary)
                }
                Section("Writing") {
                    Toggle("Typewriter Scrolling", isOn: $typewriterScrolling)
                    Text("Keep the line you’re writing in the center of the page. You can still scroll to read.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                Section("Fountage") {
                    Text("Fountage by Baz Studios").font(.headline)
                    Text("A native home for your next story.").foregroundStyle(.secondary)
                    Text("macOS development preview · \(releaseVersion)").font(.caption).foregroundStyle(.secondary)
                }
                Section("This preview") {
                    Text("Eight document types have their own editing layouts and starting outlines. Academic style editions can be recorded. Citations, production export, shareable templates and Highland parity features are still in development.")
                }
            }.formStyle(.grouped).frame(width: 490, height: 540)
                .preferredColorScheme(appearance.colorScheme)
                .onAppear { NSApp.appearance = appearance.native }
                .onChange(of: appearance) { _, value in NSApp.appearance = value.native }
        }
    }
}
