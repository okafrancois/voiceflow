import AppKit
import Combine
import SwiftUI

/// Panneau flottant de la pill.
///
/// Le verre vient de `NSGlassEffectView` (AppKit, macOS 26+) et non du
/// modificateur SwiftUI : dans un panneau borderless à fond transparent,
/// `glassEffect` ne récupère pas l'arrière-plan de la fenêtre et retombe sur
/// un matériau plat. La vue AppKit, elle, composite au niveau du serveur de
/// fenêtres — c'est le vrai matériau système, qui suit les réglages
/// d'apparence et de transparence.
@MainActor
final class PillController {
    static let shared = PillController()

    private var panel: PillPanel?
    private var glassView: NSGlassEffectView?
    private var hostingView: NSHostingView<PillView>?
    private var cancellables = Set<AnyCancellable>()

    private static let centerKey = "pillCenter"
    private var isHidden = true

    func attach(to state: AppState) {
        guard panel == nil else { return }

        let hosting = NSHostingView(rootView: PillView(state: state))

        let glass = DraggableGlassView()
        glass.style = .regular
        glass.contentView = hosting

        let panel = PillPanel(
            contentRect: NSRect(origin: .zero, size: Self.baseSize),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isFloatingPanel = true
        panel.level = .statusBar
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.contentView = glass

        self.panel = panel
        self.glassView = glass
        self.hostingView = hosting

        applyAppearance()
    }

    /// Applique teinte, opacité, taille et position choisies dans les réglages.
    func applyAppearance() {
        guard let panel, let glassView else { return }
        let state = AppState.shared
        glassView.tintColor = NSColor(hex: state.pillTint.hex)
            .withAlphaComponent(state.pillOpacity)
        resize()
        // Repositionner tout de suite si un ancrage est choisi.
        if state.pillPosition != .free {
            let size = pillSize
            let center = storedCenter()
            panel.setFrame(
                NSRect(
                    x: (center.x - size.width / 2).rounded(),
                    y: (center.y - size.height / 2).rounded(),
                    width: size.width, height: size.height),
                display: true)
        }
        refreshVisibility()
    }

    /// La pill peut être toujours visible, visible pendant la dictée, ou jamais.
    private func refreshVisibility() {
        guard let panel else { return }
        switch AppState.shared.pillVisibility {
        case .never:
            panel.orderOut(nil)
        case .always:
            panel.alphaValue = 1
            panel.orderFrontRegardless()
        case .whileActive:
            if isHidden { panel.orderOut(nil) }
        }
    }

    func apply(phase: AppState.Phase) {
        guard let panel else { return }
        guard AppState.shared.pillVisibility != .never else {
            panel.orderOut(nil)
            return
        }
        switch phase {
        case .recording, .transcribing, .polishing:
            isHidden = false
            resize()
            panel.alphaValue = 0
            panel.orderFrontRegardless()
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.18
                panel.animator().alphaValue = 1
            }
        case .idle, .preparing:
            isHidden = true
            // En mode « toujours », la pill reste à l'écran au repos.
            guard AppState.shared.pillVisibility != .always else {
                panel.alphaValue = 1
                panel.orderFrontRegardless()
                return
            }
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.15
                panel.animator().alphaValue = 0
            } completionHandler: {
                // Une dictée a pu redémarrer pendant le fondu : ne pas
                // masquer une pill redevenue utile.
                Task { @MainActor in
                    guard self.isHidden else { return }
                    panel.orderOut(nil)
                }
            }
        }
    }

    /// Ajuste la taille du panneau au contenu, en gardant le centre fixe
    /// pour que la pill grandisse symétriquement.
    /// Taille de la pill : celle de l'original, calculée depuis le contenu
    /// (boîte de points 32 × 16 + marges 12 × 5) plutôt que déduite de la
    /// mise en page, qui donnait une capsule trop large.
    private static let baseSize = NSSize(width: 32 + 24, height: 16 + 10)

    private var pillSize: NSSize {
        let scale = AppState.shared.pillScale
        return NSSize(
            width: (Self.baseSize.width * scale).rounded(),
            height: (Self.baseSize.height * scale).rounded())
    }

    private func resize() {
        guard let panel, let glassView else { return }
        let size = pillSize

        // Toujours réappliquer le rayon : il était posé après un `guard` qui
        // sortait quand la taille ne changeait pas, si bien que la pill
        // restait un rectangle à coins arrondis au lieu d'une capsule.
        glassView.cornerRadius = size.height / 2

        guard size != panel.frame.size else { return }
        let center = panel.isVisible
            ? NSPoint(x: panel.frame.midX, y: panel.frame.midY)
            : storedCenter()
        panel.setFrame(
            NSRect(
                x: (center.x - size.width / 2).rounded(),
                y: (center.y - size.height / 2).rounded(),
                width: size.width,
                height: size.height),
            display: true)
    }

    private func storedCenter() -> NSPoint {
        let frame = (NSScreen.main ?? NSScreen.screens[0]).visibleFrame
        let position = AppState.shared.pillPosition
        if position != .free {
            return position.center(in: frame, size: pillSize)
        }
        if let saved = UserDefaults.standard.string(forKey: Self.centerKey) {
            let point = NSPointFromString(saved)
            if NSScreen.screens.contains(where: { $0.frame.contains(point) }) {
                return point
            }
        }
        return PillPosition.bottomCenter.center(in: frame, size: pillSize)
    }

    /// Appelé à la fin d'un glisser-déposer réel, jamais lors d'un
    /// repositionnement programmé — nos propres `setFrame` déclenchaient
    /// `didMove` et faisaient basculer la position en « libre ».
    func userDidDrag() {
        guard let panel else { return }
        AppState.shared.pillPositionID = PillPosition.free.rawValue
        let center = NSPoint(x: panel.frame.midX, y: panel.frame.midY)
        UserDefaults.standard.set(NSStringFromPoint(center), forKey: Self.centerKey)
    }
}

