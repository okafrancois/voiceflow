import AppKit
import Combine
import SwiftUI

/// Floating panel for the pill.
///
/// The glass comes from `NSGlassEffectView` (AppKit, macOS 26+), not the
/// SwiftUI modifier: in a borderless panel with a transparent background,
/// `glassEffect` can't pick up the window's backdrop and falls back to a
/// flat material. The AppKit view, on the other hand, composites at the
/// window server level — it's the real system material, which follows the
/// appearance and transparency settings.
@MainActor
final class PillController {
    static let shared = PillController()

    private var panel: PillPanel?
    private var glassView: NSGlassEffectView?
    private var hostingView: NSHostingView<PillView>?
    private var cancellables = Set<AnyCancellable>()

    private static let centerKey = "pillCenter"
    /// No dictation in progress.
    private var isHidden = true
    /// Content size, reported by the view: it grows with a message.
    private var contentSize: NSSize?
    private var noticeTask: Task<Void, Never>?

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

        // `$notice` emits before the assignment: read the value it carries.
        state.$notice
            .receive(on: RunLoop.main)
            .sink { [weak self] notice in self?.noticeChanged(notice) }
            .store(in: &cancellables)

        applyAppearance()
    }

    /// Applies the tint, opacity, size and position chosen in settings.
    func applyAppearance() {
        guard let panel, let glassView else { return }
        let state = AppState.shared
        glassView.tintColor = NSColor(hex: state.pillTint.hex)
            .withAlphaComponent(state.pillOpacity)
        resize()
        // Reposition right away if an anchor is chosen.
        if state.pillPosition != .free {
            place(panel, center: storedCenter())
        }
        refreshVisibility()
    }

    /// The pill can be always visible, visible during dictation, or never.
    private func refreshVisibility() {
        guard let panel else { return }
        switch AppState.shared.pillVisibility {
        case .never:
            panel.orderOut(nil)
        case .always:
            panel.alphaValue = 1
            panel.orderFrontRegardless()
        case .whileActive:
            if isHidden, AppState.shared.notice == nil { panel.orderOut(nil) }
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
            let wasHidden = isHidden
            isHidden = false
            if wasHidden {
                // Every dictation appears on the screen where you're working.
                if AppState.shared.pillPosition != .free {
                    place(panel, center: storedCenter())
                }
                resize()
                panel.alphaValue = 0
            }
            panel.orderFrontRegardless()
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.18
                panel.animator().alphaValue = 1
            }
        case .idle:
            isHidden = true
            hideIfIdle()
        }
    }

    /// Hides the pill if nothing justifies it anymore: no dictation, no
    /// message, no "always" mode.
    private func hideIfIdle() {
        guard let panel, isHidden, AppState.shared.notice == nil else { return }
        // In "always" mode, the pill stays on screen at rest.
        guard AppState.shared.pillVisibility != .always else {
            panel.alphaValue = 1
            panel.orderFrontRegardless()
            return
        }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.15
            panel.animator().alphaValue = 0
        } completionHandler: {
            // A dictation may have restarted during the fade: don't
            // hide a pill that has become useful again.
            Task { @MainActor in
                guard self.isHidden, AppState.shared.notice == nil else { return }
                panel.orderOut(nil)
            }
        }
    }

    /// A message shows for a few seconds, even at rest: this is often
    /// when an error arrives, with the main window closed.
    private func noticeChanged(_ notice: Notice?) {
        noticeTask?.cancel()
        guard let panel, AppState.shared.pillVisibility != .never else { return }
        guard let notice else {
            hideIfIdle()
            return
        }
        if isHidden {
            if AppState.shared.pillPosition != .free { place(panel, center: storedCenter()) }
            panel.orderFrontRegardless()
        }
        // Via the animator: a closing fade started just before (end of
        // dictation) would otherwise overwrite the value and leave the pill invisible.
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.12
            panel.animator().alphaValue = 1
        }
        let duration: Duration = notice.kind == .error ? .seconds(5) : .seconds(2)
        noticeTask = Task { @MainActor in
            try? await Task.sleep(for: duration)
            guard !Task.isCancelled, AppState.shared.notice?.id == notice.id else { return }
            AppState.shared.notice = nil
        }
    }

    /// Pill size: the original's, computed from the content (32 × 16 dot
    /// box + 12 × 5 margins) rather than derived from the layout, which
    /// gave a capsule that was too wide.
    private static let baseSize = NSSize(width: 32 + 24, height: 16 + 10)

    private var pillSize: NSSize {
        if let contentSize { return contentSize }
        let scale = AppState.shared.pillScale
        return NSSize(
            width: (Self.baseSize.width * scale).rounded(),
            height: (Self.baseSize.height * scale).rounded())
    }

    /// The view reports its ideal size; the panel follows.
    func contentSizeChanged(_ size: CGSize) {
        let rounded = NSSize(width: size.width.rounded(.up), height: size.height.rounded(.up))
        guard rounded.width > 0, rounded.height > 0, rounded != contentSize else { return }
        contentSize = rounded
        resize()
    }

    /// Adjusts the panel's size to the content, keeping the center fixed
    /// so the pill grows symmetrically — or stuck to its edge for a
    /// lateral anchor.
    private func resize() {
        guard let panel, let glassView else { return }
        let size = pillSize

        // Always reapply the radius: it used to be set after a `guard` that
        // returned early when the size didn't change, so the pill stayed a
        // rounded rectangle instead of a capsule.
        glassView.cornerRadius = size.height / 2

        guard size != panel.frame.size else { return }
        let anchored = AppState.shared.pillPosition != .free
        let center = anchored || !panel.isVisible
            ? storedCenter(in: panel.screen)
            : NSPoint(x: panel.frame.midX, y: panel.frame.midY)
        place(panel, center: center)
    }

    private func place(_ panel: NSPanel, center: NSPoint) {
        let size = pillSize
        panel.setFrame(
            NSRect(
                x: (center.x - size.width / 2).rounded(),
                y: (center.y - size.height / 2).rounded(),
                width: size.width, height: size.height),
            display: true)
    }

    /// The screen under the mouse, falling back to the main screen.
    private static var activeScreen: NSScreen {
        let mouse = NSEvent.mouseLocation
        return NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) }
            ?? NSScreen.main ?? NSScreen.screens[0]
    }

    private func storedCenter(in screen: NSScreen? = nil) -> NSPoint {
        let frame = (screen ?? Self.activeScreen).visibleFrame
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

    /// Called at the end of an actual drag, never during a programmatic
    /// repositioning — our own `setFrame` calls used to trigger `didMove`
    /// and flip the position to "free".
    func userDidDrag() {
        guard let panel else { return }
        AppState.shared.pillPositionID = PillPosition.free.rawValue
        let center = NSPoint(x: panel.frame.midX, y: panel.frame.midY)
        UserDefaults.standard.set(NSStringFromPoint(center), forKey: Self.centerKey)
    }
}

