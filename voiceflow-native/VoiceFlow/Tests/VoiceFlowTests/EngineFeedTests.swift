import AVFoundation
import Testing
@testable import VoiceFlow

/// The microphone starts before the engine is ready: nothing said in that
/// window may be lost, and the order of the audio must be kept.
struct EngineFeedTests {
    /// Only ever used from one thread in these tests.
    private final class RecordingEngine: DictationEngine, @unchecked Sendable {
        var firstSamples: [Float] = []
        func feed(_ buffer: AVAudioPCMBuffer) {
            firstSamples.append(buffer.floatChannelData![0][0])
        }
        func finish() async throws -> String { "" }
    }

    private func buffer(_ value: Float) -> AVAudioPCMBuffer {
        let format = AVAudioFormat(standardFormatWithSampleRate: 16000, channels: 1)!
        let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4)!
        buffer.frameLength = 4
        for index in 0..<4 { buffer.floatChannelData![0][index] = value }
        return buffer
    }

    @Test func audioCapturedBeforeTheEngineIsDeliveredInOrder() {
        let feed = EngineFeed()
        feed.push(buffer(1))
        feed.push(buffer(2))
        let engine = RecordingEngine()
        feed.attach(engine)
        feed.push(buffer(3))
        #expect(engine.firstSamples == [1, 2, 3])
    }

    @Test func queuedAudioIsCopiedBecauseTheTapReusesItsBuffers() {
        let feed = EngineFeed()
        let reused = buffer(1)
        feed.push(reused)
        reused.floatChannelData![0][0] = 9
        let engine = RecordingEngine()
        feed.attach(engine)
        #expect(engine.firstSamples == [1])
    }

    @Test func pendingDurationIsReported() {
        let feed = EngineFeed()
        feed.push(buffer(1))
        feed.push(buffer(1))
        #expect(feed.pendingFrames == 8)
        feed.attach(RecordingEngine())
        #expect(feed.pendingFrames == 0)
    }
}
