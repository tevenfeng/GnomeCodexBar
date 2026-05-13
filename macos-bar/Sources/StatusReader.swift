import Foundation
import Combine
import Darwin

/// Reads status.json and selected_provider.json, monitors for changes.
@MainActor
class StatusReader: ObservableObject {
    @Published var status: StatusSnapshot?
    @Published var selectedProvider: String?
    @Published var pollInterval: Double
    @Published private(set) var providerOrder: [String]
    @Published private var providerEnabled: [String: Bool]

    private var fileSource: DispatchSourceFileSystemObject?
    private var pollTimer: Timer?

    private let statusPath: URL
    private let selectedPath: URL
    private let configPath: URL
    private let dataDir: URL

    init() {
        // Use the same directory as the Rust CLI (dirs::data_local_dir())
        // On macOS this is ~/Library/Application Support/gnome-codex-bar/
        let dataDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/gnome-codex-bar")
        self.dataDir = dataDir
        self.statusPath = dataDir.appendingPathComponent("status.json")
        self.selectedPath = dataDir.appendingPathComponent("selected_provider.json")
        self.configPath = dataDir.appendingPathComponent("config.toml")

        self.pollInterval = 300
        self.providerOrder = Self.defaultProviderOrder
        self.providerEnabled = [:]

        readCliSettings()
        readStatus()
        readSelectedProvider()
        startMonitoring()
    }

    // ── Read ──────────────────────────────────────────

    func readStatus() {
        guard let data = try? Data(contentsOf: statusPath) else {
            return
        }
        do {
            status = try JSONDecoder().decode(StatusSnapshot.self, from: data)
        } catch {
            print("Failed to decode status.json: \(error)")
        }
    }

    func readSelectedProvider() {
        guard let data = try? Data(contentsOf: selectedPath),
              !data.isEmpty else {
            selectedProvider = nil
            return
        }
        if let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let selected = object["selected_provider"] as? String,
           !selected.isEmpty {
            selectedProvider = selected
            return
        }
        // Backward compatibility for older raw-text writes.
        if let str = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
           !str.isEmpty {
            selectedProvider = str
        } else {
            selectedProvider = nil
        }
    }

    /// Write selected provider ID to selected_provider.json
    func writeSelectedProvider(_ id: String) {
        let object = ["selected_provider": id]
        if let data = try? JSONSerialization.data(withJSONObject: object) {
            try? data.write(to: selectedPath, options: .atomic)
        }
        selectedProvider = id
    }

    // ── CLI config.toml ───────────────────────────────

    /// Read settings shared with the Rust CLI daemon.
    func readCliSettings() {
        let content = (try? String(contentsOf: configPath, encoding: .utf8)) ?? defaultConfigText()

        if let value = tomlValue(forKey: "refresh_interval_secs", inSection: "general", content: content),
           let seconds = Double(value) {
            pollInterval = seconds
        }

        providerOrder = normalizedProviderOrder(
            tomlStringArray(forKey: "provider_order", inSection: "general", content: content) ?? Self.defaultProviderOrder
        )

        for id in ["deepseek", "stepfun", "opencodego"] {
            if let value = tomlValue(forKey: "enabled", inSection: "providers.\(id)", content: content),
               let enabled = tomlBool(value) {
                providerEnabled[id] = enabled
            } else {
                providerEnabled[id] = id != "opencodego"
            }
        }
    }

    /// Persist refresh interval to CLI config.toml and restart the local fallback poll timer.
    func setPollInterval(_ seconds: Double) {
        pollInterval = seconds
        updateConfigValue(
            section: "general",
            key: "refresh_interval_secs",
            value: String(Int(seconds.rounded()))
        )
        restartPollTimer()
    }

    func setProviderOrder(_ order: [String]) {
        providerOrder = normalizedProviderOrder(order)
        updateConfigValue(
            section: "general",
            key: "provider_order",
            value: tomlStringArray(providerOrder)
        )
    }

    private static let defaultProviderOrder = ["deepseek", "stepfun", "opencodego"]

    private func defaultConfigText() -> String {
        """
        [providers.deepseek]
        enabled = true

        [providers.stepfun]
        enabled = true

        [providers.opencodego]
        enabled = false

        [general]
        refresh_interval_secs = 300
        selected_provider = "deepseek"
        provider_order = ["deepseek", "stepfun", "opencodego"]
        """
    }

    private func tomlValue(forKey key: String, inSection section: String, content: String) -> String? {
        var currentSection: String?
        for rawLine in content.components(separatedBy: .newlines) {
            let line = stripTomlInlineComment(rawLine).trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("[") && line.hasSuffix("]") {
                currentSection = String(line.dropFirst().dropLast())
                continue
            }
            guard currentSection == section else { continue }
            guard let equalsIndex = line.firstIndex(of: "=") else { continue }
            let lineKey = String(line[..<equalsIndex]).trimmingCharacters(in: .whitespaces)
            if lineKey == key {
                let rawValue = String(line[line.index(after: equalsIndex)...])
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                return decodeTomlBasicString(rawValue) ?? rawValue
            }
        }
        return nil
    }

