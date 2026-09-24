import Charts
import SwiftUI

/// Main window, modeled on the current app: dark 248 px sidebar with no
/// title bar, roomy content area on the right.
struct MainWindow: View {
    @ObservedObject var state: AppState
    @State private var page: Page = .dictation

    enum Page: String, CaseIterable, Identifiable {
        case dictation, statistics, history, dictionary, snippets, styles, settings, about
        var id: String { rawValue }

        var title: String {
            switch self {
            case .dictation: L.t("Dictée")
            case .statistics: L.t("Statistiques")
            case .history: L.t("Historique")
            case .dictionary: L.t("Dictionnaire")
            case .snippets: L.t("Extraits")
            case .styles: L.t("Polissage")
            case .settings: L.t("Réglages")
            case .about: L.t("À propos")
            }
        }

        var subtitle: String {
            switch self {
            case .dictation: L.t("Votre activité et vos dernières transcriptions.")
            case .statistics: L.t("Votre usage de la dictée au fil du temps.")
            case .history: L.t("Toutes vos transcriptions, sur cet appareil.")
            case .dictionary: L.t("Les mots que la transcription doit écrire autrement.")
            case .snippets: L.t("Une phrase dictée, un texte long inséré.")
            case .styles: L.t("Comment vos dictées sont réécrites avant insertion.")
            case .settings: L.t("Langue, moteurs, raccourcis et autorisations.")
            case .about: L.t("Voice Flow, version native.")
            }
        }

        var symbol: String {
            switch self {
            case .dictation: "circle.grid.2x2"
            case .statistics: "chart.bar"
            case .history: "clock.arrow.circlepath"
            case .dictionary: "book"
            case .snippets: "text.alignleft"
            case .styles: "wand.and.sparkles"
            case .settings: "gearshape"
            case .about: "info.circle"
            }
        }
    }

    private static let mainPages: [Page] = [
        .dictation, .statistics, .history, .dictionary, .snippets, .styles,
    ]

    var body: some View {
        HStack(spacing: 0) {
            sidebar
            Rectangle().fill(VF.border).frame(width: 1)
            ScrollView {
                content
                    .padding(.horizontal, VF.contentPadding)
                    .padding(.top, 34)
                    .padding(.bottom, 40)
                    .frame(maxWidth: VF.contentMaxWidth, alignment: .leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .defaultScrollAnchor(.top)
            .background(VF.background)
        }
        .frame(minWidth: 940, minHeight: 640)
        // Rebuild the tree when the language toggles: labels are resolved
        // during rendering, so it has to happen again.
        .id(state.interfaceLanguage)
        .onAppear {
            state.refreshHistory()
            state.refreshPermissions()
        }
    }

    @ViewBuilder
    private var content: some View {
        VStack(alignment: .leading, spacing: 24) {
            VFPageHeader(title: page.title, subtitle: page.subtitle)

            switch page {
            case .dictation: DictationPage(state: state, go: { page = $0 })
            case .statistics: StatisticsPage(state: state)
            case .history: HistoryPage(state: state)
            case .dictionary: DictionaryPage()
            case .snippets: SnippetsPage()
            case .styles: StylesPage(state: state)
            case .settings: SettingsPage(state: state)
            case .about: AboutPage()
            }
        }
    }

    // MARK: Sidebar

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                VFLogo(size: 36)
                Text(L.t("Voice Flow"))
                    .font(.system(size: 22, weight: .semibold, design: .serif))
                    .italic()
                    .foregroundStyle(VF.label)
            }
            .padding(.horizontal, 20)
            .padding(.top, 24)   // adds to the hidden title bar area
            .padding(.bottom, 20)

            Rectangle().fill(VF.border).frame(height: 1)

            VStack(spacing: 5) {
                ForEach(Self.mainPages) { navItem($0) }
            }
            .padding(.horizontal, 12)
            .padding(.top, 16)

            Spacer(minLength: 20)

            VStack(spacing: 4) {
                navItem(.settings, badge: state.needsAttention)
                navItem(.about)
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 16)
        }
        .frame(width: VF.sidebarWidth, alignment: .leading)
        .background(VF.background)
    }

    private func navItem(_ item: Page, badge: Bool = false) -> some View {
        let selected = page == item
        return Button {
            page = item
        } label: {
            HStack(spacing: 12) {
                Image(systemName: item.symbol)
                    .font(.system(size: 16))
                    .frame(width: 22, height: 22)
                Text(L.t(item.title)).font(.system(size: 15))
                Spacer(minLength: 0)
                if badge {
                    Circle().fill(VF.red).frame(width: 7, height: 7)
                }
            }
            .foregroundStyle(selected ? VF.label : VF.labelMuted)
            .padding(.horizontal, 14)
            .padding(.vertical, 11)
            .background {
                if selected {
                    Capsule().fill(VF.card)
                    Capsule().strokeBorder(VF.border, lineWidth: 1)
                }
            }
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(item.title)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    }
}

