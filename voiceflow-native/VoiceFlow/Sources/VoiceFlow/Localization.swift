import Foundation

/// Traduction résolue à l'exécution.
///
/// SwiftUI ne localise que les littéraux écrits directement dans le code :
/// `Text("Réglages")` fonctionne, `Text(variable)` non, et
/// `LocalizedStringKey(variable)` pas davantage, car les clés sont extraites
/// à la compilation. Comme la moitié des libellés transitent par des
/// paramètres de fonction ou des énumérations, tout passe ici.
///
/// Second bénéfice : en choisissant nous-même le catalogue, changer de langue
/// prend effet immédiatement, sans relancer l'application.
enum L {
    /// Catalogue actif. `nil` = celui que macOS a choisi au lancement.
    /// Lu depuis n'importe quel fil (messages d'erreur des moteurs) : le
    /// verrou protège l'échange.
    nonisolated(unsafe) private static var storedOverride: Bundle?
    private static let lock = NSLock()
    private static var override: Bundle? {
        get { lock.withLock { storedOverride } }
        set { lock.withLock { storedOverride = newValue } }
    }

    /// Langue forcée par l'utilisateur ; vide pour suivre le système.
    static func setLanguage(_ code: String) {
        guard !code.isEmpty,
              let path = Bundle.main.path(forResource: code, ofType: "lproj"),
              let bundle = Bundle(path: path)
        else {
            override = nil
            return
        }
        override = bundle
    }

    /// Langue de l'interface, pour formater noms de langues et dates.
    static var locale: Locale {
        if let code = override?.bundlePath.split(separator: "/").last?
            .replacingOccurrences(of: ".lproj", with: "") {
            return Locale(identifier: code)
        }
        return Locale(identifier: Bundle.main.preferredLocalizations.first ?? "fr")
    }

    /// Traduit une clé — le libellé français, qui sert aussi de valeur par
    /// défaut si la traduction manque.
    static func t(_ key: String) -> String {
        (override ?? .main).localizedString(forKey: key, value: key, table: nil)
    }
}