    private func tomlStringArray(forKey key: String, inSection section: String, content: String) -> [String]? {
        guard let rawValue = rawTomlValue(forKey: key, inSection: section, content: content),
              rawValue.hasPrefix("["), rawValue.hasSuffix("]") else {
            return nil
        }

        let inner = rawValue.dropFirst().dropLast()
        var values: [String] = []
        var current = ""
        var inString = false
        var escaped = false

        for char in inner {
            if inString {
                current.append(char)
                if escaped {
                    escaped = false
                } else if char == "\\" {
                    escaped = true
                } else if char == "\"" {
                    inString = false
                }
                continue
            }

            if char == "\"" {
                inString = true
                current.append(char)
            } else if char == "," {
                if let value = decodeTomlBasicString(current.trimmingCharacters(in: .whitespacesAndNewlines)) {
                    values.append(value)
                }
                current = ""
            } else {
                current.append(char)
            }
        }

        if let value = decodeTomlBasicString(current.trimmingCharacters(in: .whitespacesAndNewlines)) {
            values.append(value)
        }

        return values
    }

    private func rawTomlValue(forKey key: String, inSection section: String, content: String) -> String? {
        var currentSection: String?
        for rawLine in content.components(separatedBy: .newlines) {
            let line = stripTomlInlineComment(rawLine).trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("[") && line.hasSuffix("]") {
                currentSection = String(line.dropFirst().dropLast())
                continue
            }
            guard currentSection == section else { continue }
            guard let equalsIndex = line.firstIndex(of: "=") else { continue }
            let lineKey = String(line[..<equalsIndex]).trimmingCharacters(in: .whitespaces)
            if lineKey == key {
                return String(line[line.index(after: equalsIndex)...])
                    .trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }
        return nil
    }

    private func tomlBool(_ value: String) -> Bool? {
        switch value {
        case "true": return true
        case "false": return false
        default: return nil
        }
    }

    private func stripTomlInlineComment(_ line: String) -> String {
        var result = ""
        var inString = false
        var escaped = false

        for char in line {
            if inString {
                result.append(char)
                if escaped {
                    escaped = false
                } else if char == "\\" {
                    escaped = true
                } else if char == "\"" {
                    inString = false
                }
                continue
            }

            if char == "#" {
                break
            }
            result.append(char)
            if char == "\"" {
                inString = true
            }
        }

        return result
    }

    private func decodeTomlBasicString(_ value: String) -> String? {
        guard value.hasPrefix("\""), value.hasSuffix("\"") else { return nil }
        let inner = value.dropFirst().dropLast()
        var result = ""
        var escaped = false

        for char in inner {
            if escaped {
                switch char {
                case "\\": result.append("\\")
                case "\"": result.append("\"")
                case "n": result.append("\n")
                case "t": result.append("\t")
                case "r": result.append("\r")
                default: result.append(char)
                }
                escaped = false
            } else if char == "\\" {
                escaped = true
            } else {
                result.append(char)
            }
        }

        if escaped {
            result.append("\\")
        }
        return result
    }

    private func encodeTomlBasicString(_ value: String) -> String {
        var result = "\""
        for char in value {
            switch char {
            case "\\": result += "\\\\"
            case "\"": result += "\\\""
            case "\n": result += "\\n"
            case "\t": result += "\\t"
            case "\r": result += "\\r"
            default: result.append(char)
            }
        }
        result += "\""
        return result
    }

    private func tomlStringArray(_ values: [String]) -> String {
        "[" + values.map { encodeTomlBasicString($0) }.joined(separator: ", ") + "]"
    }

    private func updateConfigValue(section: String, key: String, value: String) {
        let existing = (try? String(contentsOf: configPath, encoding: .utf8)) ?? defaultConfigText()
        let updated = replacingTomlValue(in: existing, section: section, key: key, value: value)

        do {
            try FileManager.default.createDirectory(
                at: configPath.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            try writePrivateFileAtomically(updated, to: configPath)
        } catch {
            print("Failed to write config.toml: \(error)")
        }
    }

    private func writePrivateFileAtomically(_ content: String, to url: URL) throws {
        let dir = url.deletingLastPathComponent()
        let tempURL = dir.appendingPathComponent(".\(url.lastPathComponent).\(getpid()).\(UUID().uuidString).tmp")
        var fd = open(tempURL.path, O_WRONLY | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR)
        guard fd >= 0 else {
            throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
        }

        do {
            guard let data = content.data(using: .utf8) else {
                throw NSError(domain: NSCocoaErrorDomain, code: CocoaError.fileWriteInapplicableStringEncoding.rawValue)
            }
            try data.withUnsafeBytes { buffer in
                var written = 0
                while written < buffer.count {
                    let result = Darwin.write(fd, buffer.baseAddress!.advanced(by: written), buffer.count - written)
                    if result < 0 && errno == EINTR {
                        continue
                    }
                    if result < 0 {
                        throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
                    }
                    if result == 0 {
                        throw NSError(domain: NSPOSIXErrorDomain, code: Int(EIO))
                    }
                    written += result
                }
            }
            if fsync(fd) != 0 {
                throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
            }
            if close(fd) != 0 {
                let closeError = errno
                fd = -1
                throw NSError(domain: NSPOSIXErrorDomain, code: Int(closeError))
            }
            fd = -1
            if rename(tempURL.path, url.path) != 0 {
                throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
            }
            fsyncDirectory(dir)
        } catch {
            if fd >= 0 {
                close(fd)
            }
            try? FileManager.default.removeItem(at: tempURL)
            throw error
        }
    }

    private func fsyncDirectory(_ dir: URL) {
        let fd = open(dir.path, O_RDONLY)
        guard fd >= 0 else { return }
        _ = fsync(fd)
        close(fd)
    }

    private func replacingTomlValue(in content: String, section: String, key: String, value: String) -> String {
        var lines = content.components(separatedBy: "\n")
        if lines.last == "" { lines.removeLast() }

        var sectionStart: Int?
        var sectionEnd = lines.count

        for (index, rawLine) in lines.enumerated() {
            let line = stripTomlInlineComment(rawLine).trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("[") && line.hasSuffix("]") {
                let name = String(line.dropFirst().dropLast())
                if name == section {
                    sectionStart = index
                    sectionEnd = lines.count
                } else if sectionStart != nil {
                    sectionEnd = index
                    break
                }
            }
        }

        if let start = sectionStart {
            for index in (start + 1)..<sectionEnd {
                let line = stripTomlInlineComment(lines[index]).trimmingCharacters(in: .whitespaces)
                guard let equalsIndex = line.firstIndex(of: "=") else { continue }
                let lineKey = String(line[..<equalsIndex]).trimmingCharacters(in: .whitespaces)
                if lineKey == key {
                    lines[index] = "\(key) = \(value)"
                    return lines.joined(separator: "\n") + "\n"
                }
            }
            lines.insert("\(key) = \(value)", at: sectionEnd)
        } else {
            if !lines.isEmpty { lines.append("") }
            lines.append("[\(section)]")
            lines.append("\(key) = \(value)")
        }

        return lines.joined(separator: "\n") + "\n"
    }

    // ── File monitoring ───────────────────────────────

    func startMonitoring() {
        monitorDataDirectory()

        startPollTimer()
    }

    func restartPollTimer() {
        pollTimer?.invalidate()
        startPollTimer()
    }

    private func startPollTimer() {
        pollTimer = Timer.scheduledTimer(withTimeInterval: pollInterval, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.readStatus()
                self?.readSelectedProvider()
            }
        }
    }

    private func monitorDataDirectory() {
        try? FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)

        let fd = open(dataDir.path, O_EVTONLY)
        guard fd >= 0 else { return }

        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .rename, .delete, .extend, .attrib],
            queue: DispatchQueue.main
        )

        source.setEventHandler { [weak self] in
            self?.readCliSettings()
            self?.readStatus()
            self?.readSelectedProvider()
        }

        source.setCancelHandler {
            close(fd)
        }

        source.resume()

        fileSource = source
    }

    // ── Provider enable/disable ───────────────────────

    /// All provider IDs known from status data
    var allProviderIDs: [String] {
        sortedProviderIDs(status?.providers.map { $0.providerId } ?? Self.defaultProviderOrder)
    }

    /// Display name for a provider ID
    func providerName(for id: String) -> String {
        if let p = status?.providers.first(where: { $0.providerId == id }) {
            return p.providerName
        }
        switch id {
        case "deepseek": return "DeepSeek"
        case "stepfun": return "StepFun"
        case "opencodego": return "OpenCode Go"
        default: return id
        }
    }

    /// Whether a provider is enabled in settings
    func isProviderEnabled(_ id: String) -> Bool {
        providerEnabled[id] ?? true
    }

    /// Enable or disable a provider
    func setProviderEnabled(_ id: String, enabled: Bool) {
        providerEnabled[id] = enabled
        updateConfigValue(
            section: "providers.\(id)",
            key: "enabled",
            value: enabled ? "true" : "false"
        )
    }

    /// Providers filtered by enabled state
    var enabledProviders: [ProviderStatus] {
        guard let s = status else { return [] }
        return sortProviders(s.providers.filter { isProviderEnabled($0.providerId) })
    }

    private func normalizedProviderOrder(_ order: [String]) -> [String] {
        var seen = Set<String>()
        var result: [String] = []

        for id in order where !seen.contains(id) {
            seen.insert(id)
            result.append(id)
        }
        for id in Self.defaultProviderOrder where !seen.contains(id) {
            seen.insert(id)
            result.append(id)
        }
        return result
    }

    private func sortedProviderIDs(_ ids: [String]) -> [String] {
        let rank = Dictionary(uniqueKeysWithValues: providerOrder.enumerated().map { ($0.element, $0.offset) })
        return ids.enumerated().sorted { lhs, rhs in
            let leftRank = rank[lhs.element] ?? Int.max
            let rightRank = rank[rhs.element] ?? Int.max
            if leftRank != rightRank { return leftRank < rightRank }
            return lhs.offset < rhs.offset
        }.map { $0.element }
    }

    private func sortProviders(_ providers: [ProviderStatus]) -> [ProviderStatus] {
        let rank = Dictionary(uniqueKeysWithValues: providerOrder.enumerated().map { ($0.element, $0.offset) })
        return providers.enumerated().sorted { lhs, rhs in
            let leftRank = rank[lhs.element.providerId] ?? Int.max
            let rightRank = rank[rhs.element.providerId] ?? Int.max
            if leftRank != rightRank { return leftRank < rightRank }
            return lhs.offset < rhs.offset
        }.map { $0.element }
    }

    // ── Helpers ───────────────────────────────────────

    /// Parse ISO8601 date string, with or without fractional seconds
    private func parseISO8601(_ ts: String) -> Date? {
        let f1 = ISO8601DateFormatter()
        f1.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let d = f1.date(from: ts) { return d }
        let f2 = ISO8601DateFormatter()
        f2.formatOptions = [.withInternetDateTime]
        return f2.date(from: ts)
    }

    /// Format a relative time string for "Updated X ago"
    func ago(_ ts: String?) -> String {
        guard let ts, let date = parseISO8601(ts) else { return "just now" }
        let diff = Date().timeIntervalSince(date)
        if diff < 60 { return "just now" }
        let m = Int(diff / 60)
        if m < 60 { return "\(m)m ago" }
        let h = m / 60
        if h < 24 { return "\(h)h ago" }
        return "\(h / 24)d ago"
    }

    /// Format a relative time string for "Resets in X"
    func resetIn(_ ts: String?) -> String? {
        guard let ts, let date = parseISO8601(ts) else { return nil }
        let d = date.timeIntervalSinceNow
        if d < 0 { return "now" }
        let h = Int(d / 3600), m = Int((d.truncatingRemainder(dividingBy: 3600)) / 60)
        if h > 24 {
            let days = h / 24, remH = h % 24
            return remH > 0 ? "\(days)d \(remH)h" : "\(days)d"
        }
        if h > 0 { return m > 0 ? "\(h)h\(m)m" : "\(h)h" }
        return "\(m)m"
    }

    /// Get the currently active provider for panel display
    var activeProvider: ProviderStatus? {
        let providers = enabledProviders
        guard !providers.isEmpty else { return nil }
        if let sel = selectedProvider,
           let p = providers.first(where: { $0.providerId == sel }) {
            return p
        }
        return providers.first
    }

    /// Summary text for menu bar button (selected provider only)
    var summaryText: String {
        guard let pr = activeProvider else { return "--" }
        if pr.error != nil { return "ERR" }
        if pr.providerId == "deepseek" {
            let d = pr.details
            let t = d["total_balance"]?.doubleValue ?? 0
            let cu = d["currency"]?.stringValue ?? "CNY"
            let sym = cu == "USD" ? "$" : "¥"
            return "\(sym)\(String(format: "%.2f", t))"
        } else {
            // Quota-style providers: show 5h window remaining percent
            if let rate = pr.details["five_hour_usage_left_rate"]?.doubleValue {
                let pct = Int((rate * 100).rounded())
                return "\(pct)%"
            } else {
                let v = Int(pr.remainingPercent.rounded())
                return "\(v)%"
            }
        }
    }

    /// Short label for menu bar button (DS / SF / OCG)
    var shortLabel: String {
        let providerId = activeProvider?.providerId
        switch providerId {
        case "deepseek": return "DS"
        case "stepfun": return "SF"
        case "opencodego": return "OCG"
        case nil: return "--"
        default:
            return String((providerId ?? "--").prefix(3)).uppercased()
        }
    }

    /// Status color: ok/warn/err based on remaining percent
    func statusColor(_ v: Double) -> String {
        if v < 20 { return "err" }
        if v < 50 { return "warn" }
        return "ok"
    }
}