/// Panneau qui ne prend jamais le focus : on dicte dans l'app d'en dessous.
final class PillPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// Glisser n'importe où sur la pill la déplace.
final class DraggableGlassView: NSGlassEffectView {
    override func mouseDown(with event: NSEvent) {
        // `performDrag` est bloquant : au retour le déplacement est terminé.
        window?.performDrag(with: event)
        Task { @MainActor in PillController.shared.userDidDrag() }
    }
}

/// Contenu de la pill — port fidèle de `src/components/Pill/AudioDots.tsx`
/// et du conteneur de `PillWindow.tsx`. Le bouton réglages de l'original
/// n'est pas repris : il n'a pas sa place ici.
struct PillView: View {
    @ObservedObject var state: AppState

    private var processing: Bool {
        state.phase == .transcribing || state.phase == .polishing
    }

    var body: some View {
        AudioDots(
            phase: state.phase,
            level: state.audioLevels.last ?? 0)
            // Marges de l'original : 0.75rem × 0.3125rem.
            .padding(.horizontal, 12)
            .padding(.vertical, 5)
            .scaleEffect(state.pillScale)
            .overlay {
                if processing {
                    BorderBeam()
                }
            }
            .fixedSize()
            // Les clics servent à déplacer la pill : laisser passer vers le verre.
            .allowsHitTesting(false)
    }
}

/// Trois points de 10 × 5 px.
///
/// Au repos ils se joignent en une barre unique (coins extérieurs arrondis,
/// coins intérieurs carrés). À l'enregistrement chacun se contracte en cercle
/// (scaleX 0,5) autour de son propre ancrage, ce qui fait apparaître les
/// écarts. Pendant le traitement la géométrie reste celle du repos et c'est
/// la couleur qui pulse.
struct AudioDots: View {
    let phase: AppState.Phase
    let level: Float

    @State private var pulse = false

