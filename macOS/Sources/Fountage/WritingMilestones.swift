import Foundation

struct WritingMilestone: Identifiable, Equatable {
    enum Unit: String, CaseIterable {
        case words, characters, pages
    }
    let id: String
    let unit: Unit
    let target: Int
    let completedAt: Date?
    var completed: Bool { completedAt != nil }
}

struct WritingMilestoneCounts {
    let words: Int
    let characters: Int
    // A missing or failed layout must never complete a page milestone.
    let pages: Int?
    func value(for unit: WritingMilestone.Unit) -> Int? {
        switch unit {
        case .words: words
        case .characters: characters
        case .pages: pages
        }
    }
}

extension ScreenplayDocument {
    struct MilestoneError: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// Decodes known units only; unknown units remain untouched in the raw record.
    func writingMilestones() throws -> [WritingMilestone] {
        try milestoneRecord().items.compactMap(Self.decodeMilestone)
    }

    @discardableResult
    mutating func addMilestone(unit: WritingMilestone.Unit, target: Int,
                               counts: WritingMilestoneCounts, now: Date = Date()) throws -> String {
        guard DraftGoal.validTargets.contains(target) else { throw MilestoneError(message: "Enter a whole-number target from 1 to 10,000,000.") }
        var record = try milestoneRecord()
        let id = UUID().uuidString
        var item: [String: Any] = ["id": id, "unit": unit.rawValue, "target": target]
        Self.completeMilestone(&item, unit: unit, target: target, counts: counts, now: now)
        record.items.append(item)
        storeMilestones(record)
        return id
    }

    mutating func editMilestone(id: String, unit: WritingMilestone.Unit, target: Int,
                                counts: WritingMilestoneCounts, now: Date = Date(), expected: WritingMilestone? = nil) throws {
        guard DraftGoal.validTargets.contains(target) else { throw MilestoneError(message: "Enter a whole-number target from 1 to 10,000,000.") }
        var record = try milestoneRecord()
        guard let index = record.items.firstIndex(where: { $0["id"] as? String == id }),
              let previous = try Self.decodeMilestone(record.items[index]) else {
            throw MilestoneError(message: "This milestone has changed or is no longer available. Reopen the goals list and try again.")
        }
        guard expected == nil || expected == previous else {
            throw MilestoneError(message: "This milestone changed while you were editing it. Reopen it and try again.")
        }
        guard previous.unit != unit || previous.target != target else { return }
        record.items[index]["unit"] = unit.rawValue
        record.items[index]["target"] = target
        record.items[index].removeValue(forKey: "completedAt")
        record.items[index].removeValue(forKey: "completedCount")
        Self.completeMilestone(&record.items[index], unit: unit, target: target, counts: counts, now: now)
        storeMilestones(record)
    }

    mutating func removeMilestone(id: String, expected: WritingMilestone? = nil) throws {
        var record = try milestoneRecord()
        guard let index = record.items.firstIndex(where: { $0["id"] as? String == id }),
              let previous = try Self.decodeMilestone(record.items[index]), expected == nil || expected == previous else {
            throw MilestoneError(message: "This milestone has changed or is no longer available. Reopen the goals list and try again.")
        }
        record.items.remove(at: index)
        storeMilestones(record)
    }

    /// One atomic metadata update completes every newly reached milestone.
    /// Already completed entries keep their original date/count after text deletion.
    mutating func advanceMilestones(counts: WritingMilestoneCounts, now: Date = Date()) throws {
        var record = try milestoneRecord()
        var changed = false
        for index in record.items.indices {
            guard let milestone = try Self.decodeMilestone(record.items[index]), !milestone.completed,
                  let value = counts.value(for: milestone.unit), value >= milestone.target else { continue }
            Self.completeMilestone(&record.items[index], unit: milestone.unit, target: milestone.target, counts: counts, now: now)
            changed = true
        }
        if changed { storeMilestones(record) }
    }

    private struct MilestoneRecord {
        var fields: [String: Any]
        var items: [[String: Any]]
    }
    private func milestoneRecord() throws -> MilestoneRecord {
        guard metadata[Self.namespace] == nil || metadata[Self.namespace] is [String: Any] else {
            throw MilestoneError(message: "This project’s application metadata uses an unsupported format. It has been preserved.")
        }
        let own = metadata[Self.namespace] as? [String: Any] ?? [:]
        guard own["goals"] == nil || own["goals"] is [String: Any] else {
            throw MilestoneError(message: "This project’s goals use an unsupported format. They have been preserved.")
        }
        let goals = own["goals"] as? [String: Any] ?? [:]
        guard let value = goals["milestones"] else { return MilestoneRecord(fields: ["version": 1], items: []) }
        guard let fields = value as? [String: Any], Self.milestoneInteger(fields["version"]) == 1,
              let items = fields["items"] as? [[String: Any]] else {
            throw MilestoneError(message: "This project uses an unsupported milestone format or version. No milestones were changed.")
        }
        var ids: Set<String> = []
        for item in items {
            guard let id = item["id"] as? String, !id.isEmpty, ids.insert(id).inserted else {
                throw MilestoneError(message: "This project contains invalid milestone identities. No milestones were changed.")
            }
            _ = try Self.decodeMilestone(item)
        }
        return MilestoneRecord(fields: fields, items: items)
    }
    private static func milestoneInteger(_ value: Any?) -> Int? {
        guard let value, let data = try? JSONSerialization.data(withJSONObject: [value]),
              let values = try? JSONDecoder().decode([Int].self, from: data) else { return nil }
        return values.first
    }
    private static func milestoneDate(_ value: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.date(from: value)
    }
    private static func decodeMilestone(_ item: [String: Any]) throws -> WritingMilestone? {
        guard let rawUnit = item["unit"] as? String, let unit = WritingMilestone.Unit(rawValue: rawUnit) else { return nil }
        guard let id = item["id"] as? String, !id.isEmpty,
              let target = milestoneInteger(item["target"]), DraftGoal.validTargets.contains(target) else {
            throw MilestoneError(message: "This project contains an invalid milestone target. No milestones were changed.")
        }
        var date: Date?
        if let value = item["completedAt"] {
            guard let text = value as? String, let parsed = milestoneDate(text), milestoneInteger(item["completedCount"]) == target else {
                throw MilestoneError(message: "This project contains an unsupported milestone completion record. It has been preserved.")
            }
            date = parsed
        } else if item["completedCount"] != nil {
            throw MilestoneError(message: "This project contains an incomplete milestone completion record. It has been preserved.")
        }
        return WritingMilestone(id: id, unit: unit, target: target, completedAt: date)
    }
    private static func completeMilestone(_ item: inout [String: Any], unit: WritingMilestone.Unit, target: Int,
                                          counts: WritingMilestoneCounts, now: Date) {
        guard let value = counts.value(for: unit), value >= target else { return }
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        item["completedAt"] = formatter.string(from: now)
        item["completedCount"] = target
    }
    private mutating func storeMilestones(_ record: MilestoneRecord) {
        var info = metadata
        var own = info[Self.namespace] as? [String: Any] ?? [:]
        var goals = own["goals"] as? [String: Any] ?? [:]
        var fields = record.fields
        fields["items"] = record.items
        goals["milestones"] = fields
        own["goals"] = goals; info[Self.namespace] = own; metadata = info
    }
}
