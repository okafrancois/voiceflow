import AVFoundation
import ApplicationServices

enum Permissions {
    /// Requests microphone access (shows the system prompt on first launch).
    static func requestMicrophone() async -> Bool {
        await AVAudioApplication.requestRecordPermission()
    }

    /// Reads the state without showing a prompt.
    static func isAccessibilityTrusted() -> Bool {
        AXIsProcessTrusted()
    }

    static func isMicrophoneGranted() -> Bool {
        AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
    }

    /// Checks Accessibility trust and shows the system prompt if absent.
    /// Required for the CGEventTap (shortcut) and injection.
    @discardableResult
    static func ensureAccessibility() -> Bool {
        // Value of `kAXTrustedCheckOptionPrompt`, a C global variable that
        // Swift 6 refuses to read outside isolation.
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        return AXIsProcessTrustedWithOptions(options)
    }
}
