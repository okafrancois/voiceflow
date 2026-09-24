import AVFoundation
import CoreAudio

/// Available microphones.
///
/// The list comes from AVFoundation, which only knows about real
/// microphones — querying CoreAudio directly also surfaced outputs and
/// the aggregate devices the system creates (VPAUAggregate…, CADefault…).
/// The CoreAudio identifier is still needed: it's what `AVAudioEngine`
/// expects to switch input.
enum AudioDevices {
    struct Device: Identifiable, Hashable {
        let id: AudioDeviceID
        let name: String
    }

    /// Special value: follow the system's default input device.
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

    /// UID → CoreAudio identifier table, to bridge the two worlds.
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
        // CoreAudio writes a CFString reference: go through a typed
        // pointer rather than a plain variable, which would be misread.
        var size = UInt32(MemoryLayout<CFString?>.size)
        var result: CFString?
        let status = withUnsafeMutablePointer(to: &result) { pointer in
            AudioObjectGetPropertyData(id, &address, 0, nil, &size, pointer)
        }
        guard status == noErr, let uid = result as String?, !uid.isEmpty else { return nil }
        return uid
    }
}
