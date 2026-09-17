import Foundation
import SQLite3

/// Une dictée enregistrée.
struct HistoryEntry: Identifiable, Hashable {
    let id: String
    let createdAt: Date
    let rawText: String
    let finalText: String
    let sttEngine: String
    let sttModel: String?
    let language: String?
    let audioDurationMs: Int
    let sttDurationMs: Int
    let polishDurationMs: Int?
    let polishApplied: Bool
    let appName: String?

    var wordCount: Int {
        finalText.split { $0 == " " || $0 == "\n" || $0 == "\t" }.count
    }
}

/// Statistiques du jour affichées sur l'accueil.
struct DayStats {
    var words = 0
    var dictations = 0
    var polished = 0
    var speakingSeconds = 0

    /// Mots par minute de parole.
    var wordsPerMinute: Int {
        guard speakingSeconds > 0 else { return 0 }
        return Int(Double(words) / (Double(speakingSeconds) / 60))
    }
}

/// Historique local en SQLite.
///
/// Le schéma reprend celui de l'app Tauri
/// (`apps/desktop/src-tauri/src/history/store.rs`) pour rester compatible :
/// une base existante peut être copiée telle quelle dans le dossier de
/// l'app native.
final class HistoryStore {
    static let shared = HistoryStore()

    private var db: OpaquePointer?
    private let queue = DispatchQueue(label: "fr.okatech.voiceflow.history")
    private static let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

    private init() {
        queue.sync { open() }
    }

