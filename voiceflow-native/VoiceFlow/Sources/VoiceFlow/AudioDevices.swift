import AVFoundation
import CoreAudio

/// Micros disponibles.
///
/// La liste vient d'AVFoundation, qui ne connaît que de vrais microphones —
/// interroger CoreAudio directement remontait aussi les sorties et les
/// périphériques agrégés créés par le système (VPAUAggregate…, CADefault…).
/// L'identifiant CoreAudio reste nécessaire : c'est lui qu'attend
/// `AVAudioEngine` pour changer d'entrée.
enum AudioDevices {
    struct Device: Identifiable, Hashable {
        let id: AudioDeviceID
        let name: String
    }

    /// Valeur spéciale : suivre le périphérique d'entrée par défaut du système.
    static let systemDefaultID: AudioDeviceID = 0

    static func inputs() -> [Device] {
        let session = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.microphone, .external],
            mediaType: .audio,
            position: .unspecified)

        let byUID = coreAudioInputsByUID()
        return session.devices.compactMap { device in
            guard let id = byUID[device.uniqueID] else { return nil }
            return Device(id: id, name: device.localizedName)
        }
    }

    static func exists(_ id: AudioDeviceID) -> Bool {
        id == systemDefaultID || inputs().contains { $0.id == id }
    }

    /// Table UID → identifiant CoreAudio, pour relier les deux mondes.
    private static func coreAudioInputsByUID() -> [String: AudioDeviceID] {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain)

        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(
            AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size) == noErr
        else { return [:] }

        var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
        guard AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &ids) == noErr
        else { return [:] }

        var table: [String: AudioDeviceID] = [:]
        for id in ids {
            if let uid = uid(of: id) { table[uid] = id }
        }
        return table
    }

    private static func uid(of id: AudioDeviceID) -> String? {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyDeviceUID,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain)
        // CoreAudio écrit une référence CFString : passer par un pointeur
        // typé plutôt que par une variable, qui serait mal interprétée.
        var size = UInt32(MemoryLayout<CFString?>.size)
        var result: CFString?
        let status = withUnsafeMutablePointer(to: &result) { pointer in
            AudioObjectGetPropertyData(id, &address, 0, nil, &size, pointer)
        }
        guard status == noErr, let uid = result as String?, !uid.isEmpty else { return nil }
        return uid
    }
}
