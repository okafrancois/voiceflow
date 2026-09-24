import SwiftUI

@main
struct VoiceFlowApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var state = AppState.shared

    var body: some Scene {
        MenuBarExtra {
            MenuContent(state: state)
        } label: {
            Image(systemName: state.menuBarSymbol)
                .modifier(OnboardingLauncher(state: state))
        }

        Window("VoiceFlow", id: "main") {
            MainWindow(state: state)
        }
        .defaultSize(width: 1120, height: 780)
        .windowStyle(.hiddenTitleBar)

        Window(L.t("Bienvenue"), id: "onboarding") {
            OnboardingView(state: state) {
                state.completeOnboarding()
                NSApp.keyWindow?.close()
            }
        }
        .windowResizability(.contentSize)
        .windowStyle(.hiddenTitleBar)
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        Task { @MainActor in
            await AppState.shared.bootstrap()
        }
    }
}

/// Opens onboarding on first launch.
struct OnboardingLauncher: ViewModifier {
    @ObservedObject var state: AppState
    @Environment(\.openWindow) private var openWindow

    func body(content: Content) -> some View {
        content.task {
            guard state.needsOnboarding else { return }
            openWindow(id: "onboarding")
            NSApp.activate()
        }
    }
}

struct MenuContent: View {
    @ObservedObject var state: AppState
    @ObservedObject private var whisperModels = WhisperModelStore.shared
    @ObservedObject private var sherpaModels = SherpaModelStore.shared
    @Environment(\.openWindow) private var openWindow

    /// Only engines usable right away: the system one, and models already
    /// downloaded.
    private var readyEngines: [EngineChoice] {
        EngineChoice.allCases.filter { choice in
            if let variant = choice.whisperModel { return whisperModels.isDownloaded(variant) }
            if let model = choice.sherpaModel { return sherpaModels.isDownloaded(model) }
            return true
        }
    }

    var body: some View {
        Group {
            switch state.phase {
            case .idle:
                Button(L.t("Démarrer une dictée")) { state.startDictation() }
            case .recording:
                Button(L.t("Arrêter et insérer")) { state.toggleDictation() }
            case .transcribing, .polishing:
                Text(state.readinessTitle)
            }
            if state.phase != .idle {
                Button(L.t("Annuler la dictée")) { state.cancelDictation() }
            }

            Divider()

            Button(L.t("Coller la dernière transcription")) {
                state.reinsertLast()
            }
            .disabled(state.lastTranscript.isEmpty)

            Button(L.t("Retirer la dernière insertion")) {
                state.undoLastInsertion()
            }
            .disabled(!state.canUndoLastInsertion)

            Divider()

            Picker(L.t("Moteur"), selection: $state.engineChoiceID) {
                ForEach(readyEngines) { Text($0.displayName).tag($0.rawValue) }
            }
            Picker(L.t("Langue"), selection: $state.dictationLocaleID) {
                Text(state.displayName(for: AppState.autoLocaleID)).tag(AppState.autoLocaleID)
                Divider()
                ForEach(state.availableLocaleIDs, id: \.self) { id in
                    Text(state.displayName(for: id)).tag(id)
                }
            }
            Toggle(L.t("Polir chaque dictée"), isOn: $state.polishEnabled)

            Divider()

            Button(L.t("Ouvrir VoiceFlow")) {
                openWindow(id: "main")
                NSApp.activate()
            }
            .keyboardShortcut(",")

            Button(L.t("Quitter VoiceFlow")) {
                NSApplication.shared.terminate(nil)
            }
            .keyboardShortcut("q")
        }
    }
}
