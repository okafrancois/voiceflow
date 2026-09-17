import Foundation

/// Vérification des mises à jour par simple flux JSON.
///
/// Le flux attendu :
/// ```json
/// { "version": "0.2.0", "notes": "…", "url": "https://…/VoiceFlow.dmg" }
/// ```
/// Rien n'est installé automatiquement : l'app signale la version et ouvre le
/// lien de téléchargement. L'installation silencieuse demanderait Sparkle et
/// une paire de clés de signature, ce qui n'a de sens qu'une fois la
/// distribution en place.
@MainActor
final class UpdateChecker: ObservableObject {
    static let shared = UpdateChecker()

    struct Release: Decodable {
        let version: String
        let notes: String?
        let url: String?
    }

    @Published var latest: Release?
    @Published var lastCheck: Date?
    @Published var checking = false
    @Published var error: String?

    /// Flux de la dernière release publiée : l'URL « latest » ne change pas
    /// d'une version à l'autre.
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

    /// Vérifie au lancement, au plus une fois par jour.
    func checkIfDue() async {
        guard automatic, !feedURL.isEmpty else { return }
        if let lastCheck, Date().timeIntervalSince(lastCheck) < 86400 { return }
        await check()
    }

    func check() async {
        guard let url = URL(string: feedURL), !feedURL.isEmpty else {
            error = "Aucune adresse de flux configurée."
            return
        }
        checking = true
        error = nil
        defer { checking = false }
        do {
            let (data, _) = try await URLSession.shared.data(from: url)
            latest = try JSONDecoder().decode(Release.self, from: data)
            lastCheck = Date()
        } catch {
            self.error = "Vérification impossible : \(error.localizedDescription)"
        }
    }
}
