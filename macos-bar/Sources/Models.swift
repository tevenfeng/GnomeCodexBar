import Foundation

// ── Data models matching status.json ──────────────────

struct StatusSnapshot: Codable {
    let updatedAt: String
    let providers: [ProviderStatus]

    enum CodingKeys: String, CodingKey {
        case updatedAt = "updated_at"
        case providers
    }
}

struct ProviderStatus: Codable, Identifiable {
    let providerId: String
    let providerName: String
    let available: Bool
    let remainingPercent: Double
    let details: [String: JSONValue]
    let error: String?

    var id: String { providerId }

    enum CodingKeys: String, CodingKey {
        case providerId = "provider_id"
        case providerName = "provider_name"
        case available
        case remainingPercent = "remaining_percent"
        case details
        case error
    }
}

// ── Flexible JSON value for details dict ──────────────

enum JSONValue: Codable, Equatable {
    case string(String)
    case double(Double)
    case int(Int)
    case bool(Bool)
    case null

    var stringValue: String? {
        if case .string(let v) = self { return v }
        if case .double(let v) = self { return String(v) }
        if case .int(let v) = self { return String(v) }
        return nil
    }

    var doubleValue: Double? {
        if case .double(let v) = self { return v }
        if case .int(let v) = self { return Double(v) }
        if case .string(let v) = self { return Double(v) }
        return nil
    }

    // Decode from any JSON value
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let v = try? container.decode(String.self) { self = .string(v) }
        else if let v = try? container.decode(Double.self) { self = .double(v) }
        else if let v = try? container.decode(Int.self) { self = .int(v) }
        else if let v = try? container.decode(Bool.self) { self = .bool(v) }
        else { self = .null }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let v): try container.encode(v)
        case .double(let v): try container.encode(v)
        case .int(let v): try container.encode(v)
        case .bool(let v): try container.encode(v)
        case .null: try container.encodeNil()
        }
    }
}
