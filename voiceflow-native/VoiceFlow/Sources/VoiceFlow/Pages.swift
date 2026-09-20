import SwiftUI

// MARK: - Historique

struct HistoryPage: View {
    @ObservedObject var state: AppState
    @State private var search = ""
    @State private var filter: Filter = .all

    enum Filter: String, CaseIterable, Identifiable {
        case all, apple, whisper
        var id: String { rawValue }
        var title: String {
            switch self {
            case .all: L.t("Tout")
            case .apple: L.t("Apple")
            case .whisper: L.t("Whisper")
            }
        }
    }

    private var filtered: [HistoryEntry] {
        state.entries.filter { entry in
            let engineMatches = switch filter {
            case .all: true
            case .apple: entry.sttEngine == "apple"
            case .whisper: entry.sttEngine == "whisper"
            }
            return engineMatches
                && (search.isEmpty || entry.finalText.localizedCaseInsensitiveContains(search))
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack(spacing: 10) {
                VFSearchField(text: $search)
                    .frame(maxWidth: 320)
                ForEach(Filter.allCases) { item in
                    Button(item.title) { filter = item }
                        .buttonStyle(VFButtonStyle(prominent: filter == item))
                }
                Spacer()
                if !state.entries.isEmpty {
                    Button(L.t("Tout effacer")) { state.clearHistory() }
                        .buttonStyle(VFButtonStyle())
                }
            }

            if filtered.isEmpty {
                VFCard {
                    Text(state.entries.isEmpty
                        ? "Votre prochaine transcription apparaîtra ici."
                        : "Aucune transcription ne correspond.")
                        .font(.system(size: 14))
                        .foregroundStyle(VF.labelMuted)
                        .padding(.vertical, 20)
                }
            } else {
                ForEach(groupedByDay(), id: \.day) { group in
                    VStack(alignment: .leading, spacing: 10) {
                        Text(dayLabel(group.day))
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(VF.labelMuted)
                        VFCard(padding: 0) {
                            VStack(spacing: 0) {
                                ForEach(Array(group.entries.enumerated()), id: \.element.id) { index, entry in
                                    if index > 0 {
                                        Rectangle().fill(VF.divider).frame(height: 1)
                                    }
                                    EntryRow(entry: entry, state: state)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    private func groupedByDay() -> [(day: Date, entries: [HistoryEntry])] {
        Dictionary(grouping: filtered) { Calendar.current.startOfDay(for: $0.createdAt) }
            .map { (day: $0.key, entries: $0.value) }
            .sorted { $0.day > $1.day }
    }

    private func dayLabel(_ day: Date) -> String {
        let calendar = Calendar.current
        if calendar.isDateInToday(day) { return "Aujourd'hui" }
        if calendar.isDateInYesterday(day) { return "Hier" }
        return day.formatted(.dateTime.weekday(.wide).day().month(.wide))
    }
}

/// Champ de recherche arrondi, comme celui de l'app actuelle.
struct VFSearchField: View {
    @Binding var text: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 12))
                .foregroundStyle(VF.labelFaint)
            TextField(L.t("Rechercher"), text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(VF.label)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(Capsule().strokeBorder(VF.border, lineWidth: 1))
    }
}

// MARK: - Dictionnaire

struct DictionaryPage: View {
    @ObservedObject private var store = VocabularyStore.shared
    @State private var search = ""
    @State private var heard = ""
    @State private var replacement = ""
    @State private var tab: Tab = .manual
    @State private var importing = false
    @State private var importReport: String?

    enum Tab: String, CaseIterable, Identifiable {
        case manual, learned
        var id: String { rawValue }
        var title: String {
            switch self {
            case .manual: L.t("Personnalisé")
            case .learned: L.t("Automatique")
            }
        }
    }

    private var filtered: [DictionaryEntry] {
        store.entries
            .filter { $0.learned == (tab == .learned) }
            .filter {
                search.isEmpty
                    || $0.replacement.localizedCaseInsensitiveContains(search)
                    || $0.allForms.contains { $0.localizedCaseInsensitiveContains(search) }
            }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack(spacing: 8) {
                ForEach(Tab.allCases) { item in
                    Button(item.title) { tab = item }
                        .buttonStyle(VFButtonStyle(prominent: tab == item))
                }
                Spacer()
                if !store.entries.isEmpty {
                    VFSearchField(text: $search).frame(maxWidth: 240)
                }
            }

            if tab == .manual {
                VFCard {
                    VStack(alignment: .leading, spacing: 14) {
                        HStack {
                            Text(L.t("Ajouter un terme"))
                                .font(.system(size: 15, weight: .medium))
                                .foregroundStyle(VF.label)
                            Spacer()
                            Button(L.t("Importer un CSV…")) { importing = true }
                                .buttonStyle(VFButtonStyle())
                        }
                        HStack(spacing: 10) {
                            VFTextField(placeholder: "Entendu", text: $heard)
                            Image(systemName: "arrow.right")
                                .font(.system(size: 12))
                                .foregroundStyle(VF.labelFaint)
                            VFTextField(placeholder: "Écrire", text: $replacement)
                            Button("Ajouter", action: add)
                                .buttonStyle(VFButtonStyle(prominent: true))
                                .disabled(heard.isEmpty || replacement.isEmpty)
                        }
                        if let importReport {
                            Text(importReport)
                                .font(.system(size: 12))
                                .foregroundStyle(VF.labelMuted)
                        }
                    }
                }
            } else {
                VFCard {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(L.t("Apprendre mes corrections"))
                                .font(.system(size: 14))
                                .foregroundStyle(VF.label)
                            Text(L.t("Après une insertion, VoiceFlow relit le champ et retient les mots que vous avez remplacés."))
                                .font(.system(size: 12))
                                .foregroundStyle(VF.labelMuted)
                        }
                        Spacer()
                        Toggle("", isOn: $store.learnCorrections)
                            .labelsHidden().toggleStyle(.switch)
                    }
                }
            }

            VFCard(padding: 0) {
                if filtered.isEmpty {
                    Text(L.t(emptyMessage))
                        .font(.system(size: 14))
                        .foregroundStyle(VF.labelMuted)
                        .padding(20)
                } else {
                    VStack(spacing: 0) {
                        ForEach(Array(filtered.enumerated()), id: \.element.id) { index, entry in
                            if index > 0 { Rectangle().fill(VF.divider).frame(height: 1) }
                            entryRow(entry)
                        }
                    }
                }
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.commaSeparatedText, .text]) { result in
            guard let url = try? result.get() else { return }
            let accessed = url.startAccessingSecurityScopedResource()
            defer { if accessed { url.stopAccessingSecurityScopedResource() } }
            do {
                let added = try store.importCSV(from: url)
                importReport = added == 0
                    ? "Aucune nouvelle entrée : le fichier doit contenir « entendu,correction » par ligne."
                    : "\(added) terme\(added > 1 ? "s" : "") importé\(added > 1 ? "s" : "")."
            } catch {
                importReport = "Import impossible : \(error.localizedDescription)"
            }
        }
    }

    private var emptyMessage: String {
        if !search.isEmpty { return "Aucun terme ne correspond." }
        return tab == .manual
            ? "Aucun terme. Ajoutez les noms propres et le jargon que la dictée écrit mal."
            : "Rien d'appris pour l'instant. Corrigez un mot après une dictée et il apparaîtra ici."
    }

    @ViewBuilder
    private func entryRow(_ entry: DictionaryEntry) -> some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 8) {
                    Text(entry.heard)
                        .font(.system(size: 14))
                        .foregroundStyle(VF.labelMuted)
                    Image(systemName: "arrow.right")
                        .font(.system(size: 11))
                        .foregroundStyle(VF.labelFaint)
                    Text(entry.replacement)
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(VF.label)
                }
                if !entry.variants.isEmpty {
                    Text("aussi entendu : " + entry.variants.joined(separator: ", "))
                        .font(.system(size: 12))
                        .foregroundStyle(VF.labelFaint)
                }
            }
            Spacer()
            if entry.useCount > 0 {
                Text("\(entry.useCount)×")
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelFaint)
                    .monospacedDigit()
            }
            if let lastUsed = entry.lastUsed {
                Text(lastUsed.formatted(.relative(presentation: .numeric)))
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelFaint)
            }
            Button {
                store.entries.removeAll { $0.id == entry.id }
            } label: {
                Image(systemName: "trash")
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelMuted)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
    }

    private func add() {
        let key = heard.trimmingCharacters(in: .whitespaces)
        let value = replacement.trimmingCharacters(in: .whitespaces)
        guard !key.isEmpty, !value.isEmpty else { return }
        store.entries.append(DictionaryEntry(heard: key, replacement: value))
        heard = ""
        replacement = ""
    }
}

