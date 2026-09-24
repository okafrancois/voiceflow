import AppKit
import SwiftUI

/// Shortcut recording field: click, then press the desired
/// combination. Escape cancels.
struct ShortcutRecorder: View {
    @ObservedObject var state: AppState
    let keyPath: ReferenceWritableKeyPath<AppState, Shortcut?>
    /// The shortcut can be removed (command mode).
    var clearable = false
    @State private var recording = false
    @State private var monitors: [Any] = []

    private var currentDisplay: String {
        state[keyPath: keyPath]?.display ?? L.t("Aucun")
    }

    var body: some View {
        HStack(spacing: 8) {
            Button {
                recording ? stop() : start()
            } label: {
                Text(recording ? L.t("Pressez une combinaison…") : currentDisplay)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(recording ? VF.labelMuted : VF.label)
                    .frame(minWidth: 130)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(
                        Capsule().strokeBorder(recording ? VF.purple : VF.border, lineWidth: 1))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L.t("Raccourci actuel") + " : " + currentDisplay)
            .accessibilityHint(L.t("Activer, puis presser la combinaison souhaitée"))

            if clearable, state[keyPath: keyPath] != nil, !recording {
                Button {
                    state[keyPath: keyPath] = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 13))
                        .foregroundStyle(VF.labelFaint)
                }
                .buttonStyle(.plain)
                .help(L.t("Retirer le raccourci"))
            }
        }
        .onDisappear(perform: stop)
    }

    private func start() {
        recording = true
        // Safety net: a forgotten capture would swallow a keystroke elsewhere.
        DispatchQueue.main.asyncAfter(deadline: .now() + 15) {
            if recording { stop() }
        }
        // The global tap sees everything, including combinations the app
        // normally intercepts.
        state.captureShortcut(into: keyPath) { recording = false }
        guard !state.hotkeyTapActive else { return }

        // Without Accessibility permission, no tap: fall back to a local
        // listener. It must cover both keystrokes *and* modifiers, otherwise
        // a standalone key like Fn would never be captured.
        var pendingModifier: UInt16?
        let keys = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard event.keyCode != 53 else { stop(); return nil }
            guard Shortcut.isAssignable(
                keyCode: event.keyCode, modifiers: event.modifierFlags) else { return nil }
            let modifiers = event.modifierFlags
                .intersection([.control, .option, .shift, .command])
            state[keyPath: keyPath] = Shortcut(
                keyCode: event.keyCode, modifiers: modifiers.rawValue)
            stop()
            return nil
        }
        let modifiers = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { event in
            guard Shortcut.modifierKeyCodes.contains(event.keyCode) else { return event }
            if Shortcut.isPressed(event.keyCode, event.modifierFlags) {
                // Might be the start of a combination: wait for release.
                pendingModifier = event.keyCode
                return event
            }
            let relevant = event.modifierFlags
                .intersection([.control, .option, .shift, .command])
            guard pendingModifier == event.keyCode, relevant.isEmpty else { return event }
            state[keyPath: keyPath] = Shortcut(keyCode: event.keyCode, modifiers: 0)
            stop()
            return event
        }
        monitors = [keys, modifiers].compactMap { $0 }
    }

    private func stop() {
        recording = false
        state.cancelShortcutCapture()
        monitors.forEach(NSEvent.removeMonitor)
        monitors = []
    }
}
