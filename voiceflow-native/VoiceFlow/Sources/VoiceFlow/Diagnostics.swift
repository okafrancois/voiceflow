import Foundation

/// Journal de diagnostic écrit dans un fichier.
///
/// `os_log` ne conserve pas les messages de niveau `info` : `log show` ne
/// remonte donc rien, et l'utilisateur comme le développeur se retrouvent
/// sans trace quand quelque chose échoue. Ce journal-ci est toujours lisible,
/// et accessible depuis les réglages.
enum Diagnostics {
    private static let queue = DispatchQueue(label: "fr.okatech.voiceflow.diagnostics")
    private static let maxBytes = 512 * 1024

    static var fileURL: URL {
        let directory = URL.applicationSupportDirectory.appending(path: "VoiceFlow")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory.appending(path: "voiceflow.log")
    }

    static func log(_ message: String) {
        let stamp = Date().formatted(.dateTime.hour().minute().second()
            .locale(Locale(identifier: "fr_FR")))
        let line = "\(stamp)  \(message)\n"
        queue.async {
            let url = fileURL
            rotate(url)
            guard let data = line.data(using: .utf8) else { return }
            if let handle = try? FileHandle(forWritingTo: url) {
                defer { try? handle.close() }
                try? handle.seekToEnd()
                try? handle.write(contentsOf: data)
            } else {
                try? data.write(to: url)
            }
        }
    }

    /// Repart à zéro quand le fichier devient gros : un journal de diagnostic
    /// ne doit pas grossir sans fin.
    private static func rotate(_ url: URL) {
        let attributes = try? FileManager.default
            .attributesOfItem(atPath: url.path(percentEncoded: false))
        guard let size = attributes?[.size] as? Int, size > maxBytes else { return }
        try? FileManager.default.removeItem(at: url)
    }
}
