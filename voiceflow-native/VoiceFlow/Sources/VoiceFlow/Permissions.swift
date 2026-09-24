import AVFoundation
import ApplicationServices

enum Permissions {
    /// Demande l'accès micro (affiche la boîte système au premier lancement).
    static func requestMicrophone() async -> Bool {
        await AVAudioApplication.requestRecordPermission()
    }

    /// Lit l'état sans afficher d'invite.
    static func isAccessibilityTrusted() -> Bool {
        AXIsProcessTrusted()
    }

    static func isMicrophoneGranted() -> Bool {
        AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
    }

    /// Vérifie la confiance Accessibilité et affiche l'invite système si absente.
    /// Nécessaire pour le CGEventTap (raccourci) et l'injection.
    @discardableResult
    static func ensureAccessibility() -> Bool {
        // Valeur de `kAXTrustedCheckOptionPrompt`, variable globale C que
        // Swift 6 refuse de lire hors isolation.
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        return AXIsProcessTrustedWithOptions(options)
    }
}
