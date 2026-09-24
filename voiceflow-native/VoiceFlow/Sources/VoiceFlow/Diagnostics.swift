import Foundation

/// Diagnostic log written to a file.
///
/// `os_log` doesn't retain `info`-level messages: `log show` therefore
/// surfaces nothing, leaving both user and developer without a trace when
/// something fails. This log stays readable, and accessible from
/// settings.
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
            .locale(Locale(identifier: "en_US_POSIX")))
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

    /// Previous log, kept at rotation time: wiping it outright would lose
    /// exactly the most recent dictations, the ones being investigated.
    static var previousFileURL: URL {
        fileURL.deletingPathExtension().appendingPathExtension("previous.log")
    }

    /// A diagnostic log must not grow forever: past the maximum size, it
    /// becomes the previous log.
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
