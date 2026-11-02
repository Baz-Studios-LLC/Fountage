import SwiftUI

struct MilestonePanel: View {
    @Binding var document: ScreenplayDocument
    @Environment(\.dismiss) private var dismiss
    @StateObject private var pages = DraftPageCount()
    @State private var cache = ManuscriptAnalysisCache()
    @State private var unit = WritingMilestone.Unit.words
    @State private var input = ""
    @State private var editing: WritingMilestone?
    @State private var error: String?
    @FocusState private var targetFocused: Bool
    private var milestones: [WritingMilestone] { (try? document.writingMilestones()) ?? [] }
    private var readError: String? {
        if document.text.utf8.contains(0) { return ProjectError.invalidManuscript.localizedDescription }
        do { _ = try document.writingMilestones(); return nil }
        catch { return error.localizedDescription }
    }
    private var request: PageGoalRequest {
        PageGoalRequest(source: ExactSource(document.text), options: document.outputOptions(),
                        enabled: document.format == .screenplay && milestones.contains { !$0.completed && $0.unit == .pages })
    }
    private var counts: WritingMilestoneCounts {
        let analysis = cache.analysis(for: document.text, format: document.format)
        return WritingMilestoneCounts(words: analysis.words, characters: analysis.character_count,
                                      pages: pages.resolvedRequest == request ? pages.count : nil)
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 5) {
                    Text("Writing milestones").font(.system(size: 25, design: .serif))
                    Text("Set the next milestone. Keep the ones you’ve reached.").foregroundStyle(.secondary)
                }
                Spacer()
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }.padding(22)
            Divider()
            if let readError {
                ContentUnavailableView("Milestones Unavailable", systemImage: "flag.slash", description: Text(readError))
            } else {
                editor.padding(20)
                Divider()
                List {
                    Section("In Progress") {
                        let active = milestones.filter { !$0.completed }
                        if active.isEmpty { Text("Room for your next milestone.").foregroundStyle(.secondary) }
                        ForEach(active) { row($0) }
                    }
                    Section("Completed") {
                        let complete = milestones.filter(\.completed)
                        if complete.isEmpty { Text("Reached milestones will stay here as your draft evolves.").foregroundStyle(.secondary) }
                        ForEach(complete) { row($0) }
                    }
                }.listStyle(.inset)
            }
        }
        .frame(minWidth: 600, idealWidth: 660, minHeight: 480, idealHeight: 600)
        .task(id: request) { await pages.refresh(request) }
        .alert("Milestone Unavailable", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK") { error = nil }
        } message: { Text(error ?? "") }
    }
    private var editor: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(editing == nil ? "NEW MILESTONE" : "EDIT MILESTONE")
                .font(.caption.weight(.semibold)).tracking(1).foregroundStyle(.secondary)
            HStack {
                TextField("Target", text: $input, prompt: Text("e.g. 1000"))
                    .textFieldStyle(.roundedBorder).focused($targetFocused).onSubmit { save() }
                    .accessibilityLabel("Milestone target")
                Picker("Unit", selection: $unit) {
                    ForEach(WritingMilestone.Unit.allCases, id: \.self) { value in
                        Text(value.rawValue.capitalized).tag(value)
                            .disabled(value == .pages && document.format != .screenplay)
                    }
                }.labelsHidden().frame(width: 130).accessibilityLabel("Milestone unit")
                if editing != nil { Button("Cancel") { reset() } }
                Button(editing == nil ? "Add Milestone" : "Save Changes") { save() }
                    .disabled(DraftGoal.parseTarget(input) == nil || (unit == .pages && document.format != .screenplay))
            }
            Text("Completed milestones keep their date, even if you later shorten your draft. Page milestones count screenplay body pages.")
                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
    }
    private func row(_ milestone: WritingMilestone) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                Label("\(milestone.target.formatted()) \(milestone.unit.rawValue)", systemImage: milestone.completed ? "checkmark.circle.fill" : "flag")
                    .font(.headline).foregroundStyle(milestone.completed ? Color(nsColor: StudioTheme.accent) : .primary)
                Spacer()
                Button("Edit") { beginEditing(milestone) }
                    .accessibilityLabel("Edit \(milestone.target) \(milestone.unit.rawValue) milestone")
                Button(role: .destructive) { remove(milestone) } label: { Image(systemName: "trash") }
                    .accessibilityLabel("Remove \(milestone.target) \(milestone.unit.rawValue) milestone")
            }.buttonStyle(.borderless)
            if let date = milestone.completedAt {
                Text("Completed \(date.formatted(date: .abbreviated, time: .shortened))")
                    .font(.caption).foregroundStyle(.secondary)
            } else if let value = counts.value(for: milestone.unit) {
                ProgressView(value: min(1, max(0, Double(value) / Double(milestone.target))))
                    .accessibilityLabel("\(milestone.target) \(milestone.unit.rawValue) milestone")
                    .accessibilityValue("\(value) of \(milestone.target) \(milestone.unit.rawValue)")
                Text("\(value.formatted()) of \(milestone.target.formatted()) \(milestone.unit.rawValue)")
                    .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            } else {
                Text(document.format == .screenplay ? (pages.error ?? "Counting pages…") : "Page progress is available in screenplay documents.")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }.padding(.vertical, 8)
            .accessibilityElement(children: .contain)
            .contextMenu {
                Button("Edit Milestone…") { beginEditing(milestone) }
                Button("Remove Milestone", role: .destructive) { remove(milestone) }
            }
    }
    private func beginEditing(_ milestone: WritingMilestone) {
        editing = milestone; unit = milestone.unit; input = String(milestone.target); targetFocused = true
    }
    private func save() {
        guard let target = DraftGoal.parseTarget(input) else { return }
        do {
            var updated = document
            let values = MilestoneWorkflow.counts(in: updated, cache: cache, includePages: unit == .pages)
            if let editing {
                try updated.editMilestone(id: editing.id, unit: unit, target: target, counts: values, expected: editing)
            } else { try updated.addMilestone(unit: unit, target: target, counts: values) }
            document = updated
            reset()
        } catch { self.error = error.localizedDescription }
    }
    private func remove(_ milestone: WritingMilestone) {
        do {
            var updated = document
            try updated.removeMilestone(id: milestone.id, expected: milestone)
            document = updated
            if editing?.id == milestone.id { reset() }
        } catch { self.error = error.localizedDescription }
    }
    private func reset() { editing = nil; input = "" }
}
