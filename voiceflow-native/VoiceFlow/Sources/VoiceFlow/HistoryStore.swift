import Foundation
import NaturalLanguage
import SQLite3

/// A recorded dictation.
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
    let wordCount: Int
}

/// Counts the words of a dictation, including in languages written
/// without spaces (Chinese, Japanese) that SenseVoice and Qwen3-ASR
/// transcribe: splitting on spaces there counted a whole sentence as one
/// word.
enum WordCounter {
    static func count(_ text: String) -> Int {
        let needsSegmentation = text.unicodeScalars.contains {
            (0x3040...0x30FF).contains($0.value)      // kana
                || (0x3400...0x9FFF).contains($0.value)   // CJK ideographs
                || (0xAC00...0xD7AF).contains($0.value)   // hangul
        }
        guard needsSegmentation else {
            return text.split(whereSeparator: { $0.isWhitespace }).count
        }
        let tokenizer = NLTokenizer(unit: .word)
        tokenizer.string = text
        return tokenizer.tokens(for: text.startIndex..<text.endIndex).count
    }
}

/// Today's statistics shown on the home screen.
struct DayStats {
    var words = 0
    var dictations = 0
    var polished = 0
    var speakingSeconds = 0

    /// Words per minute of speech.
    var wordsPerMinute: Int {
        guard speakingSeconds > 0 else { return 0 }
        return Int(Double(words) / (Double(speakingSeconds) / 60))
    }
}

/// Local SQLite history.
///
/// The schema mirrors the Tauri app's
/// (`apps/desktop/src-tauri/src/history/store.rs`) to stay compatible: an
/// existing database can be copied or imported as-is. Columns specific to
/// the native app (`app_name`, `word_count`) are added through
/// migrations, tracked in the `native_meta` table.
///
/// All reads and writes to the connection go through `queue`: that's
/// what makes sharing across threads safe.
final class HistoryStore: @unchecked Sendable {
    static let shared = HistoryStore(
        path: URL.applicationSupportDirectory
            .appending(path: "VoiceFlow").appending(path: "history.db")
            .path(percentEncoded: false))

    private var db: OpaquePointer?
    private let queue = DispatchQueue(label: "fr.okatech.voiceflow.history")
    private static let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

    init(path: String) {
        queue.sync { open(path) }
    }

    private func open(_ path: String) {
        let directory = URL(fileURLWithPath: path).deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

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
        migrate()
    }

    // MARK: - Migrations

    /// Native schema version, stored separately: `PRAGMA user_version`
    /// belongs to the Tauri app (4 today), and a database copied from it
    /// would skip our migrations.
    private func nativeVersion() -> Int {
        exec("CREATE TABLE IF NOT EXISTS native_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL)")
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(
            db, "SELECT value FROM native_meta WHERE key = 'schema_version'", -1, &statement, nil)
            == SQLITE_OK
        else { return 0 }
        defer { sqlite3_finalize(statement) }
        return sqlite3_step(statement) == SQLITE_ROW ? Int(sqlite3_column_int(statement, 0)) : 0
    }

    private func setNativeVersion(_ version: Int) {
        exec("INSERT OR REPLACE INTO native_meta (key, value) VALUES ('schema_version', \(version))")
    }

    private func migrate() {
        // Columns specific to the native app are checked on every open,
        // regardless of version: without them, every insert would fail.
        let columnsReady = ensureColumn("app_name", type: "TEXT")
            && ensureColumn("word_count", type: "INTEGER")
        guard columnsReady else { return }

        if nativeVersion() < 1 {
            // v1 — the app name used to be stored in `source_path`,
            // which in Tauri designates the imported audio file. It gets
            // its own column back; a real path stays where it is.
            if exec("""
                UPDATE transcription_history
                SET app_name = source_path, source_path = NULL
                WHERE source_kind = 'recording' AND source_path IS NOT NULL
                  AND source_path NOT LIKE '/%' AND app_name IS NULL
                """) {
                setNativeVersion(1)
            }
        }
        // Word count for entries that don't have one yet (old, imported
        // ones): statistics are computed in SQL.
        backfillWordCounts()
    }

