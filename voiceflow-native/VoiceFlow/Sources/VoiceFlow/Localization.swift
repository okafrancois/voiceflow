import Foundation

/// Translation resolved at runtime.
///
/// SwiftUI only localizes literals written directly in the code:
/// `Text("Réglages")` works, `Text(variable)` doesn't, and
/// `LocalizedStringKey(variable)` doesn't either, because the keys are
/// extracted at compile time. Since half the labels pass through function
/// parameters or enums, everything goes through here.
///
/// Second benefit: by choosing the catalog ourselves, changing language
/// takes effect immediately, without restarting the application.
enum L {
    /// Active catalog. `nil` = the one macOS chose at launch.
    /// Read from any thread (engine error messages): the lock protects
    /// the exchange.
    nonisolated(unsafe) private static var storedOverride: Bundle?
    private static let lock = NSLock()
    private static var override: Bundle? {
        get { lock.withLock { storedOverride } }
        set { lock.withLock { storedOverride = newValue } }
    }

    /// Language forced by the user; empty to follow the system.
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

    /// Interface language, used to format language names and dates.
    static var locale: Locale {
        if let code = override?.bundlePath.split(separator: "/").last?
            .replacingOccurrences(of: ".lproj", with: "") {
            return Locale(identifier: code)
        }
        return Locale(identifier: Bundle.main.preferredLocalizations.first ?? "fr")
    }

    /// Translates a key — the French label, which also serves as the
    /// default value if the translation is missing.
    static func t(_ key: String) -> String {
        (override ?? .main).localizedString(forKey: key, value: key, table: nil)
    }
}