/// Champ de saisie arrondi, assorti au reste.
struct VFTextField: View {
    let placeholder: String
    @Binding var text: String

    var body: some View {
        TextField(L.t(placeholder), text: $text)
            .textFieldStyle(.plain)
            .font(.system(size: 13))
            .foregroundStyle(VF.label)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(VF.border, lineWidth: 1))
    }
}

// MARK: - Extraits

struct SnippetsPage: View {
    @ObservedObject private var store = VocabularyStore.shared
    @State private var trigger = ""
    @State private var expansion = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            VFCard {
                VStack(alignment: .leading, spacing: 14) {
                    Text(L.t("Nouvel extrait"))
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(VF.label)
                    VFTextField(placeholder: "Quand je dis…", text: $trigger)
                    ZStack(alignment: .topLeading) {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .strokeBorder(VF.border, lineWidth: 1)
                        if expansion.isEmpty {
                            Text(L.t("Texte à insérer"))
                                .font(.system(size: 13))
                                .foregroundStyle(VF.labelFaint)
                                .padding(.horizontal, 18)
                                .padding(.vertical, 14)
                        }
                        TextEditor(text: $expansion)
                            .font(.system(size: 13))
                            .scrollContentBackground(.hidden)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 10)
                    }
                    .frame(height: 90)
                    HStack {
                        Spacer()
                        Button("Ajouter", action: add)
                            .buttonStyle(VFButtonStyle(prominent: true))
                            .disabled(trigger.isEmpty || expansion.isEmpty)
                    }
                }
            }

            VFCard(padding: 0) {
                if store.snippets.isEmpty {
                    Text(L.t("Aucun extrait pour l'instant."))
                        .font(.system(size: 14))
                        .foregroundStyle(VF.labelMuted)
                        .padding(20)
                } else {
                    VStack(spacing: 0) {
                        ForEach(Array(store.snippets.enumerated()), id: \.element.id) { index, snippet in
                            if index > 0 { Rectangle().fill(VF.divider).frame(height: 1) }
                            HStack(alignment: .top, spacing: 12) {
                                VStack(alignment: .leading, spacing: 5) {
                                    Text(snippet.trigger)
                                        .font(.system(size: 14, weight: .medium))
                                        .foregroundStyle(VF.label)
                                    Text(snippet.expansion)
                                        .font(.system(size: 13))
                                        .foregroundStyle(VF.labelMuted)
                                        .lineLimit(3)
                                }
                                Spacer()
                                Button {
                                    store.snippets.removeAll { $0.id == snippet.id }
                                } label: {
                                    Image(systemName: "trash")
                                        .font(.system(size: 12))
                                        .foregroundStyle(VF.labelMuted)
                                }
                                .buttonStyle(.plain)
                            }
                            .padding(20)
                        }
                    }
                }
            }
        }
    }

    private func add() {
        let key = trigger.trimmingCharacters(in: .whitespaces)
        let value = expansion.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty, !value.isEmpty else { return }
        store.snippets.append(Snippet(trigger: key, expansion: value))
        trigger = ""
        expansion = ""
    }
}