    /// Adds the column if missing; false if it's still absent.
    private func ensureColumn(_ name: String, type: String) -> Bool {
        guard !columnExists(name) else { return true }
        return exec("ALTER TABLE transcription_history ADD COLUMN \(name) \(type)")
    }

    private func columnExists(_ name: String) -> Bool {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(db, "PRAGMA table_info(transcription_history)", -1, &statement, nil) == SQLITE_OK
        else { return false }
        defer { sqlite3_finalize(statement) }
        while sqlite3_step(statement) == SQLITE_ROW {
            if text(statement, 1) == name { return true }
        }
        return false
    }

    private func backfillWordCounts() {
        var rows: [(String, Int)] = []
        var select: OpaquePointer?
        if sqlite3_prepare_v2(
            db, "SELECT id, final_text FROM transcription_history WHERE word_count IS NULL",
            -1, &select, nil) == SQLITE_OK {
            while sqlite3_step(select) == SQLITE_ROW {
                rows.append((text(select, 0) ?? "", WordCounter.count(text(select, 1) ?? "")))
            }
        }
        sqlite3_finalize(select)
        guard !rows.isEmpty else { return }

        exec("BEGIN")
        var update: OpaquePointer?
        if sqlite3_prepare_v2(
            db, "UPDATE transcription_history SET word_count = ? WHERE id = ?",
            -1, &update, nil) == SQLITE_OK {
            for (id, count) in rows {
                sqlite3_bind_int64(update, 1, Int64(count))
                bindText(update, 2, id)
                sqlite3_step(update)
                sqlite3_reset(update)
            }
        }
        sqlite3_finalize(update)
        exec("COMMIT")
    }

    @discardableResult
    private func exec(_ sql: String) -> Bool {
        guard sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK else {
            log.error("history sql failed: \(String(cString: sqlite3_errmsg(self.db)))")
            return false
        }
        return true
    }

    // MARK: - Writing