    private func open() {
        let directory = URL.applicationSupportDirectory.appending(path: "VoiceFlow")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let path = directory.appending(path: "history.db").path(percentEncoded: false)

        guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, nil) == SQLITE_OK else {
            log.error("history: cannot open \(path)")
            return
        }
        exec("PRAGMA journal_mode=WAL")
        exec("""
            CREATE TABLE IF NOT EXISTS transcription_history (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                raw_text TEXT NOT NULL,
                final_text TEXT NOT NULL,
                stt_engine TEXT NOT NULL,
                stt_model TEXT,
                language TEXT,
                audio_duration_ms INTEGER,
                stt_duration_ms INTEGER,
                polish_duration_ms INTEGER,
                total_duration_ms INTEGER,
                polish_applied INTEGER NOT NULL DEFAULT 0,
                polish_engine TEXT,
                is_cloud INTEGER NOT NULL DEFAULT 0,
                audio_path TEXT,
                status TEXT NOT NULL DEFAULT 'success',
                error TEXT,
                source_kind TEXT NOT NULL DEFAULT 'recording',
                source_path TEXT,
                translation_target TEXT,
                timed_segments TEXT NOT NULL DEFAULT '[]',
                delivery_status TEXT NOT NULL DEFAULT 'not_recorded'
            )
            """)
        exec("CREATE INDEX IF NOT EXISTS idx_history_created_at ON transcription_history(created_at)")
    }

    private func exec(_ sql: String) {
        if sqlite3_exec(db, sql, nil, nil, nil) != SQLITE_OK {
            log.error("history sql failed: \(String(cString: sqlite3_errmsg(self.db)))")
        }
    }

    // MARK: - Écriture

    func insert(
        rawText: String, finalText: String,
        appName: String?,
        engine: String, model: String?, language: String?,
        audioDurationMs: Int, sttDurationMs: Int,
        polishDurationMs: Int?, polishEngine: String?
    ) {
        queue.sync {
            let sql = """
                INSERT INTO transcription_history
                (id, created_at, raw_text, final_text, stt_engine, stt_model, language,
                 audio_duration_ms, stt_duration_ms, polish_duration_ms, total_duration_ms,
                 polish_applied, polish_engine, is_cloud, status, source_kind, source_path)
                VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,0,'success','recording',?)
                """
            var statement: OpaquePointer?
            guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else { return }
            defer { sqlite3_finalize(statement) }

            let total = audioDurationMs + sttDurationMs + (polishDurationMs ?? 0)
            bindText(statement, 1, UUID().uuidString)
            sqlite3_bind_int64(statement, 2, Int64(Date().timeIntervalSince1970 * 1000))
            bindText(statement, 3, rawText)
            bindText(statement, 4, finalText)
            bindText(statement, 5, engine)
            bindText(statement, 6, model)
            bindText(statement, 7, language)
            sqlite3_bind_int64(statement, 8, Int64(audioDurationMs))
            sqlite3_bind_int64(statement, 9, Int64(sttDurationMs))
            if let polishDurationMs {
                sqlite3_bind_int64(statement, 10, Int64(polishDurationMs))
            } else {
                sqlite3_bind_null(statement, 10)
            }
            sqlite3_bind_int64(statement, 11, Int64(total))
            sqlite3_bind_int(statement, 12, polishDurationMs == nil ? 0 : 1)
            bindText(statement, 13, polishEngine)
            bindText(statement, 14, appName)

            if sqlite3_step(statement) != SQLITE_DONE {
                log.error("history insert failed: \(String(cString: sqlite3_errmsg(self.db)))")
            }
        }
    }

    func delete(id: String) {
        queue.sync {
            var statement: OpaquePointer?
            guard sqlite3_prepare_v2(db, "DELETE FROM transcription_history WHERE id = ?", -1, &statement, nil) == SQLITE_OK
            else { return }
            defer { sqlite3_finalize(statement) }
            bindText(statement, 1, id)
            sqlite3_step(statement)
        }
    }

    /// Supprime les entrées plus vieilles que `days` jours.
    func deleteOlderThan(days: Int) {
        queue.sync {
            let cutoff = Date().addingTimeInterval(-Double(days) * 86400).timeIntervalSince1970 * 1000
            var statement: OpaquePointer?
            guard sqlite3_prepare_v2(
                db, "DELETE FROM transcription_history WHERE created_at < ?", -1, &statement, nil)
                == SQLITE_OK
            else { return }
            defer { sqlite3_finalize(statement) }
            sqlite3_bind_int64(statement, 1, Int64(cutoff))
            sqlite3_step(statement)
        }
    }

    func deleteAll() {
        queue.sync { exec("DELETE FROM transcription_history") }
    }

    private func bindText(_ statement: OpaquePointer?, _ index: Int32, _ value: String?) {
        if let value {
            sqlite3_bind_text(statement, index, value, -1, Self.transient)
        } else {
            sqlite3_bind_null(statement, index)
        }
    }

    // MARK: - Lecture

    func recent(limit: Int = 200) -> [HistoryEntry] {
        queue.sync {
            let sql = """
                SELECT id, created_at, raw_text, final_text, stt_engine, stt_model, language,
                       audio_duration_ms, stt_duration_ms, polish_duration_ms, polish_applied,
                       source_path
                FROM transcription_history ORDER BY created_at DESC LIMIT ?
                """
            var statement: OpaquePointer?
            guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else { return [] }
            defer { sqlite3_finalize(statement) }
            sqlite3_bind_int(statement, 1, Int32(limit))

            var entries: [HistoryEntry] = []
            while sqlite3_step(statement) == SQLITE_ROW {
                entries.append(HistoryEntry(
                    id: text(statement, 0) ?? UUID().uuidString,
                    createdAt: Date(timeIntervalSince1970: Double(sqlite3_column_int64(statement, 1)) / 1000),
                    rawText: text(statement, 2) ?? "",
                    finalText: text(statement, 3) ?? "",
                    sttEngine: text(statement, 4) ?? "",
                    sttModel: text(statement, 5),
                    language: text(statement, 6),
                    audioDurationMs: Int(sqlite3_column_int64(statement, 7)),
                    sttDurationMs: Int(sqlite3_column_int64(statement, 8)),
                    polishDurationMs: sqlite3_column_type(statement, 9) == SQLITE_NULL
                        ? nil : Int(sqlite3_column_int64(statement, 9)),
                    polishApplied: sqlite3_column_int(statement, 10) != 0,
                    appName: text(statement, 11)))
            }
            return entries
        }
    }

    /// Résumé d'usage sur une période, comme le bandeau « 7 derniers jours »
    /// de l'app actuelle.
    struct Usage {
        var words = 0
        var dictations = 0
        var audioSeconds = 0
        var activeDays = 0
        var byEngine: [String: Int] = [:]

        var audioMinutes: Double { Double(audioSeconds) / 60 }
    }

    struct DayPoint: Identifiable {
        var id: Date { day }
        let day: Date
        let words: Int
        let dictations: Int
    }

    /// `days == nil` : tout l'historique.
    func usage(days: Int?) -> (usage: Usage, daily: [DayPoint]) {
        let calendar = Calendar.current
        let from = days.map {
            calendar.date(byAdding: .day, value: -($0 - 1), to: calendar.startOfDay(for: Date()))!
        }

        var usage = Usage()
        var perDay: [Date: (words: Int, dictations: Int)] = [:]

        for entry in recent(limit: 5000) {
            if let from, entry.createdAt < from { continue }
            let day = calendar.startOfDay(for: entry.createdAt)
            let words = entry.wordCount
            usage.words += words
            usage.dictations += 1
            usage.audioSeconds += entry.audioDurationMs / 1000
            usage.byEngine[entry.sttEngine, default: 0] += 1
            perDay[day, default: (0, 0)].words += words
            perDay[day, default: (0, 0)].dictations += 1
        }
        usage.activeDays = perDay.count

        // Série continue, jours vides compris, pour que la courbe ne saute pas.
        var daily: [DayPoint] = []
        let start = from ?? perDay.keys.min() ?? calendar.startOfDay(for: Date())
        var cursor = start
        let today = calendar.startOfDay(for: Date())
        while cursor <= today {
            let point = perDay[cursor] ?? (0, 0)
            daily.append(DayPoint(day: cursor, words: point.words, dictations: point.dictations))
            cursor = calendar.date(byAdding: .day, value: 1, to: cursor)!
        }
        return (usage, daily)
    }

    /// Statistiques du jour + mots par heure pour la courbe d'activité.
    func todayStats() -> (stats: DayStats, hourly: [Int]) {
        let startOfDay = Calendar.current.startOfDay(for: Date())
        var stats = DayStats()
        var hourly = [Int](repeating: 0, count: 24)

        for entry in recent(limit: 1000) where entry.createdAt >= startOfDay {
            let words = entry.wordCount
            stats.words += words
            stats.dictations += 1
            stats.speakingSeconds += entry.audioDurationMs / 1000
            if entry.polishApplied { stats.polished += 1 }
            let hour = Calendar.current.component(.hour, from: entry.createdAt)
            hourly[hour] += words
        }
        return (stats, hourly)
    }

    private func text(_ statement: OpaquePointer?, _ index: Int32) -> String? {
        guard let cString = sqlite3_column_text(statement, index) else { return nil }
        return String(cString: cString)
    }
}
