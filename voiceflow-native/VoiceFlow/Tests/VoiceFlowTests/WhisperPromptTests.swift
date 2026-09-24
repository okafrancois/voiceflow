import Testing
@testable import VoiceFlow

struct WhisperPromptTests {
    @Test func anEchoedGlossaryIsDropped() {
        let hints = ["Anthropic", "Kubernetes"]
        #expect(WhisperEngine.removingEchoedPrompt("Anthropic, Kubernetes.", hints: hints) == "")
        #expect(WhisperEngine.removingEchoedPrompt("Anthropic, Kubernetes. Bonjour.", hints: hints) == "Bonjour.")
    }

    @Test func ordinaryTextUsingATermIsKept() {
        #expect(WhisperEngine.removingEchoedPrompt("Déployé sur Kubernetes.", hints: ["Kubernetes"])
            == "Déployé sur Kubernetes.")
    }
}
