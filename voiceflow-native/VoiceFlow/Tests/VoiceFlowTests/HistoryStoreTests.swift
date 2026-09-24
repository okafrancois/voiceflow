import Foundation
import SQLite3
import Testing
@testable import VoiceFlow

struct HistoryStoreTests {
    private func temporaryPath() -> String {
        FileManager.default.temporaryDirectory
            .appending(path: "history-\(UUID().uuidString).db")
            .path(percentEncoded: false)
    }

    private func insert(_ store: HistoryStore, _ text: String, engine: String = "apple",
                        app: String? = "Mail", at date: Date = Date()) {
        store.insert(
            rawText: text, finalText: text, appName: app, engine: engine, model: nil,
            language: "fr-FR", audioDurationMs: 60_000, sttDurationMs: 100,
            polishDurationMs: nil, polishEngine: nil, createdAt: date)
    }

    @Test func appNameHasItsOwnColumn() {
        let store = HistoryStore(path: temporaryPath())
        insert(store, "bonjour", app: "Notes")
        let entry = store.recent().first
        #expect(entry?.appName == "Notes")
    }

    @Test func legacyRowsMoveTheAppNameOutOfSourcePath() throws {
        let path = temporaryPath()
        var db: OpaquePointer?
        #expect(sqlite3_open(path, &db) == SQLITE_OK)
        let legacy = """
            CREATE TABLE transcription_history (
                id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, raw_text TEXT NOT NULL,
                final_text TEXT NOT NULL, stt_engine TEXT NOT NULL, stt_model TEXT, language TEXT,
                audio_duration_ms INTEGER, stt_duration_ms INTEGER, polish_duration_ms INTEGER,
                total_duration_ms INTEGER, polish_applied INTEGER NOT NULL DEFAULT 0,
                polish_engine TEXT, is_cloud INTEGER NOT NULL DEFAULT 0, audio_path TEXT,
                status TEXT NOT NULL DEFAULT 'success', error TEXT,
                source_kind TEXT NOT NULL DEFAULT 'recording', source_path TEXT,
                translation_target TEXT, timed_segments TEXT NOT NULL DEFAULT '[]',
                delivery_status TEXT NOT NULL DEFAULT 'not_recorded');
            INSERT INTO transcription_history (id, created_at, raw_text, final_text, stt_engine, source_path)
                VALUES ('a', 1, 'un deux trois', 'un deux trois', 'whisper', 'Safari');
            INSERT INTO transcription_history (id, created_at, raw_text, final_text, stt_engine, source_kind, source_path)
                VALUES ('b', 2, 'x', 'x', 'whisper', 'file', '/Users/me/memo.m4a');
            """
        #expect(sqlite3_exec(db, legacy, nil, nil, nil) == SQLITE_OK)
        sqlite3_close(db)

        let store = HistoryStore(path: path)
        let entries = store.recent()
        #expect(entries.first { $0.id == "a" }?.appName == "Safari")
        #expect(entries.first { $0.id == "a" }?.wordCount == 3)
        // A real file path stays a path.
        #expect(entries.first { $0.id == "b" }?.appName == nil)
    }

    @Test func aCopiedTauriDatabaseStillGetsTheNativeColumns() {
        let path = temporaryPath()
        var db: OpaquePointer?
        #expect(sqlite3_open(path, &db) == SQLITE_OK)
        // Tauri tracks its own schema in user_version (4 today).
        let tauri = """
            CREATE TABLE transcription_history (
                id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, raw_text TEXT NOT NULL,
                final_text TEXT NOT NULL, stt_engine TEXT NOT NULL, stt_model TEXT, language TEXT,
                audio_duration_ms INTEGER, stt_duration_ms INTEGER, polish_duration_ms INTEGER,
                total_duration_ms INTEGER, polish_applied INTEGER NOT NULL DEFAULT 0,
                polish_engine TEXT, is_cloud INTEGER NOT NULL DEFAULT 0, audio_path TEXT,
                status TEXT NOT NULL DEFAULT 'success', error TEXT,
                source_kind TEXT NOT NULL DEFAULT 'recording', source_path TEXT,
                translation_target TEXT, timed_segments TEXT NOT NULL DEFAULT '[]',
                delivery_status TEXT NOT NULL DEFAULT 'not_recorded');
            PRAGMA user_version = 4;
            """
        #expect(sqlite3_exec(db, tauri, nil, nil, nil) == SQLITE_OK)
        sqlite3_close(db)

        let store = HistoryStore(path: path)
        insert(store, "encore une dictée", app: "Notes")
        #expect(store.recent().first?.appName == "Notes")
        #expect(store.recent().first?.wordCount == 3)
    }

    @Test func statisticsCoverTheWholeHistory() {
        let store = HistoryStore(path: temporaryPath())
        let old = Calendar.current.date(byAdding: .day, value: -40, to: Date())!
        for _ in 0..<3 { insert(store, "un deux", engine: "whisper", at: old) }
        insert(store, "trois quatre cinq", engine: "sensevoice")

        let all = store.usage(days: nil)
        #expect(all.usage.dictations == 4)
        #expect(all.usage.words == 9)
        #expect(all.usage.activeDays == 2)
        #expect(all.usage.byEngine == ["whisper": 3, "sensevoice": 1])
        #expect(all.daily.count == 41)

        let week = store.usage(days: 7)
        #expect(week.usage.dictations == 1)
        #expect(week.daily.count == 7)
    }

    @Test func todayStatsBucketWordsByHour() {
        let store = HistoryStore(path: temporaryPath())
        insert(store, "un deux trois")
        let today = store.todayStats()
        #expect(today.stats.words == 3)
        #expect(today.stats.dictations == 1)
        #expect(today.hourly[Calendar.current.component(.hour, from: Date())] == 3)
    }

    @Test func wordsAreCountedInLanguagesWithoutSpaces() {
        #expect(WordCounter.count("bonjour tout le monde") == 4)
        #expect(WordCounter.count("今天天气很好") > 1)
    }
}
