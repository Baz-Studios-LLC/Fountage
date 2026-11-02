import SwiftUI

struct DraftGoal {
    static let validTargets = 1...10_000_000
    let target: Int
    let words: Int
    var remaining: Int { max(0, target - max(0, words)) }
    var reached: Bool { words >= target }
    var progress: Double { min(1, max(0, Double(words) / Double(max(1, target)))) }

    static func parseTarget(_ text: String) -> Int? {
        guard let target = Int(text.trimmingCharacters(in: .whitespacesAndNewlines)),
              validTargets.contains(target) else { return nil }
        return target
    }
}

struct DraftGoalView: View {
    @Binding var target: Int?
    let words: Int
    var unit = "words"
    var unavailable: String? = nil
    var goalName: String {
        switch unit {
        case "pages": "Draft page goal"
        case "characters": "Draft character goal"
        default: "Draft word goal"
        }
    }
    private var explanation: String {
        switch unit {
        case "pages": "Counts screenplay body pages using this project’s output settings. The title page is excluded."
        case "characters": "Includes spaces and formatting markers. Screenplay notes, synopses and omitted text are excluded."
        default: "A target for this manuscript."
        }
    }
    @State private var editing = false
    @State private var input = ""
    @FocusState private var targetFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            if let target {
                let goal = DraftGoal(target: target, words: words)
                HStack(alignment: .firstTextBaseline) {
                    Text(goalName).font(.caption).foregroundStyle(.secondary)
                    Spacer()
                    Button("Edit") { beginEditing() }
                        .buttonStyle(.link).font(.caption)
                        .accessibilityLabel("Edit \(goalName.lowercased())")
                }
                if let unavailable {
                    Text(unavailable).font(.caption).foregroundStyle(.secondary)
                        .accessibilityLabel("Page count unavailable: \(unavailable)")
                } else {
                    ProgressView(value: goal.progress)
                        .accessibilityLabel(goalName)
                        .accessibilityValue("\(words.formatted()) of \(target.formatted()) \(unit)")
                    Text("\(words.formatted()) of \(target.formatted()) \(unit)").font(.caption.monospacedDigit())
                    if goal.reached {
                        Label("Goal reached", systemImage: "checkmark.circle.fill")
                            .font(.caption).foregroundStyle(Color(nsColor: StudioTheme.accent))
                    } else {
                        Text("\(goal.remaining.formatted()) to go")
                            .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                    }
                }
            } else {
                Button { beginEditing() } label: {
                    Label("Set \(goalName.capitalized)", systemImage: "scope").font(.caption)
                }.buttonStyle(.link)
            }
        }
        .popover(isPresented: $editing, arrowEdge: .leading) {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 5) {
                    Text(goalName).font(.headline)
                    Text(explanation).font(.subheadline).foregroundStyle(.secondary)
                }
                VStack(alignment: .leading, spacing: 6) {
                    TextField("Target \(unit)", text: $input, prompt: Text("e.g. 50000"))
                        .textFieldStyle(.roundedBorder).focused($targetFocused)
                        .onSubmit { save() }
                    Text("Enter a whole number from 1 to 10,000,000.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                HStack {
                    if target != nil {
                        Button("Remove Goal", role: .destructive) { target = nil; editing = false }
                    }
                    Spacer()
                    Button("Cancel") { editing = false }.keyboardShortcut(.cancelAction)
                    Button("Save") { save() }.keyboardShortcut(.defaultAction)
                        .disabled(DraftGoal.parseTarget(input) == nil)
                }
            }.padding(20).frame(width: 340)
                .onAppear {
                    // The popover must join its window before focus can enter the field.
                    DispatchQueue.main.async { targetFocused = true }
                }
                .onDisappear { targetFocused = false }
        }
    }
    private func beginEditing() { input = target.map(String.init) ?? ""; editing = true }
    private func save() {
        guard let value = DraftGoal.parseTarget(input) else { return }
        target = value
        editing = false
    }
}