// MARK: - Styles

struct StylesPage: View {
    @ObservedObject var state: AppState
    @ObservedObject private var store = VocabularyStore.shared
    @State private var pickingApp = false
    @State private var expanded: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            VFCard(padding: 0) {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(L.t("Styles de polissage"))
                            .font(.system(size: 15, weight: .medium))
                            .foregroundStyle(VF.label)
                        Text(L.t("Le style coché s'applique par défaut. Dépliez-en un pour lire et modifier son prompt système."))
                            .font(.system(size: 13))
                            .foregroundStyle(VF.labelMuted)
                    }
                    .padding(20)

                    ForEach(PolishCatalog.all) { template in
                        Rectangle().fill(VF.divider).frame(height: 1)
                        styleRow(template)
                    }
                }
            }

            VFCard(padding: 0) {
                VStack(alignment: .leading, spacing: 0) {
                    HStack {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(L.t("Règles par application"))
                                .font(.system(size: 15, weight: .medium))
                                .foregroundStyle(VF.label)
                            Text(L.t("Le style choisi ici remplace le style par défaut dans cette app."))
                                .font(.system(size: 13))
                                .foregroundStyle(VF.labelMuted)
                        }
                        Spacer()
                        Button(L.t("Ajouter…")) { pickingApp = true }
                            .buttonStyle(VFButtonStyle())
                    }
                    .padding(20)

                    if store.appRules.isEmpty {
                        Text(L.t("Aucune règle : le style par défaut s'applique partout."))
                            .font(.system(size: 14))
                            .foregroundStyle(VF.labelMuted)
                            .padding(.horizontal, 20)
                            .padding(.bottom, 20)
                    } else {
                        ForEach(store.appRules) { rule in
                            Rectangle().fill(VF.divider).frame(height: 1)
                            HStack(spacing: 12) {
                                Text(rule.appName)
                                    .font(.system(size: 14))
                                    .foregroundStyle(VF.label)
                                Spacer()
                                Picker("", selection: Binding(
                                    get: { rule.templateID },
                                    set: { newValue in
                                        if let position = store.appRules.firstIndex(where: { $0.id == rule.id }) {
                                            store.appRules[position].templateID = newValue
                                        }
                                    })) {
                                    ForEach(PolishCatalog.all) { Text($0.name).tag($0.id) }
                                }
                                .labelsHidden()
                                .fixedSize()
                                Button {
                                    store.appRules.removeAll { $0.id == rule.id }
                                } label: {
                                    Image(systemName: "trash")
                                        .font(.system(size: 12))
                                        .foregroundStyle(VF.labelMuted)
                                }
                                .buttonStyle(.plain)
                            }
                            .padding(.horizontal, 20)
                            .padding(.vertical, 13)
                        }
                    }
                }
            }
        }
        .fileImporter(isPresented: $pickingApp, allowedContentTypes: [.application]) { result in
            guard let url = try? result.get(),
                  let bundle = Bundle(url: url),
                  let bundleID = bundle.bundleIdentifier,
                  !store.appRules.contains(where: { $0.bundleID == bundleID })
            else { return }
            let name = FileManager.default.displayName(atPath: url.path)
                .replacingOccurrences(of: ".app", with: "")
            store.appRules.append(AppRule(
                bundleID: bundleID, appName: name, templateID: state.polishTemplateID))
        }
    }

    @ViewBuilder
    private func styleRow(_ template: PolishTemplate) -> some View {
        let isOpen = expanded == template.id
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Button {
                    state.polishTemplateID = template.id
                } label: {
                    Image(systemName: state.polishTemplateID == template.id
                        ? "checkmark.circle.fill" : "circle")
                        .font(.system(size: 15))
                        .foregroundStyle(state.polishTemplateID == template.id
                            ? VF.green : VF.labelFaint)
                }
                .buttonStyle(.plain)
                .help("Choisir comme style par défaut")

                Text(L.t(template.name))
                    .font(.system(size: 14))
                    .foregroundStyle(VF.label)

                // Le badge dit d'un coup d'œil si le prompt est celui d'origine.
                Text(template.isCustomized ? "modifié" : "système")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(template.isCustomized ? VF.amber : VF.labelFaint)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(
                        Capsule().fill((template.isCustomized ? VF.amber : VF.labelFaint)
                            .opacity(0.14)))

                Spacer()

                Button {
                    withAnimation(.easeOut(duration: 0.15)) {
                        expanded = isOpen ? nil : template.id
                    }
                } label: {
                    HStack(spacing: 5) {
                        Text(isOpen ? "Masquer le prompt" : "Voir le prompt")
                            .font(.system(size: 12))
                        Image(systemName: isOpen ? "chevron.up" : "chevron.down")
                            .font(.system(size: 10))
                    }
                    .foregroundStyle(VF.labelMuted)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 13)

            if isOpen {
                PromptEditor(templateID: template.id)
                    .padding(.horizontal, 20)
                    .padding(.bottom, 18)
            }
        }
    }
}