// MARK: - Dictation

struct DictationPage: View {
    @ObservedObject var state: AppState
    var go: (MainWindow.Page) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            statusBand

            VStack(alignment: .leading, spacing: 14) {
                VFSectionHeader(
                    title: "7 derniers jours",
                    linkTitle: "Voir les statistiques",
                    action: { go(.statistics) })
                HStack(spacing: 14) {
                    VFMetric(label: "Mots", value: number(state.weekUsage.words))
                    VFMetric(label: "Dictées", value: number(state.weekUsage.dictations))
                    VFMetric(
                        label: "Minutes audio",
                        value: String(format: "%.1f", state.weekUsage.audioMinutes))
                    VFMetric(label: "Jours actifs", value: number(state.weekUsage.activeDays))
                }
            }

            VFCard(padding: 0) {
                VStack(alignment: .leading, spacing: 0) {
                    HStack {
                        Text(L.t("Transcriptions récentes"))
                            .font(.system(size: 17, weight: .medium))
                            .foregroundStyle(VF.label)
                        Spacer()
                        Button { go(.history) } label: {
                            Text(L.t("Ouvrir l'historique"))
                                .font(.system(size: 13))
                                .foregroundStyle(VF.labelMuted)
                                .underline()
                        }
                        .buttonStyle(.plain)
                    }
                    .padding(20)

                    if state.entries.isEmpty {
                        Text(L.t("Votre prochaine transcription apparaîtra ici."))
                            .font(.system(size: 14))
                            .foregroundStyle(VF.labelMuted)
                            .padding(.horizontal, 20)
                            .padding(.bottom, 24)
                    } else {
                        ForEach(state.entries.prefix(4)) { entry in
                            Rectangle().fill(VF.divider).frame(height: 1)
                            EntryRow(entry: entry, state: state)
                        }
                    }
                }
            }
        }
    }

    private var statusBand: some View {
        HStack(alignment: .center, spacing: 20) {
            VStack(alignment: .leading, spacing: 4) {
                Text(L.t(state.readinessTitle))
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(VF.label)
                Text(L.t(state.triggerHint))
                    .font(.system(size: 14))
                    .foregroundStyle(VF.labelMuted)
                if state.needsAttention {
                    Button { go(.settings) } label: {
                        Text(L.t("Ouvrir la configuration"))
                            .font(.system(size: 13))
                            .foregroundStyle(VF.labelMuted)
                            .underline()
                    }
                    .buttonStyle(.plain)
                    .padding(.top, 2)
                }
            }
            Spacer(minLength: 12)
            HStack(spacing: 14) {
                Text(state.engineChoice.shortLabel)
                    .font(.system(size: 13))
                    .foregroundStyle(VF.labelMuted)
                VFKeycap(text: state.dictateShortcut.display)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: VF.bandRadius, style: .continuous)
                .strokeBorder(VF.border, lineWidth: 1))
    }
}

func number(_ value: Int) -> String {
    let formatter = NumberFormatter()
    formatter.numberStyle = .decimal
    formatter.groupingSeparator = "\u{202F}"
    return formatter.string(from: NSNumber(value: value)) ?? "\(value)"
}

// MARK: - Statistics

struct StatisticsPage: View {
    @ObservedObject var state: AppState
    @State private var period: Period = .week
    @State private var data: (usage: HistoryStore.Usage, daily: [HistoryStore.DayPoint]) =
        (HistoryStore.Usage(), [])

    enum Period: String, CaseIterable, Identifiable {
        case week, month, all
        var id: String { rawValue }
        var title: String {
            switch self {
            case .week: L.t("7 jours")
            case .month: L.t("30 jours")
            case .all: L.t("Tout l'historique")
            }
        }
        var days: Int? {
            switch self {
            case .week: 7
            case .month: 30
            case .all: nil
            }
        }
    }