/// Panel that never takes focus: dictation happens in the app underneath.
final class PillPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// Dragging anywhere on the pill moves it.
final class DraggableGlassView: NSGlassEffectView {
    override func mouseDown(with event: NSEvent) {
        // `performDrag` is blocking: by the time it returns, the move is done.
        window?.performDrag(with: event)
        Task { @MainActor in PillController.shared.userDidDrag() }
    }
}

/// Pill content — a faithful port of `src/components/Pill/AudioDots.tsx`
/// and the container from `PillWindow.tsx`. The original's settings button
/// isn't carried over: it doesn't belong here.
struct PillView: View {
    @ObservedObject var state: AppState

    private var processing: Bool {
        state.phase == .transcribing || state.phase == .polishing
    }

    /// Text shown next to the dots: a message, otherwise the live
    /// transcript (Apple engine) if the option is active.
    private var message: (text: String, color: Color)? {
        if let notice = state.notice {
            return (notice.text, notice.kind == .error
                ? Color(nsColor: NSColor(hex: 0xFFB340)) : .white.opacity(0.9))
        }
        if state.showLivePreview, state.phase == .recording, !state.volatileTranscript.isEmpty {
            return (state.volatileTranscript, .white.opacity(0.85))
        }
        return nil
    }

    var body: some View {
        let scale = state.pillScale
        HStack(spacing: 8 * scale) {
            AudioDots(
                phase: state.phase,
                level: state.audioLevels.last ?? 0)
                .scaleEffect(scale)
                .frame(width: 32 * scale, height: 16 * scale)
            if let message {
                Text(message.text)
                    .font(.system(size: 12 * scale, weight: .medium))
                    .foregroundStyle(message.color)
                    .lineLimit(1)
                    // The last words spoken stay visible.
                    .truncationMode(.head)
                    .frame(maxWidth: 340 * scale, alignment: .leading)
            }
        }
        // Original's margins: 0.75rem × 0.3125rem.
        .padding(.horizontal, 12 * scale)
        .padding(.vertical, 5 * scale)
        .overlay {
            if processing {
                BorderBeam()
            }
        }
        .fixedSize()
        .background(GeometryReader { proxy in
            Color.clear.preference(key: PillSizeKey.self, value: proxy.size)
        })
        .onPreferenceChange(PillSizeKey.self) { size in
            PillController.shared.contentSizeChanged(size)
        }
        // Clicks are used to move the pill: let them pass through to the glass.
        .allowsHitTesting(false)
    }
}

private struct PillSizeKey: PreferenceKey {
    static let defaultValue: CGSize = .zero
    static func reduce(value: inout CGSize, nextValue: () -> CGSize) {
        value = nextValue()
    }
}

/// Three 10 × 5 px dots.
///
/// At rest they join into a single bar (rounded outer corners, square inner
/// corners). While recording each one contracts into a circle (scaleX 0.5)
/// around its own anchor, which reveals the gaps. During processing the
/// geometry stays the resting one and it's the color that pulses.
struct AudioDots: View {
    let phase: AppState.Phase
    let level: Float

    @State private var pulse = false

    private let dotHeight: CGFloat = 5
    private let idleWidth: CGFloat = 10
    private let activeWidth: CGFloat = 5
    private var radius: CGFloat { dotHeight / 2 }

    /// Voice activity threshold, equivalent to AUDIO_ACTIVITY_THRESHOLD (25/100).
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

    /// The group always occupies 30 pt: the dots separate without the
    /// group shifting.
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

    /// At rest: rounded only on the outer edges, so the three dots form
    /// a continuous bar.
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

/// Rotating halo during processing — equivalent of the original's "ocean"
/// BorderBeam (1 s duration).
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