    private let dotHeight: CGFloat = 5
    private let idleWidth: CGFloat = 10
    private let activeWidth: CGFloat = 5
    private var radius: CGFloat { dotHeight / 2 }

    /// Seuil d'activité vocale, équivalent à AUDIO_ACTIVITY_THRESHOLD (25/100).
    private var hasAudio: Bool { level > 0.25 }

    private var isRecording: Bool { phase == .recording }
    private var isProcessing: Bool { phase == .transcribing || phase == .polishing }

    private static let idle = Color(nsColor: NSColor(white: 1, alpha: 0.7))
    private static let active = Color(nsColor: NSColor(hex: 0x03E334))
    private static let sttPulse = (
        Color(nsColor: NSColor(hex: 0x00AAFF)), Color(nsColor: NSColor(hex: 0xCDDBFF)))
    private static let polishPulse = (
        Color(nsColor: NSColor(hex: 0x4894FF)), Color(nsColor: NSColor(hex: 0xD6E7FF)))

    private var color: Color {
        switch phase {
        case .transcribing: pulse ? Self.sttPulse.1 : Self.sttPulse.0
        case .polishing: pulse ? Self.polishPulse.1 : Self.polishPulse.0
        case .recording: hasAudio ? Self.active : Self.idle
        default: Self.idle
        }
    }

    /// L'ensemble occupe toujours 30 pt : les points se séparent sans que le
    /// groupe se décale.
    private var spacing: CGFloat {
        isRecording ? (idleWidth * 3 - activeWidth * 3) / 2 : 0
    }

    var body: some View {
        HStack(spacing: spacing) {
            ForEach(0..<3, id: \.self) { index in
                shape(index)
                    .fill(color)
                    .frame(width: isRecording ? activeWidth : idleWidth, height: dotHeight)
            }
        }
        .frame(width: idleWidth * 3, height: dotHeight)
        .frame(width: 32, height: 16)
        .animation(.easeOut(duration: 0.18), value: isRecording)
        .animation(.easeOut(duration: 0.28), value: hasAudio)
        .onChange(of: isProcessing) { _, processing in
            pulse = false
            guard processing else { return }
            withAnimation(.easeInOut(duration: 1.4).repeatForever(autoreverses: true)) {
                pulse = true
            }
        }
    }

    /// Au repos : arrondi seulement sur les bords extérieurs, pour que les
    /// trois points forment une barre continue.
    private func shape(_ index: Int) -> UnevenRoundedRectangle {
        guard !isRecording else {
            return UnevenRoundedRectangle(
                topLeadingRadius: radius, bottomLeadingRadius: radius,
                bottomTrailingRadius: radius, topTrailingRadius: radius)
        }
        switch index {
        case 0:
            return UnevenRoundedRectangle(
                topLeadingRadius: radius, bottomLeadingRadius: radius,
                bottomTrailingRadius: 0, topTrailingRadius: 0)
        case 2:
            return UnevenRoundedRectangle(
                topLeadingRadius: 0, bottomLeadingRadius: 0,
                bottomTrailingRadius: radius, topTrailingRadius: radius)
        default:
            return UnevenRoundedRectangle(
                topLeadingRadius: 0, bottomLeadingRadius: 0,
                bottomTrailingRadius: 0, topTrailingRadius: 0)
        }
    }
}

/// Halo tournant pendant le traitement — équivalent du BorderBeam « ocean »
/// de l'original (durée 1 s).
struct BorderBeam: View {
    @State private var angle: Double = 0

    var body: some View {
        Capsule()
            .strokeBorder(
                AngularGradient(
                    gradient: Gradient(colors: [
                        .clear, .clear, .clear,
                        Color(nsColor: NSColor(hex: 0x00AAFF)).opacity(0.9),
                        Color(nsColor: NSColor(hex: 0xCDDBFF)),
                        .clear,
                    ]),
                    center: .center,
                    angle: .degrees(angle)),
                lineWidth: 1.5)
            .onAppear {
                withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) {
                    angle = 360
                }
            }
    }
}