/// Éditeur du prompt système d'un style : texte intégral, modifiable,
/// avec retour à la valeur d'origine.
struct PromptEditor: View {
    let templateID: String
    @ObservedObject private var store = VocabularyStore.shared
    @State private var draft = ""
    @State private var loaded = false

    private var defaultPrompt: String { PolishCatalog.defaultPrompt(templateID) }
    private var isCustomized: Bool { store.customPrompts[templateID] != nil }
    private var isDirty: Bool { draft != (store.customPrompts[templateID] ?? defaultPrompt) }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            TextEditor(text: $draft)
                .font(.system(size: 12, design: .monospaced))
                .scrollContentBackground(.hidden)
                .padding(10)
                .frame(height: 220)
                .background(VF.background, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .strokeBorder(VF.border, lineWidth: 1))

            HStack(spacing: 8) {
                Text(isCustomized
                    ? "Prompt modifié — il remplace celui livré avec l'app."
                    : "Prompt d'origine, livré avec l'app.")
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelMuted)

                Spacer()

                if isCustomized {
                    Button(L.t("Rétablir l'original")) {
                        store.customPrompts[templateID] = nil
                        draft = defaultPrompt
                    }
                    .buttonStyle(VFButtonStyle())
                }

                Button(L.t("Enregistrer")) {
                    let trimmed = draft.trimmingCharacters(in: .whitespacesAndNewlines)
                    if trimmed == defaultPrompt.trimmingCharacters(in: .whitespacesAndNewlines) {
                        store.customPrompts[templateID] = nil
                    } else {
                        store.customPrompts[templateID] = draft
                    }
                }
                .buttonStyle(VFButtonStyle(prominent: true))
                .disabled(!isDirty)
            }
        }
        .onAppear {
            guard !loaded else { return }
            draft = store.customPrompts[templateID] ?? defaultPrompt
            loaded = true
        }
    }
}