    func insert(
        rawText: String, finalText: String,
        appName: String?,
        engine: String, model: String?, language: String?,
        audioDurationMs: Int, sttDurationMs: Int,
        polishDurationMs: Int?, polishEngine: String?,
        createdAt: Date = Date()
    ) {
        queue.sync {
            let sql = """
                INSERT INTO transcription_history
                (id, created_at, raw_text, final_text, stt_engine, stt_model, language,
                 audio_duration_ms, stt_duration_ms, polish_duration_ms, total_duration_ms,
                 polish_applied, polish_engine, is_cloud, status, source_kind, app_name, word_count)
                VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,0,'success','recording',?,?)
                """
            var statement: OpaquePointer?
            guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else {
                let message = String(cString: sqlite3_errmsg(self.db))
                log.error("history insert prepare failed: \(message)")
                Diagnostics.log("history: unable to record (\(message))")
                return
            }
            defer { sqlite3_finalize(statement) }

            let total = audioDurationMs + sttDurationMs + (polishDurationMs ?? 0)
            bindText(statement, 1, UUID().uuidString)
            sqlite3_bind_int64(statement, 2, Int64(createdAt.timeIntervalSince1970 * 1000))
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
            sqlite3_bind_int64(statement, 15, Int64(WordCounter.count(finalText)))

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

    /// Deletes entries older than `days` days.
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
        queue.sync { _ = exec("DELETE FROM transcription_history") }
    }

    /// Picks up history from the Tauri app. Entries already present (same
    /// identifier) are ignored: the import can be re-run without
    /// duplicates. Returns the number of entries added.
    func importTauriHistory(from path: String) throws -> Int {
        try queue.sync {
            var attach: OpaquePointer?
            guard sqlite3_prepare_v2(db, "ATTACH DATABASE ? AS tauri", -1, &attach, nil) == SQLITE_OK else {
                throw HistoryError.sql(String(cString: sqlite3_errmsg(db)))
            }
            bindText(attach, 1, path)
            let attached = sqlite3_step(attach)
            sqlite3_finalize(attach)
            guard attached == SQLITE_DONE else {
                throw HistoryError.sql(String(cString: sqlite3_errmsg(db)))
            }
            defer { exec("DETACH DATABASE tauri") }

            let before = sqlite3_total_changes(db)
            let columns = """
                id, created_at, raw_text, final_text, stt_engine, stt_model, language,
                audio_duration_ms, stt_duration_ms, polish_duration_ms, total_duration_ms,
                polish_applied, polish_engine, is_cloud, audio_path, status, error,
                source_kind, source_path, translation_target, timed_segments, delivery_status
                """
            let sql = """
                INSERT OR IGNORE INTO main.transcription_history (\(columns))
                SELECT \(columns) FROM tauri.transcription_history
                WHERE status = 'success'
                """
            guard sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK else {
                throw HistoryError.sql(String(cString: sqlite3_errmsg(db)))
            }
            let added = Int(sqlite3_total_changes(db) - before)
            backfillWordCounts()
            return added
        }
    }

    enum HistoryError: LocalizedError {
        case sql(String)
        var errorDescription: String? {
            switch self {
            case .sql(let message): message
            }
        }
    }

    private func bindText(_ statement: OpaquePointer?, _ index: Int32, _ value: String?) {
        if let value {
            sqlite3_bind_text(statement, index, value, -1, Self.transient)
        } else {
            sqlite3_bind_null(statement, index)
        }
    }

    // MARK: - Reading

    func recent(limit: Int = 200) -> [HistoryEntry] {
        queue.sync {
            let sql = """
                SELECT id, created_at, raw_text, final_text, stt_engine, stt_model, language,
                       audio_duration_ms, stt_duration_ms, polish_duration_ms, polish_applied,
                       app_name, word_count
                FROM transcription_history ORDER BY created_at DESC LIMIT ?
                """
            var statement: OpaquePointer?
            guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else { return [] }
            defer { sqlite3_finalize(statement) }
            sqlite3_bind_int(statement, 1, Int32(limit))

            var entries: [HistoryEntry] = []
            while sqlite3_step(statement) == SQLITE_ROW {
                let finalText = text(statement, 3) ?? ""
                entries.append(HistoryEntry(
                    id: text(statement, 0) ?? UUID().uuidString,
                    createdAt: Date(timeIntervalSince1970: Double(sqlite3_column_int64(statement, 1)) / 1000),
                    rawText: text(statement, 2) ?? "",
                    finalText: finalText,
                    sttEngine: text(statement, 4) ?? "",
                    sttModel: text(statement, 5),
                    language: text(statement, 6),
                    audioDurationMs: Int(sqlite3_column_int64(statement, 7)),
                    sttDurationMs: Int(sqlite3_column_int64(statement, 8)),
                    polishDurationMs: sqlite3_column_type(statement, 9) == SQLITE_NULL
                        ? nil : Int(sqlite3_column_int64(statement, 9)),
                    polishApplied: sqlite3_column_int(statement, 10) != 0,
                    appName: text(statement, 11),
                    wordCount: sqlite3_column_type(statement, 12) == SQLITE_NULL
                        ? WordCounter.count(finalText) : Int(sqlite3_column_int64(statement, 12))))
            }
            return entries
        }
    }

    /// Usage summary over a period, like the "last 7 days" banner in the
    /// current app.
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

    /// Local day of an entry, computed by SQLite.
    private static let localDay = "date(created_at / 1000, 'unixepoch', 'localtime')"

    /// `days == nil`: the whole history. Computed in SQL: no limit on
    /// the number of entries considered.
    func usage(days: Int?) -> (usage: Usage, daily: [DayPoint]) {
        let calendar = Calendar.current
        let today = calendar.startOfDay(for: Date())
        let from = days.map { calendar.date(byAdding: .day, value: -($0 - 1), to: today)! }
        let fromMs = Int64((from ?? .distantPast).timeIntervalSince1970 * 1000)

        return queue.sync {
            var usage = Usage()
            var perDay: [Date: (words: Int, dictations: Int)] = [:]

            query("""
                SELECT \(Self.localDay), COALESCE(SUM(word_count), 0), COUNT(*),
                       COALESCE(SUM(audio_duration_ms), 0)
                FROM transcription_history WHERE created_at >= ? GROUP BY 1
                """, fromMs) { statement in
                guard let dayText = text(statement, 0), let day = Self.parseDay(dayText) else { return }
                let words = Int(sqlite3_column_int64(statement, 1))
                let count = Int(sqlite3_column_int64(statement, 2))
                perDay[day] = (words, count)
                usage.words += words
                usage.dictations += count
                usage.audioSeconds += Int(sqlite3_column_int64(statement, 3) / 1000)
            }
            query("""
                SELECT stt_engine, COUNT(*) FROM transcription_history
                WHERE created_at >= ? GROUP BY stt_engine
                """, fromMs) { statement in
                usage.byEngine[text(statement, 0) ?? "", default: 0] += Int(sqlite3_column_int64(statement, 1))
            }
            usage.activeDays = perDay.count

            // Continuous series, empty days included, so the curve doesn't jump.
            var daily: [DayPoint] = []
            var cursor = from ?? perDay.keys.min() ?? today
            while cursor <= today {
                let point = perDay[cursor] ?? (0, 0)
                daily.append(DayPoint(day: cursor, words: point.words, dictations: point.dictations))
                cursor = calendar.date(byAdding: .day, value: 1, to: cursor)!
            }
            return (usage, daily)
        }
    }

    /// Today's statistics + words per hour for the activity curve.
    func todayStats() -> (stats: DayStats, hourly: [Int]) {
        let startOfDay = Int64(Calendar.current.startOfDay(for: Date()).timeIntervalSince1970 * 1000)
        return queue.sync {
            var stats = DayStats()
            var hourly = [Int](repeating: 0, count: 24)
            query("""
                SELECT CAST(strftime('%H', created_at / 1000, 'unixepoch', 'localtime') AS INTEGER),
                       COALESCE(SUM(word_count), 0), COUNT(*),
                       COALESCE(SUM(audio_duration_ms), 0), COALESCE(SUM(polish_applied), 0)
                FROM transcription_history WHERE created_at >= ? GROUP BY 1
                """, startOfDay) { statement in
                let hour = Int(sqlite3_column_int(statement, 0))
                let words = Int(sqlite3_column_int64(statement, 1))
                if (0..<24).contains(hour) { hourly[hour] += words }
                stats.words += words
                stats.dictations += Int(sqlite3_column_int64(statement, 2))
                stats.speakingSeconds += Int(sqlite3_column_int64(statement, 3) / 1000)
                stats.polished += Int(sqlite3_column_int64(statement, 4))
            }
            return (stats, hourly)
        }
    }

    /// Everything the home screen and history display, read off the
    /// main thread.
    struct Snapshot {
        var entries: [HistoryEntry]
        var today: (stats: DayStats, hourly: [Int])
        var week: Usage
    }

    func snapshot() async -> Snapshot {
        await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                continuation.resume(returning: Snapshot(
                    entries: self.recent(),
                    today: self.todayStats(),
                    week: self.usage(days: 7).usage))
            }
        }
    }

    private func query(_ sql: String, _ value: Int64, row: (OpaquePointer?) -> Void) {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else {
            log.error("history query failed: \(String(cString: sqlite3_errmsg(self.db)))")
            return
        }
        defer { sqlite3_finalize(statement) }
        sqlite3_bind_int64(statement, 1, value)
        while sqlite3_step(statement) == SQLITE_ROW { row(statement) }
    }

    private static func parseDay(_ text: String) -> Date? {
        let parts = text.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3 else { return nil }
        return Calendar.current.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))
    }

    private func text(_ statement: OpaquePointer?, _ index: Int32) -> String? {
        guard let cString = sqlite3_column_text(statement, index) else { return nil }
        return String(cString: cString)
    }
}
