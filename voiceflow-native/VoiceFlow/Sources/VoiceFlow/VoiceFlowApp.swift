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

        Window("Bienvenue", id: "onboarding") {
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

/// Ouvre l'accueil au premier lancement.
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
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Group {
            Button(L.t("Ouvrir VoiceFlow")) {
                openWindow(id: "main")
                NSApp.activate()
            }
            .keyboardShortcut(",")

            Button(L.t("Coller la dernière transcription")) {
                state.reinsertLast()
            }
            .disabled(state.lastTranscript.isEmpty)

            Divider()

            Button(L.t("Quitter VoiceFlow")) {
                NSApplication.shared.terminate(nil)
            }
            .keyboardShortcut("q")
        }
    }
}