// MARK: - Réglages

struct SettingsPage: View {
    @ObservedObject var state: AppState
    @ObservedObject private var models = WhisperModelStore.shared
    @ObservedObject private var sherpaModels = SherpaModelStore.shared
    @ObservedObject private var updates = UpdateChecker.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            settingsCard("Dictée") {
                row("Langue", help: languageHelp) {
                    Picker("", selection: $state.dictationLocaleID) {
                        // Toujours proposée : la masquer selon le moteur
                        // laissait croire à une option manquante.
                        Text(state.displayName(for: AppState.autoLocaleID))
                            .tag(AppState.autoLocaleID)
                        Divider()
                        ForEach(state.availableLocaleIDs, id: \.self) { id in
                            Text(state.displayName(for: id)).tag(id)
                        }
                    }
                    .labelsHidden().fixedSize()
                }
                if state.autoLanguageUnavailable {
                    divider
                    HStack(spacing: 10) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.system(size: 13))
                            .foregroundStyle(VF.amber)
                        Text(L.t("Le moteur d'Apple ne détecte pas la langue. Choisissez un modèle Whisper ci-dessous, ou une langue précise."))
                            .font(.system(size: 12))
                            .foregroundStyle(VF.labelMuted)
                        Spacer()
                        Button(L.t("Utiliser Whisper Small")) {
                            state.engineChoiceID = EngineChoice.whisperSmall.rawValue
                        }
                        .buttonStyle(VFButtonStyle())
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 13)
                }
            }

            VFCard(padding: 0) {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(L.t("Moteur de transcription"))
                            .font(.system(size: 15, weight: .medium))
                            .foregroundStyle(VF.label)
                        Text(L.t("Les modèles Whisper se téléchargent au premier usage, puis restent sur l'appareil."))
                            .font(.system(size: 13))
                            .foregroundStyle(VF.labelMuted)
                    }
                    .padding(20)

                    Rectangle().fill(VF.divider).frame(height: 1)
                    engineRow(.apple)
                    ForEach(EngineChoice.downloadableChoices) { choice in
                        Rectangle().fill(VF.divider).frame(height: 1)
                        engineRow(choice)
                    }
                }
            }

            settingsCard("Raccourcis") {
                row("Raccourci de dictée") {
                    ShortcutRecorder(state: state, keyPath: \.dictateShortcut)
                }
                divider
                row("Déclenchement", help: state.triggerMode.help) {
                    Picker("", selection: $state.triggerMode) {
                        ForEach(TriggerMode.allCases) { Text($0.title).tag($0) }
                    }
                    .labelsHidden().fixedSize()
                }
            }

            settingsCard("Polissage") {
                row("Polir chaque dictée",
                    help: "Le texte dicté passe par le modèle avant d'être inséré.") {
                    Toggle("", isOn: $state.polishEnabled).labelsHidden().toggleStyle(.switch)
                }
                divider
                row("Moteur",
                    help: "Modèle du système, sur l'appareil. À activer dans Réglages Système › Apple Intelligence.") {
                    Text(L.t("Apple Intelligence"))
                        .font(.system(size: 13))
                        .foregroundStyle(VF.labelMuted)
                }
                divider
                row("Style", help: "Les prompts se lisent et se modifient dans la section Polissage.") {
                    Picker("", selection: $state.polishTemplateID) {
                        ForEach(PolishCatalog.all) { Text($0.name).tag($0.id) }
                    }
                    .labelsHidden().fixedSize()
                }
            }

            settingsCard("Entrée audio") {
                row("Microphone") {
                    Picker("", selection: $state.inputDeviceID) {
                        Text(L.t("Périphérique du système")).tag(AudioDevices.systemDefaultID)
                        ForEach(AudioDevices.inputs()) { device in
                            Text(device.name).tag(device.id)
                        }
                    }
                    .labelsHidden().fixedSize()
                }
                divider
                row("Réduction du bruit",
                    help: "Traitement vocal du système : atténue le bruit ambiant et l'écho.") {
                    Toggle("", isOn: $state.noiseReduction).labelsHidden().toggleStyle(.switch)
                }
                divider
                row("Coupe du silence",
                    help: "N'envoie au moteur que ce qui porte de la voix : dictées plus courtes, transcription plus rapide.") {
                    Toggle("", isOn: $state.trimSilence).labelsHidden().toggleStyle(.switch)
                }
                if state.trimSilence {
                    divider
                    row("Sensibilité",
                        help: "Plus haut, plus sévère : convient à un environnement bruyant.") {
                        HStack(spacing: 10) {
                            Slider(value: $state.silenceMargin, in: 0.02...0.15)
                                .frame(width: 160)
                            Text(String(format: "%.2f", state.silenceMargin))
                                .font(.system(size: 12))
                                .foregroundStyle(VF.labelMuted)
                                .monospacedDigit()
                        }
                    }
                }
            }

            settingsCard("Apparence") {
                row("Langue de l'interface") {
                    Picker("", selection: $state.interfaceLanguage) {
                        Text(L.t("Système")).tag("")
                        Text(L.t("Français")).tag("fr")
                        Text(L.t("English")).tag("en")
                    }
                    .labelsHidden().fixedSize()
                }
                divider
                row("Thème") {
                    Picker("", selection: $state.themeID) {
                        ForEach(AppTheme.allCases) { Text($0.title).tag($0.rawValue) }
                    }
                    .labelsHidden().fixedSize()
                }
                divider
                row("Affichage de la pill") {
                    Picker("", selection: $state.pillVisibilityID) {
                        ForEach(PillVisibility.allCases) { Text($0.title).tag($0.rawValue) }
                    }
                    .labelsHidden().fixedSize()
                }
                divider
                row("Position", help: "Déplacer la pill à la main bascule sur « libre ».") {
                    Picker("", selection: $state.pillPositionID) {
                        ForEach(PillPosition.allCases) { Text($0.title).tag($0.rawValue) }
                    }
                    .labelsHidden().fixedSize()
                }
                divider
                row("Couleur") {
                    Picker("", selection: $state.pillTintID) {
                        ForEach(PillTint.allCases) { tint in
                            HStack {
                                Circle().fill(tint.color).frame(width: 10, height: 10)
                                Text(tint.title)
                            }
                            .tag(tint.rawValue)
                        }
                    }
                    .labelsHidden().fixedSize()
                }
                divider
                row("Taille") {
                    HStack(spacing: 10) {
                        Slider(value: $state.pillScale, in: 0.875...1.375, step: 0.125)
                            .frame(width: 160)
                        Text("×\(String(format: "%.3f", state.pillScale))")
                            .font(.system(size: 12))
                            .foregroundStyle(VF.labelMuted)
                            .monospacedDigit()
                    }
                }
                divider
                row("Opacité") {
                    HStack(spacing: 10) {
                        Slider(value: $state.pillOpacity, in: 0.2...1)
                            .frame(width: 160)
                        Text("\(Int(state.pillOpacity * 100)) %")
                            .font(.system(size: 12))
                            .foregroundStyle(VF.labelMuted)
                            .monospacedDigit()
                    }
                }
            }

            settingsCard("Insertion") {
                row("Insérer dans le champ d'origine",
                    help: "Mémorise le champ actif au déclenchement et y écrit sans réactiver l'application. Sinon, le texte part là où se trouve le curseur.") {
                    Toggle("", isOn: $state.insertInOriginalField).labelsHidden().toggleStyle(.switch)
                }
            }

            settingsCard("Comportement") {
                row("Ouvrir à la connexion") {
                    Toggle("", isOn: $state.launchAtLogin).labelsHidden().toggleStyle(.switch)
                }
                divider
                row("Sons de confirmation") {
                    Toggle("", isOn: $state.soundsEnabled).labelsHidden().toggleStyle(.switch)
                }
                divider
                row("Conserver l'historique") {
                    Picker("", selection: Binding(
                        get: { state.retentionDays ?? 0 },
                        set: { state.retentionDays = $0 == 0 ? nil : $0 })) {
                        Text(L.t("Sans limite")).tag(0)
                        Text(L.t("30 jours")).tag(30)
                        Text(L.t("90 jours")).tag(90)
                        Text(L.t("1 an")).tag(365)
                    }
                    .labelsHidden().fixedSize()
                }
            }

            settingsCard("Autorisations") {
                permissionRow("Microphone", granted: state.microphoneGranted, pane: "Privacy_Microphone")
                divider
                permissionRow("Accessibilité", granted: state.accessibilityGranted, pane: "Privacy_Accessibility")
            }

            settingsCard("Mises à jour") {
                row("Version installée") {
                    Text(updates.currentVersion)
                        .font(.system(size: 13))
                        .foregroundStyle(VF.labelMuted)
                        .monospacedDigit()
                }
                divider
                row("Vérifier automatiquement", help: "Une fois par jour au lancement.") {
                    Toggle("", isOn: $updates.automatic).labelsHidden().toggleStyle(.switch)
                }
                divider
                row("Adresse du flux",
                    help: "JSON contenant version, notes et lien de téléchargement.") {
                    VFTextField(placeholder: "https://…/appcast.json", text: $updates.feedURL)
                        .frame(width: 280)
                }
                divider
                row(updateStatusTitle, help: updates.latest?.notes) {
                    Button(updates.checking ? "Vérification…" : "Vérifier") {
                        Task { await updates.check() }
                    }
                    .buttonStyle(VFButtonStyle())
                    .disabled(updates.checking || updates.feedURL.isEmpty)
                }
                if updates.updateAvailable, let link = updates.latest?.url,
                   let url = URL(string: link) {
                    divider
                    row("Nouvelle version disponible") {
                        Button(L.t("Télécharger")) { NSWorkspace.shared.open(url) }
                            .buttonStyle(VFButtonStyle(prominent: true))
                    }
                }
                if let error = updates.error {
                    divider
                    row("Erreur") {
                        Text(L.t(error)).font(.system(size: 12)).foregroundStyle(VF.amber)
                    }
                }
            }

            settingsCard("Données") {
                row("Transcriptions enregistrées") {
                    Text(number(state.entries.count))
                        .font(.system(size: 13))
                        .foregroundStyle(VF.labelMuted)
                        .monospacedDigit()
                }
                divider
                row("Journal de diagnostic",
                    help: "Ce que l'app a fait à chaque dictée : audio capté, durée, erreurs.") {
                    Button("Ouvrir le journal") {
                        NSWorkspace.shared.open(Diagnostics.fileURL)
                    }
                    .buttonStyle(VFButtonStyle())
                }
                divider
                row("Dossier de l'app") {
                    Button(L.t("Révéler dans le Finder")) {
                        NSWorkspace.shared.activateFileViewerSelecting(
                            [URL.applicationSupportDirectory.appending(path: "VoiceFlow")])
                    }
                    .buttonStyle(VFButtonStyle())
                }
                divider
                row("Effacer l'historique") {
                    Button(L.t("Effacer…")) { state.clearHistory() }
                        .buttonStyle(VFButtonStyle())
                        .disabled(state.entries.isEmpty)
                }
            }

            if let error = state.lastError {
                VFCard {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(L.t("Dernière erreur"))
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(VF.amber)
                        Text(L.t(error)).font(.system(size: 13)).foregroundStyle(VF.labelMuted)
                    }
                }
            }
        }
        .onAppear { state.refreshPermissions() }
    }

    /// Ce qu'il y a à afficher à droite d'une ligne de moteur. Les deux
    /// familles téléchargeables ont chacune leur magasin ; la ligne, elle,
    /// n'a pas à savoir laquelle.
    private struct DownloadState {
        var needsModel = false
        var downloaded = false
        var fraction: Double?
        var start: () -> Void = {}
        var remove: () -> Void = {}
    }

    private func downloadState(_ choice: EngineChoice) -> DownloadState {
        if let variant = choice.whisperModel {
            return DownloadState(
                needsModel: true,
                downloaded: models.isDownloaded(variant),
                fraction: models.downloading.flatMap { $0.variant == variant ? $0.fraction : nil },
                start: { Task { try? await models.ensureAvailable(variant) } },
                remove: { models.forget(variant) })
        }
        if let model = choice.sherpaModel {
            return DownloadState(
                needsModel: true,
                downloaded: sherpaModels.isDownloaded(model),
                fraction: sherpaModels.downloading.flatMap {
                    $0.model == model.id ? $0.fraction : nil
                },
                start: { Task { try? await sherpaModels.ensureAvailable(model) } },
                remove: { sherpaModels.forget(model) })
        }
        return DownloadState()
    }

    private var anyDownloadRunning: Bool {
        models.downloading != nil || sherpaModels.downloading != nil
    }

    /// Une ligne par moteur : sélection, taille, état de téléchargement.
    @ViewBuilder
    private func engineRow(_ choice: EngineChoice) -> some View {
        let selected = state.engineChoiceID == choice.rawValue
        let download = downloadState(choice)

        HStack(spacing: 12) {
            Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                .font(.system(size: 15))
                .foregroundStyle(selected ? VF.green : VF.labelFaint)

            VStack(alignment: .leading, spacing: 3) {
                Text(L.t(choice.displayName))
                    .font(.system(size: 14))
                    .foregroundStyle(VF.label)
                Text(L.t(choice.detail))
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelMuted)
            }

            Spacer()

            if let fraction = download.fraction {
                HStack(spacing: 8) {
                    ProgressView(value: fraction).frame(width: 110)
                    Text("\(Int(fraction * 100)) %")
                        .font(.system(size: 12))
                        .foregroundStyle(VF.labelMuted)
                        .monospacedDigit()
                }
            } else if download.needsModel, download.downloaded {
                HStack(spacing: 10) {
                    Text(L.t("Téléchargé"))
                        .font(.system(size: 12))
                        .foregroundStyle(VF.green)
                    Button(action: download.remove) {
                        Image(systemName: "trash")
                            .font(.system(size: 12))
                            .foregroundStyle(VF.labelMuted)
                    }
                    .buttonStyle(.plain)
                    .help("Supprimer le modèle de l'appareil")
                }
            } else if download.needsModel {
                Button(L.t("Télécharger"), action: download.start)
                    .buttonStyle(VFButtonStyle())
                    .disabled(anyDownloadRunning)
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 13)
        .contentShape(Rectangle())
        .onTapGesture { state.engineChoiceID = choice.rawValue }
    }

    private var languageHelp: String {
        state.engineChoice.detectsLanguage
            ? "La détection automatique laisse le moteur reconnaître la langue parlée."
            : "Le moteur d'Apple transcrit dans la langue choisie ; il ne la détecte pas."
    }

    private var updateStatusTitle: String {
        if updates.updateAvailable, let version = updates.latest?.version {
            return "Version \(version) disponible"
        }
        if updates.latest != nil { return "Vous êtes à jour" }
        return "Rechercher une mise à jour"
    }

    private var divider: some View {
        Rectangle().fill(VF.divider).frame(height: 1)
    }

    private func settingsCard<Content: View>(
        _ title: String, @ViewBuilder content: () -> Content
    ) -> some View {
        VFCard(padding: 0) {
            VStack(alignment: .leading, spacing: 0) {
                Text(L.t(title))
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(VF.label)
                    .padding(20)
                content()
            }
        }
    }

    private func row<Control: View>(
        _ title: String, help: String? = nil, @ViewBuilder control: () -> Control
    ) -> some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 3) {
                Text(L.t(title)).font(.system(size: 14)).foregroundStyle(VF.label)
                if let help {
                    Text(L.t(help)).font(.system(size: 12)).foregroundStyle(VF.labelMuted)
                }
            }
            Spacer()
            control()
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 13)
    }

    private func permissionRow(_ title: String, granted: Bool, pane: String) -> some View {
        row(title) {
            if granted {
                HStack(spacing: 6) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 13))
                        .foregroundStyle(VF.green)
                    Text(L.t("Autorisé")).font(.system(size: 13)).foregroundStyle(VF.labelMuted)
                }
            } else {
                Button(L.t("Autoriser…")) {
                    if pane.contains("Microphone") {
                        Task { await state.requestMicrophone() }
                    } else {
                        state.requestAccessibility()
                    }
                }
                .buttonStyle(VFButtonStyle())
            }
        }
    }
}

// MARK: - À propos

struct AboutPage: View {
    var body: some View {
        VFCard {
            VStack(alignment: .leading, spacing: 16) {
                HStack(spacing: 14) {
                    VFLogo(size: 48)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(L.t("Voice Flow"))
                            .font(.system(size: 20, weight: .semibold, design: .serif))
                            .italic()
                            .foregroundStyle(VF.label)
                        Text(L.t("Version 0.1.0 — native, macOS 26+"))
                            .font(.system(size: 13))
                            .foregroundStyle(VF.labelMuted)
                    }
                }
                Rectangle().fill(VF.divider).frame(height: 1)
                Text(L.t("Transcription par SpeechAnalyzer ou Whisper, polissage par Apple Intelligence. Tout se passe sur cet appareil."))
                    .font(.system(size: 14))
                    .foregroundStyle(VF.labelMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
