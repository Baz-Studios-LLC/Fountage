import SwiftUI

struct TitlePageEditor: View {
    let source: String
    let apply: (ManuscriptEdit) throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var draft: TitlePageEdit?
    @State private var values: [String: String] = [:]
    @State private var error: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Title Page").font(.system(size: 25, design: .serif))
                Text("Use multiple lines for coauthors or contact details.").foregroundStyle(.secondary)
            }.padding(24)
            Divider()
            if draft != nil {
                Form {
                    ForEach(TitlePageEdit.keys, id: \.self) { label in
                        VStack(alignment: .leading, spacing: 7) {
                            Text(label).font(.headline)
                            TitleFieldEditor(text: Binding(get: { values[TitlePageEdit.canonical(label)] ?? "" }, set: { values[TitlePageEdit.canonical(label)] = $0; error = nil }), label: label)
                                .frame(height: label == "Contact" ? 72 : 44)
                                .padding(5).background(.background).clipShape(RoundedRectangle(cornerRadius: 5))
                                .overlay(RoundedRectangle(cornerRadius: 5).stroke(.quaternary))
                                .accessibilityLabel(label)
                        }
                    }
                }.formStyle(.grouped)
            }
            if let error { Text(error).font(.callout).foregroundStyle(.red).padding(.horizontal, 24).padding(.vertical, 12) }
            Divider()
            HStack {
                Text("Additional title fields are preserved.").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Apply") {
                    do {
                        if let edit = try draft?.edit(values: values) { try apply(edit) }
                        dismiss()
                    } catch { self.error = error.localizedDescription }
                }.keyboardShortcut(.defaultAction).disabled(draft == nil)
            }.padding(18)
        }.frame(width: 620, height: 700)
        .onAppear {
            do { let parsed = try TitlePageEdit(source: source); draft = parsed; values = parsed.original }
            catch { self.error = error.localizedDescription }
        }
    }
}
