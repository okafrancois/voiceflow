import AVFoundation

/// Start and stop beeps, reused as-is from the current app
/// (`src-tauri/assets/*.wav`). 300 ms debounce like in `beep.rs`, so a
/// quick back-and-forth doesn't produce a burst.
@MainActor
final class BeepPlayer {
    static let shared = BeepPlayer()

    private var start: AVAudioPlayer?
    private var stop: AVAudioPlayer?
    private var lastPlayed = Date.distantPast
    private static let debounce: TimeInterval = 0.3

    private init() {
        start = Self.load("start_beep")
        stop = Self.load("stop_beep")
    }

    private static func load(_ name: String) -> AVAudioPlayer? {
        guard let url = Bundle.main.url(forResource: name, withExtension: "wav") else {
            log.error("beep asset missing: \(name).wav")
            return nil
        }
        let player = try? AVAudioPlayer(contentsOf: url)
        player?.prepareToPlay()
        return player
    }

    func play(start isStart: Bool) {
        guard Date().timeIntervalSince(lastPlayed) >= Self.debounce else { return }
        lastPlayed = Date()
        let player = isStart ? start : stop
        player?.currentTime = 0
        player?.play()
    }
}
