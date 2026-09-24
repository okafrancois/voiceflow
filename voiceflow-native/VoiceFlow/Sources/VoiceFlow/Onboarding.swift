import SwiftUI

/// First launch: permissions, language, shortcut, try it out. The steps
/// follow the order of what actually blocks dictation.
struct OnboardingView: View {
    @ObservedObject var state: AppState
    var finish: () -> Void
    @State private var step = 0

    private static let steps = ["Bienvenue", "Autorisations", "Langue", "Raccourci", "Essai"]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                ForEach(Array(Self.steps.enumerated()), id: \.offset) { index, _ in
                    Capsule()
                        .fill(index <= step ? VF.label : VF.border)
                        .frame(height: 3)
                }
            }
            .padding(.horizontal, 32)
            .padding(.top, 28)

            ScrollView {
                content
                    .padding(32)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            Divider()

            HStack {
                if step > 0 {
                    Button(L.t("Précédent")) { step -= 1 }
                        .buttonStyle(VFButtonStyle())
                }
                Spacer()
                Button(L.t("Passer")) { finish() }
                    .buttonStyle(VFButtonStyle())
                Button(step == Self.steps.count - 1 ? "Commencer" : "Suivant") {
                    step == Self.steps.count - 1 ? finish() : (step += 1)
                }
                .buttonStyle(VFButtonStyle(prominent: true))
            }
            .padding(20)
        }
        .frame(width: 620, height: 560)
        .background(VF.background)
        .onAppear { state.refreshPermissions() }
    }

    @ViewBuilder
    private var content: some View {
        switch step {
        case 0: welcome
        case 1: permissions
        case 2: language
        case 3: shortcut
        default: tryItOut
        }
    }

    private var welcome: some View {
        VStack(alignment: .leading, spacing: 18) {
            VFLogo(size: 56)
            Text(L.t("Bienvenue dans Voice Flow"))
                .font(.system(size: 26, weight: .semibold))
                .foregroundStyle(VF.label)
            Text(L.t("Dictez dans n'importe quelle application : le texte s'insère là où se trouve votre curseur."))
                .font(.system(size: 15))
                .foregroundStyle(VF.labelMuted)
            Text(L.t("Tout se passe sur cet appareil. Ni votre voix ni vos textes ne quittent le Mac."))
                .font(.system(size: 14))
                .foregroundStyle(VF.labelMuted)
        }
    }

    private var permissions: some View {
        VStack(alignment: .leading, spacing: 18) {
            stepTitle("Deux autorisations", "Sans elles, la dictée ne peut ni écouter ni écrire.")
            VFCard(padding: 0) {
                VStack(spacing: 0) {
                    permissionRow(
                        "Microphone", "Pour capter votre voix.",
                        granted: state.microphoneGranted, pane: "Privacy_Microphone")
                    Rectangle().fill(VF.divider).frame(height: 1)
                    permissionRow(
                        "Accessibilité", "Pour insérer le texte et écouter le raccourci.",
                        granted: state.accessibilityGranted, pane: "Privacy_Accessibility")
                }
            }
            Text(L.t("Après avoir accordé l'accessibilité, relancez l'application : le raccourci global est mis en place au lancement."))
                .font(.system(size: 13))
                .foregroundStyle(VF.labelMuted)
        }
    }

    private var language: some View {
        VStack(alignment: .leading, spacing: 18) {
            stepTitle("Votre langue de dictée", "Elle est indépendante de la langue de votre Mac.")
            VFCard {
                HStack {
                    Text(L.t("Langue")).font(.system(size: 14)).foregroundStyle(VF.label)
                    Spacer()
                    Picker("", selection: $state.dictationLocaleID) {
                        ForEach(state.availableLocaleIDs, id: \.self) { id in
                            Text(state.displayName(for: id)).tag(id)
                        }
                    }
                    .labelsHidden().fixedSize()
                }
            }
            Text(L.t("Le moteur d'Apple est utilisé par défaut : rien à télécharger, et la transcription s'affiche pendant que vous parlez. Les modèles Whisper restent disponibles dans les réglages."))
                .font(.system(size: 13))
                .foregroundStyle(VF.labelMuted)
        }
    }

    private var shortcut: some View {
        VStack(alignment: .leading, spacing: 18) {
            stepTitle("Votre raccourci", "Une touche seule fonctionne aussi : Fn, ou une touche de fonction.")
            VFCard {
                VStack(spacing: 14) {
                    HStack {
                        Text(L.t("Raccourci")).font(.system(size: 14)).foregroundStyle(VF.label)
                        Spacer()
                        ShortcutRecorder(state: state, keyPath: \.dictateShortcutSetting)
                    }
                    HStack {
                        Text(L.t("Déclenchement")).font(.system(size: 14)).foregroundStyle(VF.label)
                        Spacer()
                        Picker("", selection: $state.triggerMode) {
                            ForEach(TriggerMode.allCases) { Text($0.title).tag($0) }
                        }
                        .labelsHidden().fixedSize()
                    }
                }
            }
            Text(state.triggerMode.help)
                .font(.system(size: 13))
                .foregroundStyle(VF.labelMuted)
        }
    }

    private var tryItOut: some View {
        VStack(alignment: .leading, spacing: 18) {
            stepTitle("Essayez", "Ouvrez n'importe quel champ de texte et dictez.")
            VFCard {
                VStack(alignment: .leading, spacing: 10) {
                    Text(state.triggerHint)
                        .font(.system(size: 14))
                        .foregroundStyle(VF.label)
                    Text(L.t("Une capsule apparaît pendant la dictée. Vous pouvez la déplacer où vous voulez."))
                        .font(.system(size: 13))
                        .foregroundStyle(VF.labelMuted)
                }
            }
            Text(L.t("Tout se règle ensuite dans la fenêtre : styles de polissage, dictionnaire, extraits, apparence."))
                .font(.system(size: 13))
                .foregroundStyle(VF.labelMuted)
        }
    }

    /// The mic goes through a system dialog; accessibility through Apple's
    /// prompt, which itself opens System Settings.
    private func request(_ pane: String) {
        if pane.contains("Microphone") {
            Task { await state.requestMicrophone() }
        } else {
            state.requestAccessibility()
        }
    }

    private func stepTitle(_ title: String, _ subtitle: String) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(L.t(title)).font(.system(size: 22, weight: .semibold)).foregroundStyle(VF.label)
            Text(L.t(subtitle)).font(.system(size: 14)).foregroundStyle(VF.labelMuted)
        }
    }

    private func permissionRow(
        _ title: String, _ detail: String, granted: Bool, pane: String
    ) -> some View {
        HStack(spacing: 12) {
            Image(systemName: granted ? "checkmark.circle.fill" : "exclamationmark.circle")
                .font(.system(size: 16))
                .foregroundStyle(granted ? VF.green : VF.amber)
            VStack(alignment: .leading, spacing: 2) {
                Text(L.t(title)).font(.system(size: 14)).foregroundStyle(VF.label)
                Text(L.t(detail)).font(.system(size: 12)).foregroundStyle(VF.labelMuted)
            }
            Spacer()
            if granted {
                Text(L.t("Accordé")).font(.system(size: 13)).foregroundStyle(VF.green)
            } else {
                Button(L.t("Autoriser…")) { request(pane) }
                    .buttonStyle(VFButtonStyle())
            }
        }
        .padding(16)
    }
}
