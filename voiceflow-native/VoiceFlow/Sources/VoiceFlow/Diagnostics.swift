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
                _ = try? handle.seekToEnd()
                try? handle.write(contentsOf: data)
            } else {
                try? data.write(to: url)
            }
        }
    }

    /// Journal précédent, gardé à la rotation : effacer d'un coup perdait
    /// justement les dernières dictées, celles qu'on vient investiguer.
    static var previousFileURL: URL {
        fileURL.deletingPathExtension().appendingPathExtension("previous.log")
    }

    /// Un journal de diagnostic ne doit pas grossir sans fin : au-delà de la
    /// taille maximale, il devient le journal précédent.
    private static func rotate(_ url: URL) {
        let attributes = try? FileManager.default
            .attributesOfItem(atPath: url.path(percentEncoded: false))
        guard let size = attributes?[.size] as? Int, size > maxBytes else { return }
        let previous = previousFileURL
        try? FileManager.default.removeItem(at: previous)
        do {
            try FileManager.default.moveItem(at: url, to: previous)
        } catch {
            try? FileManager.default.removeItem(at: url)
        }
    }
}
