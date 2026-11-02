import SwiftUI

struct NewDocumentChooser: View {
    @Environment(\.dismissWindow) private var dismissWindow
    @State private var format = DocumentFormat.screenplay
    @State private var preset = LayoutPreset.screenplay
    @State private var academicStyleIndex = 0
    @State private var includeStarter = false
    let create: (DocumentFormat, LayoutPreset, AcademicStyle?, Bool) -> Void
    private let accent = Color(nsColor: StudioTheme.accent)

    var body: some View {
        GeometryReader { geometry in
        ScrollView {
        VStack(spacing: 0) {
            Spacer(minLength: 24)
            VStack(spacing: 10) {
                Text("Fountage").font(.system(size: 30, weight: .regular, design: .serif))
                Text("by Baz Studios").font(.caption).foregroundStyle(.secondary)
            }.padding(.bottom, 30)
            VStack(alignment: .leading, spacing: 22) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("What will you write?").font(.system(size: 28, weight: .regular, design: .serif))
                    Text("Choose a document type to set up your workspace.").foregroundStyle(.secondary)
                }
                LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: geometry.size.width >= 1050 ? 4 : 2), spacing: 12) {
                    ForEach(DocumentFormat.allCases) { option in
                        formatCard(option)
                    }
                }
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Text("Layout").frame(width: 105, alignment: .leading)
                        Picker("Layout", selection: $preset) {
                            ForEach(format.presets) { Text($0.name).tag($0) }
                        }.labelsHidden().frame(maxWidth: 270)
                        Spacer()
                    }
                    if format == .academic {
                        HStack {
                            Text("Required style").frame(width: 105, alignment: .leading)
                            Picker("Required style", selection: $academicStyleIndex) {
                                Text("Not selected").tag(0)
                                ForEach(AcademicStyle.choices.indices, id: \.self) { index in
                                    Text(AcademicStyle.choices[index].name).tag(index + 1)
                                }
                            }.labelsHidden().frame(maxWidth: 270)
                            Spacer()
                        }
                        Text("Your edition is saved with the paper. Citation formatting is not yet available.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Toggle("Include a starting outline", isOn: $includeStarter).toggleStyle(.checkbox)
                }
                Divider()
                HStack {
                    Text("You can change the layout as you write.").font(.caption).foregroundStyle(.secondary)
                    Spacer()
                    Button("Cancel") { dismissWindow() }.keyboardShortcut(.cancelAction)
                    Button("Create") {
                        let style = academicStyleIndex == 0 ? nil : AcademicStyle.choices[academicStyleIndex - 1]
                        create(format, preset, format == .academic ? style : nil, includeStarter)
                    }.keyboardShortcut(.defaultAction).buttonStyle(.borderedProminent)
                }
            }.frame(maxWidth: geometry.size.width >= 1050 ? 1080 : 640)
            Spacer(minLength: 24)
        }
        .padding(.horizontal, 40)
        .frame(maxWidth: .infinity, minHeight: geometry.size.height)
        }
        .scrollBounceBehavior(.basedOnSize)
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .tint(accent)
        .onChange(of: format) { _, value in preset = value.presets[0] }
    }
    private func formatCard(_ option: DocumentFormat) -> some View {
        let selected = option == format
        return Button { format = option } label: {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: option.icon).font(.system(size: 24, weight: .light))
                    .foregroundStyle(selected ? accent : .secondary).frame(width: 32, height: 30)
                VStack(alignment: .leading, spacing: 7) {
                    Text(option.name).font(.system(size: 16, weight: .semibold))
                    Text(option.summary).font(.system(size: 12)).foregroundStyle(.secondary)
                        .multilineTextAlignment(.leading).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(selected ? accent : Color.secondary.opacity(0.35))
            }
            .padding(18).frame(maxWidth: .infinity, minHeight: 96, alignment: .leading)
            .background(selected ? accent.opacity(0.08) : Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(selected ? accent.opacity(0.8) : Color.primary.opacity(0.1), lineWidth: selected ? 1.5 : 1))
            .contentShape(RoundedRectangle(cornerRadius: 10))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(option.name)
        .accessibilityValue(selected ? "Selected" : "Not selected")
        .accessibilityHint(option.summary)
    }
}

private extension DocumentFormat {
    var icon: String {
        switch self { case .screenplay: "film"; case .comic: "square.grid.2x2"; case .novel: "book.closed"; case .academic: "graduationcap"; case .stageplay: "theatermasks"; case .audio: "waveform"; case .general: "pencil.and.outline"; case .poetry: "music.note" }
    }
    var summary: String {
        switch self {
        case .screenplay: "Scenes, action and dialogue."
        case .comic: "Comic books and graphic novels."
        case .novel: "Novels, memoirs and nonfiction."
        case .academic: "Sections and a saved style edition."
        case .stageplay: "Acts, scenes and stage directions."
        case .audio: "Audio dramas and podcasts."
        case .general: "Articles, notes and fresh ideas."
        case .poetry: "Stanzas, verses and choruses."
        }
    }
}
