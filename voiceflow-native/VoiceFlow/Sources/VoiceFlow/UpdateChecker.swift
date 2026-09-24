import Foundation

/// Update checking via a simple JSON feed.
///
/// Expected feed:
/// ```json
/// { "version": "0.2.0", "notes": "…", "url": "https://…/VoiceFlow.dmg" }
/// ```
/// Nothing is installed automatically: the app reports the version and
/// opens the download link. Silent installation would require Sparkle
/// and a signing key pair, which only makes sense once distribution is
/// in place.
@MainActor
final class UpdateChecker: ObservableObject {
    static let shared = UpdateChecker()

    struct Release: Decodable {
        let version: String
        let notes: String?
        let url: String?
    }

    @Published var latest: Release?
    /// Kept from one launch to the next: without it, "once a day" would
    /// mean "on every launch".
    @Published var lastCheck: Date? = UserDefaults.standard.object(forKey: "lastUpdateCheck") as? Date {
        didSet { UserDefaults.standard.set(lastCheck, forKey: "lastUpdateCheck") }
    }
    @Published var checking = false
    @Published var error: String?

    /// Feed of the latest published release: the "latest" URL doesn't
    /// change from one version to the next.
    static let defaultFeed =
        "https://github.com/okafrancois/voiceflow/releases/latest/download/appcast.json"

    @Published var feedURL: String = UserDefaults.standard.string(forKey: "updateFeed") ?? UpdateChecker.defaultFeed {
        didSet { UserDefaults.standard.set(feedURL, forKey: "updateFeed") }
    }

    @Published var automatic = UserDefaults.standard.object(forKey: "autoUpdateCheck") as? Bool ?? true {
        didSet { UserDefaults.standard.set(automatic, forKey: "autoUpdateCheck") }
    }

    var currentVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.0.0"
    }

    var updateAvailable: Bool {
        guard let latest else { return false }
        return latest.version.compare(currentVersion, options: .numeric) == .orderedDescending
    }

    /// Checks at launch, at most once a day.
    func checkIfDue() async {
        guard automatic, !feedURL.isEmpty else { return }
        if let lastCheck, Date().timeIntervalSince(lastCheck) < 86400 { return }
        await check()
    }

    func check() async {
        guard let url = URL(string: feedURL), !feedURL.isEmpty else {
            error = L.t("Aucune adresse de flux configurée.")
            return
        }
        checking = true
        error = nil
        defer { checking = false }
        do {
            let (data, response) = try await URLSession.shared.data(from: url)
            if let http = response as? HTTPURLResponse, http.statusCode != 200 {
                throw URLError(.badServerResponse)
            }
            latest = try JSONDecoder().decode(Release.self, from: data)
            lastCheck = Date()
        } catch {
            self.error = L.t("Vérification impossible") + " : \(error.localizedDescription)"
        }
    }
}
