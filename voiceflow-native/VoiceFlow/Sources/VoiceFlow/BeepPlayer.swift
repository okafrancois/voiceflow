import AVFoundation

/// Bips de début et de fin, repris tels quels de l'app actuelle
/// (`src-tauri/assets/*.wav`). Anti-rebond de 300 ms comme dans `beep.rs`,
/// pour qu'un aller-retour rapide ne produise pas une rafale.
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