    /// Recomputed when the period changes or a dictation comes in.
    private var reloadKey: String {
        "\(period.rawValue)-\(state.entries.first?.id ?? "")-\(state.entries.count)"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            HStack(spacing: 8) {
                ForEach(Period.allCases) { item in
                    Button(item.title) { period = item }
                        .buttonStyle(VFButtonStyle(prominent: period == item))
                }
            }

            HStack(spacing: 14) {
                VFMetric(label: "Mots", value: number(data.usage.words))
                VFMetric(label: "Dictées", value: number(data.usage.dictations))
                VFMetric(
                    label: "Minutes audio",
                    value: String(format: "%.1f", data.usage.audioMinutes))
                VFMetric(label: "Jours actifs", value: number(data.usage.activeDays))
            }

            VFCard {
                VStack(alignment: .leading, spacing: 16) {
                    Text(L.t("Activité quotidienne"))
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(VF.label)

                    if data.daily.allSatisfy({ $0.words == 0 }) {
                        Text(L.t("Pas encore de données sur cette période."))
                            .font(.system(size: 14))
                            .foregroundStyle(VF.labelMuted)
                            .frame(height: 160)
                    } else {
                        Chart(data.daily) { point in
                            AreaMark(
                                x: .value(L.t("Jour"), point.day, unit: .day),
                                y: .value(L.t("Mots"), point.words))
                            .foregroundStyle(LinearGradient(
                                colors: [VF.label.opacity(0.14), VF.label.opacity(0.01)],
                                startPoint: .top, endPoint: .bottom))
                            .interpolationMethod(.monotone)

                            LineMark(
                                x: .value(L.t("Jour"), point.day, unit: .day),
                                y: .value(L.t("Mots"), point.words))
                            .foregroundStyle(VF.label)
                            .lineStyle(StrokeStyle(lineWidth: 2))
                            .interpolationMethod(.monotone)
                        }
                        .chartYAxis {
                            AxisMarks(position: .trailing) {
                                AxisGridLine().foregroundStyle(VF.divider)
                                AxisValueLabel().foregroundStyle(VF.labelFaint)
                            }
                        }
                        .chartXAxis {
                            AxisMarks { _ in
                                AxisValueLabel(format: .dateTime.day().month(.abbreviated))
                                    .foregroundStyle(VF.labelFaint)
                            }
                        }
                        .frame(height: 200)
                    }
                }
            }

            VFCard {
                VStack(alignment: .leading, spacing: 16) {
                    Text(L.t("Moteur de transcription"))
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(VF.label)
                    HStack(spacing: 0) {
                        // All engines actually used, from most to least called on.
                        let engines = data.usage.byEngine.sorted { $0.value > $1.value }
                        if engines.isEmpty {
                            engineColumn("Apple", 0, 0)
                        }
                        ForEach(engines, id: \.key) { engine in
                            engineColumn(
                                EngineChoice.historyDisplayName(engine.key),
                                engine.value, data.usage.dictations)
                        }
                    }
                }
            }
        }
        .task(id: reloadKey) {
            let days = period.days
            data = await Task.detached(priority: .userInitiated) {
                HistoryStore.shared.usage(days: days)
            }.value
        }
    }

    private func engineColumn(_ title: String, _ count: Int, _ total: Int) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L.t(title)).font(.system(size: 13)).foregroundStyle(VF.labelMuted)
            Text(number(count))
                .font(.system(size: 30))
                .foregroundStyle(VF.label)
                .monospacedDigit()
            if total > 0 {
                // Translatable format: element order varies from one language to another.
                Text(String(format: L.t("%d %% des dictées"),
                            Int(Double(count) / Double(total) * 100)))
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelFaint)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Transcription row

struct EntryRow: View {
    let entry: HistoryEntry
    @ObservedObject var state: AppState
    @State private var expanded = false
    @State private var hovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text(EngineChoice.historyDisplayName(entry.sttEngine))
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelMuted)
                Text(L.t("·")).foregroundStyle(VF.labelFaint)
                Text(entry.language.map { state.displayName(for: $0) } ?? "—")
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelMuted)
                if entry.polishApplied {
                    Text(L.t("·")).foregroundStyle(VF.labelFaint)
                    Text(L.t("poli")).font(.system(size: 12)).foregroundStyle(VF.purple)
                }
                if let appName = entry.appName {
                    Text(L.t("·")).foregroundStyle(VF.labelFaint)
                    Text(appName).font(.system(size: 12)).foregroundStyle(VF.labelMuted)
                }
                Spacer(minLength: 8)
                Text(entry.createdAt.formatted(.relative(presentation: .numeric).locale(L.locale)))
                    .font(.system(size: 12))
                    .foregroundStyle(VF.labelFaint)
                Image(systemName: expanded ? "chevron.up" : "chevron.down")
                    .font(.system(size: 11))
                    .foregroundStyle(VF.labelFaint)
            }

            Text(entry.finalText)
                .font(.system(size: 14))
                .foregroundStyle(VF.label)
                .lineLimit(expanded ? nil : 3)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)

            if expanded {
                HStack(spacing: 8) {
                    Button(L.t("Copier")) {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(entry.finalText, forType: .string)
                    }
                    .buttonStyle(VFButtonStyle())
                    Button(L.t("Insérer")) { state.insertEntry(entry) }
                        .buttonStyle(VFButtonStyle())
                    Spacer()
                    Button(L.t("Supprimer")) { state.deleteEntry(entry) }
                        .buttonStyle(VFButtonStyle())
                }
                .padding(.top, 2)
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(hovering ? VF.cardHover : Color.clear)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .onTapGesture { withAnimation(.easeOut(duration: 0.15)) { expanded.toggle() } }
    }
}
