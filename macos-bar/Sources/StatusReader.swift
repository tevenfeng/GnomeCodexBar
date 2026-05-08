import Foundation
import Combine

/// Reads status.json and selected_provider.json, monitors for changes.
@MainActor
class StatusReader: ObservableObject {
    @Published var status: StatusSnapshot?
    @Published var selectedProvider: String?
    @Published var pollInterval: Double

    private var fileSource: DispatchSourceFileSystemObject?
    private var selectedSource: DispatchSourceFileSystemObject?
    private var pollTimer: Timer?

    private let statusPath: URL
    private let selectedPath: URL

    init() {
        // Use the same directory as the Rust CLI (dirs::data_local_dir())
        // On macOS this is ~/Library/Application Support/gnome-codex-bar/
        let dataDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/gnome-codex-bar")
        self.statusPath = dataDir.appendingPathComponent("status.json")
        self.selectedPath = dataDir.appendingPathComponent("selected_provider.json")

        // Load saved poll interval (default: 5s)
        self.pollInterval = UserDefaults.standard.object(forKey: "pollInterval") as? Double ?? 5.0

        readStatus()
        readSelectedProvider()
        startMonitoring()
    }

    // ── Read ──────────────────────────────────────────

    func readStatus() {
        guard let data = try? Data(contentsOf: statusPath) else {
            status = nil
            return
        }
        status = try? JSONDecoder().decode(StatusSnapshot.self, from: data)
    }

    func readSelectedProvider() {
        guard let data = try? Data(contentsOf: selectedPath),
              let str = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
              !str.isEmpty else {
            selectedProvider = nil
            return
        }
        selectedProvider = str
    }

    /// Write selected provider ID to selected_provider.json
    func writeSelectedProvider(_ id: String) {
        try? id.data(using: .utf8)?.write(to: selectedPath, options: .atomic)
        selectedProvider = id
    }

    // ── File monitoring ───────────────────────────────

    func startMonitoring() {
        monitorFile(at: statusPath) { [weak self] in
            self?.readStatus()
        }
        monitorFile(at: selectedPath) { [weak self] in
            self?.readSelectedProvider()
        }

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

    private func monitorFile(at url: URL, onChange: @escaping () -> Void) {
        let path = url.path
        guard FileManager.default.fileExists(atPath: path) else { return }

        let fd = open(path, O_EVTONLY)
        guard fd >= 0 else { return }

        let source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: .write,
            queue: DispatchQueue.main
        )

        source.setEventHandler {
            onChange()
        }

        source.setCancelHandler {
            close(fd)
        }

        source.resume()

        // Store the source to keep it alive
        if url.path.contains("status.json") && !url.path.contains("selected") {
            fileSource = source
        } else {
            selectedSource = source
        }
    }

    // ── Provider enable/disable ───────────────────────

    /// All provider IDs known from status data
    var allProviderIDs: [String] {
        status?.providers.map { $0.providerId } ?? ["deepseek", "stepfun"]
    }

    /// Display name for a provider ID
    func providerName(for id: String) -> String {
        if let p = status?.providers.first(where: { $0.providerId == id }) {
            return p.providerName
        }
        return id == "deepseek" ? "DeepSeek" : "StepFun"
    }

    /// Whether a provider is enabled in settings
    func isProviderEnabled(_ id: String) -> Bool {
        // Default to enabled if not explicitly set
        let key = "provider_enabled_\(id)"
        return UserDefaults.standard.object(forKey: key) as? Bool ?? true
    }

    /// Enable or disable a provider
    func setProviderEnabled(_ id: String, enabled: Bool) {
        let key = "provider_enabled_\(id)"
        UserDefaults.standard.set(enabled, forKey: key)
        // Trigger UI refresh
        objectWillChange.send()
    }

    /// Providers filtered by enabled state
    var enabledProviders: [ProviderStatus] {
        guard let s = status else { return [] }
        return s.providers.filter { isProviderEnabled($0.providerId) }
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
        guard let s = status else { return nil }
        if let sel = selectedProvider,
           let p = s.providers.first(where: { $0.providerId == sel }) {
            return p
        }
        return s.providers.first
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
            // StepFun: show 5h window remaining percent
            if let rate = pr.details["five_hour_usage_left_rate"]?.doubleValue {
                let pct = Int((rate * 100).rounded())
                return "\(pct)%"
            } else {
                let v = Int(pr.remainingPercent.rounded())
                return "\(v)%"
            }
        }
    }

    /// Short label for menu bar button (DS / SF)
    var shortLabel: String {
        (selectedProvider ?? "stepfun") == "deepseek" ? "DS" : "SF"
    }

    /// Status color: ok/warn/err based on remaining percent
    func statusColor(_ v: Double) -> String {
        if v < 20 { return "err" }
        if v < 50 { return "warn" }
        return "ok"
    }
}
