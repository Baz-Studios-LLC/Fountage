import SwiftUI

struct ReferenceCard: Identifiable, Equatable {
    enum Kind: String, CaseIterable, Identifiable {
        case character, location
        var id: String { rawValue }
        var title: String { self == .character ? "Characters" : "Locations" }
    }
    var id: String = UUID().uuidString
    var kind: Kind
    var name: String
    var notes: String = ""
}

enum ReferenceEditError: LocalizedError {
    case unsupported, changed, invalidName
    var errorDescription: String? {
        switch self {
        case .unsupported: "This project’s reference data uses an unsupported format and has been preserved."
        case .changed: "These details changed while this editor was open. Your changes have not been saved. Cancel and reopen the details to review the latest version."
        case .invalidName: "Enter a name for these details."
        }
    }
}

extension ReferenceCard {
    /// Swift String equality normalizes Unicode; saved source details must compare exactly.
    func matches(_ other: ReferenceCard) -> Bool {
        id.utf8.elementsEqual(other.id.utf8) && kind == other.kind &&
        name.utf8.elementsEqual(other.name.utf8) && notes.utf8.elementsEqual(other.notes.utf8)
    }
}

extension ScreenplayDocument {
    var canEditReferenceCards: Bool {
        guard metadata[Self.namespace] == nil || metadata[Self.namespace] is [String: Any] else { return false }
        let own = metadata[Self.namespace] as? [String: Any] ?? [:]
        return own["referenceCards"] == nil || own["referenceCards"] is [String: Any]
    }
    var referenceCards: [ReferenceCard] {
        let own = metadata[Self.namespace] as? [String: Any] ?? [:]
        let records = own["referenceCards"] as? [String: Any] ?? [:]
        return records.compactMap { id, value in
            guard let record = value as? [String: Any],
                  let raw = record["kind"] as? String, let kind = ReferenceCard.Kind(rawValue: raw),
                  let name = record["name"] as? String, let notes = record["notes"] as? String else { return nil }
            return ReferenceCard(id: id, kind: kind, name: name, notes: notes)
        }.sorted { $0.name == $1.name ? $0.id < $1.id : $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
    @discardableResult
    mutating func saveReferenceCard(_ card: ReferenceCard, replacing expected: ReferenceCard? = nil) throws -> Bool {
        guard canEditReferenceCards else { throw ReferenceEditError.unsupported }
        let name = card.name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, !card.id.isEmpty else { throw ReferenceEditError.invalidName }
        var info = metadata
        var own = info[Self.namespace] as? [String: Any] ?? [:]
        var records = own["referenceCards"] as? [String: Any] ?? [:]
        if let expected {
            guard card.id.utf8.elementsEqual(expected.id.utf8),
                  let current = referenceCards.first(where: { $0.id == card.id }), current.matches(expected) else { throw ReferenceEditError.changed }
            var normalized = card; normalized.name = name
            if current.matches(normalized) { return false }
        } else {
            // A new card may not replace an existing, malformed or future record.
            guard records[card.id] == nil else { throw ReferenceEditError.changed }
        }
        var record = records[card.id] as? [String: Any] ?? [:]
        record["kind"] = card.kind.rawValue
        record["name"] = name
        record["notes"] = card.notes
        records[card.id] = record
        own["referenceCards"] = records
        info[Self.namespace] = own
        metadata = info
        return true
    }
    mutating func removeReferenceCard(_ expected: ReferenceCard) throws {
        guard canEditReferenceCards else { throw ReferenceEditError.unsupported }
        guard let current = referenceCards.first(where: { $0.id == expected.id }), current.matches(expected) else { throw ReferenceEditError.changed }
        var info = metadata
        var own = info[Self.namespace] as? [String: Any] ?? [:]
        var records = own["referenceCards"] as? [String: Any] ?? [:]
        records.removeValue(forKey: expected.id)
        own["referenceCards"] = records
        info[Self.namespace] = own
        metadata = info
    }
}

private struct ReferenceEditSession: Identifiable {
    let id = UUID()
    let card: ReferenceCard
    let original: ReferenceCard?
    init(card: ReferenceCard, existing: Bool = false) {
        self.card = card
        self.original = existing ? card : nil
    }
}

struct ReferenceLibrary: View {
    @Binding var document: ScreenplayDocument
    let analysis: ManuscriptAnalysis
    @Environment(\.dismiss) private var dismiss
    @State private var kind: ReferenceCard.Kind = .character
    @State private var editing: ReferenceEditSession?
    @State private var query = ""
    @State private var selection: String?
    private var cards: [ReferenceCard] { document.referenceCards.filter { $0.kind == kind } }
    private var filteredCards: [ReferenceCard] {
        cards.filter { query.isEmpty || $0.name.localizedStandardContains(query) || $0.notes.localizedStandardContains(query) }
    }
    private var detected: [String] {
        let names = kind == .character ? Array(analysis.characters.keys) : Array(analysis.locations.keys)
        return names.filter { name in !cards.contains { $0.name == name } && (query.isEmpty || name.localizedStandardContains(query)) }.sorted()
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Cast & Locations").font(.title2.weight(.semibold))
            Text("Keep character backgrounds and location details with your project. These notes never change your manuscript.")
                .foregroundStyle(.secondary)
            Picker("Reference type", selection: $kind) {
                ForEach(ReferenceCard.Kind.allCases) { Text($0.title).tag($0) }
            }.pickerStyle(.segmented)
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField("Search names and notes", text: $query).textFieldStyle(.plain)
                if !query.isEmpty {
                    Button { query = "" } label: { Image(systemName: "xmark.circle.fill") }
                        .buttonStyle(.plain).accessibilityLabel("Clear reference search")
                }
            }.padding(8).background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
            List(selection: $selection) {
                Section("Saved in this project") {
                    if filteredCards.isEmpty { Text(query.isEmpty ? "No saved details yet." : "No saved details match your search.").foregroundStyle(.secondary) }
                    ForEach(filteredCards) { card in
                        Button { editing = ReferenceEditSession(card: card, existing: true) } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(card.name).font(.headline)
                                Text(card.notes.isEmpty ? "Add details…" : card.notes).lineLimit(2).foregroundStyle(.secondary)
                            }.frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 4).contentShape(Rectangle())
                        }.buttonStyle(.plain).tag("saved:" + card.id)
                    }
                }
                if !detected.isEmpty {
                    Section("Detected in manuscript") {
                        ForEach(detected, id: \.self) { name in
                            Button { editing = ReferenceEditSession(card: ReferenceCard(kind: kind, name: name)) } label: {
                                Label(name, systemImage: "plus.circle").frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
                            }.buttonStyle(.plain).tag("detected:" + name).help("Add saved details for \(name)")
                        }
                    }
                }
            }.listStyle(.inset).disabled(!document.canEditReferenceCards)
                .onKeyPress(.return) {
                    guard document.canEditReferenceCards, let selection else { return .ignored }
                    if let card = filteredCards.first(where: { "saved:" + $0.id == selection }) {
                        editing = ReferenceEditSession(card: card, existing: true)
                        return .handled
                    }
                    if let name = detected.first(where: { "detected:" + $0 == selection }) {
                        editing = ReferenceEditSession(card: ReferenceCard(kind: kind, name: name))
                        return .handled
                    }
                    return .ignored
                }
                .onChange(of: query) { selection = nil }
                .onChange(of: kind) { selection = nil }
                .help("Use the arrow keys to select a reference, then Return to edit its details.")
            if !document.canEditReferenceCards {
                Text("This project's reference data uses an unsupported format and has been preserved. Editing is unavailable.").font(.caption)
            }
            HStack {
                Button(kind == .character ? "Add Character" : "Add Location", systemImage: "plus") {
                    editing = ReferenceEditSession(card: ReferenceCard(kind: kind, name: ""))
                }.disabled(!document.canEditReferenceCards)
                Spacer()
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }.padding(24).frame(width: 550, height: 520)
        .sheet(item: $editing) { session in
            ReferenceCardEditor(card: session.card, original: session.original,
                                save: { card, original in
                                    var updated = document
                                    if try updated.saveReferenceCard(card, replacing: original) { document = updated }
                                }, remove: { original in
                                    var updated = document
                                    try updated.removeReferenceCard(original)
                                    document = updated
                                })
        }
    }
}

private struct ReferenceCardEditor: View {
    @State var card: ReferenceCard
    let original: ReferenceCard?
    let save: (ReferenceCard, ReferenceCard?) throws -> Void
    let remove: (ReferenceCard) throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var confirmRemoval = false
    @State private var editError: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(card.kind == .character ? "Character Details" : "Location Details").font(.title2.weight(.semibold))
            TextField("Name", text: $card.name).textFieldStyle(.roundedBorder)
            Text("Notes").font(.headline)
            TextEditor(text: $card.notes).font(.body).border(Color.secondary.opacity(0.25))
                .accessibilityLabel("Reference notes")
            Text("Changing this name does not rename occurrences in the manuscript.").font(.caption).foregroundStyle(.secondary)
            HStack {
                if original != nil {
                    Button("Remove Details", role: .destructive) { confirmRemoval = true }
                }
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Save") {
                    do { try save(card, original); dismiss() }
                    catch { editError = error.localizedDescription }
                }.keyboardShortcut(.defaultAction)
                    .disabled(card.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }.padding(24).frame(width: 500, height: 390)
        .alert("Details Not Changed", isPresented: Binding(get: { editError != nil }, set: { if !$0 { editError = nil } })) {
            Button("OK") { editError = nil }
        } message: { Text(editError ?? "") }
        .confirmationDialog("Remove these saved details? The manuscript will remain unchanged.", isPresented: $confirmRemoval) {
            Button("Remove Details", role: .destructive) {
                guard let original else { return }
                do { try remove(original); dismiss() }
                catch { editError = error.localizedDescription }
            }
        }
    }
}
