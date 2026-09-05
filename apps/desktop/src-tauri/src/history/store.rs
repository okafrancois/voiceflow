use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, Result as SqlResult};
use unicode_segmentation::UnicodeSegmentation;

use super::models::{
    DailyStatistics, HistoryFilter, HistoryStatistics, NewTranscriptionEntry, StatisticsPeriod,
    TranscriptionEntry,
};
use super::RetentionPolicy;
#[cfg(not(test))]
use crate::utils::AppPaths;

const CREATE_TABLE_SQL: &str = "\
CREATE TABLE IF NOT EXISTS transcription_history (\
    id TEXT PRIMARY KEY,\
    created_at INTEGER NOT NULL,\
    raw_text TEXT NOT NULL,\
    final_text TEXT NOT NULL,\
    stt_engine TEXT NOT NULL,\
    stt_model TEXT,\
    language TEXT,\
    audio_duration_ms INTEGER,\
    stt_duration_ms INTEGER,\
    polish_duration_ms INTEGER,\
    total_duration_ms INTEGER,\
    polish_applied INTEGER NOT NULL DEFAULT 0,\
    polish_engine TEXT,\
    is_cloud INTEGER NOT NULL DEFAULT 0,\
    audio_path TEXT,\
    status TEXT NOT NULL DEFAULT 'success',\
    error TEXT,\
    source_kind TEXT NOT NULL DEFAULT 'recording',\
    source_path TEXT,\
    translation_target TEXT,\
    timed_segments TEXT NOT NULL DEFAULT '[]',\
    delivery_status TEXT NOT NULL DEFAULT 'not_recorded'\
)";

const CREATE_INDEX_SQL: &str = "\
CREATE INDEX IF NOT EXISTS idx_history_created_at ON transcription_history(created_at)";

const CREATE_RETAINED_AUDIO_TABLE_SQL: &str = "\
CREATE TABLE IF NOT EXISTS retained_audio (\
    path TEXT PRIMARY KEY,\
    created_at INTEGER NOT NULL\
)";

const CREATE_RETAINED_AUDIO_INDEX_SQL: &str = "\
CREATE INDEX IF NOT EXISTS idx_retained_audio_created_at ON retained_audio(created_at)";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct RetentionCleanupReport {
    pub text_entries_deleted: u64,
    pub audio_files_deleted: u64,
    pub missing_audio_references_cleared: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct RetentionStatus {
    pub text_entries: u64,
    pub audio_files: u64,
    pub audio_bytes: u64,
}

pub struct HistoryStore {
    conn: parking_lot::Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestTranscriptionText {
    pub id: String,
    pub final_text: String,
}

/// Updates to apply to an entry after retry.
#[derive(Debug, Clone)]
pub struct EntryUpdates {
    pub raw_text: String,
    pub final_text: String,
    pub stt_engine: String,
    pub stt_model: Option<String>,
    pub language: Option<String>,
    pub stt_duration_ms: Option<i64>,
    pub polish_duration_ms: Option<i64>,
    pub polish_applied: bool,
    pub polish_engine: Option<String>,
    pub is_cloud: bool,
}

#[derive(Debug, Clone)]
pub struct WorkbenchEntryUpdates {
    pub raw_text: String,
    pub final_text: String,
    pub stt_engine: String,
    pub stt_model: Option<String>,
    pub language: Option<String>,
    pub audio_duration_ms: Option<i64>,
    pub stt_duration_ms: Option<i64>,
    pub polish_duration_ms: Option<i64>,
    pub total_duration_ms: Option<i64>,
    pub polish_applied: bool,
    pub polish_engine: Option<String>,
    pub is_cloud: bool,
    pub translation_target: Option<String>,
    pub timed_segments: Vec<super::models::TimedSegment>,
}

impl HistoryStore {
    pub fn new_in_memory() -> Result<Self, String> {
        let connection = Connection::open_in_memory()
            .map_err(|error| format!("failed to open in-memory history database: {error}"))?;
        Self::from_connection(connection)
    }

    pub fn new() -> Result<Self, String> {
        #[cfg(test)]
        {
            Self::new_in_memory()
        }

        #[cfg(not(test))]
        {
            let db_path = AppPaths::data_dir().join("transcription_history.db");
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let conn =
                Connection::open(&db_path).map_err(|e| format!("failed to open database: {e}"))?;
            Self::from_connection(conn)
        }
    }

    fn from_connection(conn: Connection) -> Result<Self, String> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| format!("failed to set pragmas: {e}"))?;

        Self::run_migrations(&conn)?;

        Ok(Self {
            conn: parking_lot::Mutex::new(conn),
        })
    }

    fn run_migrations(conn: &Connection) -> Result<(), String> {
        let current_version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap_or(0);

        if current_version < 1 {
            conn.execute_batch(
                format!(
                    "BEGIN;
                     {CREATE_TABLE_SQL};
                     {CREATE_INDEX_SQL};
                     PRAGMA user_version = 1;
                     COMMIT;"
                )
                .as_str(),
            )
            .map_err(|e| format!("migration v1 failed: {e}"))?;
        }

        // Migration v2: Add audio_path, status, error columns (only if not present)
        // These columns are now in CREATE_TABLE_SQL, but existing DBs may need migration
        if current_version < 2 {
            // Check if columns already exist (handles case where schema includes them)
            let has_audio_path: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('transcription_history') WHERE name='audio_path'",
                    [],
                    |row| row.get::<_, i32>(0),
                )
                .unwrap_or(0) > 0;

            if !has_audio_path {
                conn.execute_batch(
                    "BEGIN;
                     ALTER TABLE transcription_history ADD COLUMN audio_path TEXT;
                     ALTER TABLE transcription_history ADD COLUMN status TEXT NOT NULL DEFAULT 'success';
                     ALTER TABLE transcription_history ADD COLUMN error TEXT;
                     CREATE INDEX IF NOT EXISTS idx_history_status ON transcription_history(status);
                     COMMIT;",
                )
                .map_err(|e| format!("migration v2 failed: {e}"))?;
            }
            // Always set version to 2 after schema is complete
            conn.execute("PRAGMA user_version = 2", [])
                .map_err(|e| format!("failed to set user_version: {e}"))?;
        }

        if current_version < 3 {
            conn.execute_batch(
                format!(
                    "BEGIN;
                     {CREATE_RETAINED_AUDIO_TABLE_SQL};
                     {CREATE_RETAINED_AUDIO_INDEX_SQL};
                     INSERT OR IGNORE INTO retained_audio (path, created_at)
                     SELECT audio_path, created_at FROM transcription_history
                     WHERE audio_path IS NOT NULL AND audio_path != '';
                     PRAGMA user_version = 3;
                     COMMIT;"
                )
                .as_str(),
            )
            .map_err(|e| format!("migration v3 failed: {e}"))?;
        }

        if current_version < 4 {
            Self::add_column_if_missing(conn, "source_kind", "TEXT NOT NULL DEFAULT 'recording'")?;
            Self::add_column_if_missing(conn, "source_path", "TEXT")?;
            Self::add_column_if_missing(conn, "translation_target", "TEXT")?;
            Self::add_column_if_missing(conn, "timed_segments", "TEXT NOT NULL DEFAULT '[]'")?;
            Self::add_column_if_missing(
                conn,
                "delivery_status",
                "TEXT NOT NULL DEFAULT 'not_recorded'",
            )?;
            conn.execute("PRAGMA user_version = 4", [])
                .map_err(|e| format!("failed to set history schema version 4: {e}"))?;
        }

        Ok(())
    }

    fn add_column_if_missing(
        conn: &Connection,
        column: &str,
        declaration: &str,
    ) -> Result<(), String> {
        let present: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('transcription_history') WHERE name = ?1",
                params![column],
                |row| row.get(0),
            )
            .map_err(|error| format!("failed to inspect history schema: {error}"))?;
        if present == 0 {
            conn.execute_batch(
                format!("ALTER TABLE transcription_history ADD COLUMN {column} {declaration};")
                    .as_str(),
            )
            .map_err(|error| format!("migration v4 failed for {column}: {error}"))?;
        }
        Ok(())
    }

    pub fn insert(&self, entry: NewTranscriptionEntry) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().timestamp_millis();
        let retained_audio_path = entry.audio_path.clone();

        let mut conn = self.conn.lock();
        let tx = conn
            .transaction()
            .map_err(|e| format!("failed to start history insert: {e}"))?;
        tx.execute(
            "INSERT INTO transcription_history \
             (id, created_at, raw_text, final_text, stt_engine, stt_model, language, \
              audio_duration_ms, stt_duration_ms, polish_duration_ms, total_duration_ms, \
              polish_applied, polish_engine, is_cloud, audio_path, status, error, source_kind, \
              source_path, translation_target, timed_segments, delivery_status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                id,
                created_at,
                entry.raw_text,
                entry.final_text,
                entry.stt_engine,
                entry.stt_model,
                entry.language,
                entry.audio_duration_ms,
                entry.stt_duration_ms,
                entry.polish_duration_ms,
                entry.total_duration_ms,
                entry.polish_applied as i32,
                entry.polish_engine,
                entry.is_cloud as i32,
                entry.audio_path,
                entry.status,
                entry.error,
                entry.source_kind,
                entry.source_path,
                entry.translation_target,
                serde_json::to_string(&entry.timed_segments)
                    .map_err(|error| format!("failed to serialize timed segments: {error}"))?,
                entry.delivery_status,
            ],
        )
        .map_err(|e| format!("failed to insert history: {e}"))?;

        if let Some(path) = retained_audio_path.as_deref() {
            tx.execute(
                "INSERT OR REPLACE INTO retained_audio (path, created_at) VALUES (?1, ?2)",
                params![path, created_at],
            )
            .map_err(|e| format!("failed to register retained audio: {e}"))?;
        }

        tx.commit()
            .map_err(|e| format!("failed to commit history insert: {e}"))?;

        Ok(id)
    }

    pub fn get_history(&self, filter: &HistoryFilter) -> Result<Vec<TranscriptionEntry>, String> {
        let mut sql = String::from(
            "SELECT id, created_at, raw_text, final_text, stt_engine, \
             stt_model, language, audio_duration_ms, stt_duration_ms, polish_duration_ms, \
             total_duration_ms, polish_applied, polish_engine, is_cloud, audio_path, status, error, \
             source_kind, source_path, translation_target, timed_segments, delivery_status \
             FROM transcription_history WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref search) = filter.search {
            sql.push_str(&format!(" AND final_text LIKE ?{param_idx}"));
            param_values.push(Box::new(format!("%{search}%")));
            param_idx += 1;
        }

        if let Some(ref engine) = filter.engine {
            if engine == "local" {
                sql.push_str(" AND is_cloud = 0");
            } else if engine == "cloud" {
                sql.push_str(" AND is_cloud = 1");
            } else {
                sql.push_str(&format!(" AND stt_engine = ?{param_idx}"));
                param_values.push(Box::new(engine.clone()));
                param_idx += 1;
            }
        }

        if let Some(ref status) = filter.status {
            sql.push_str(&format!(" AND status = ?{param_idx}"));
            param_values.push(Box::new(status.clone()));
            param_idx += 1;
        }

        if let Some(date_from) = filter.date_from {
            sql.push_str(&format!(" AND created_at >= ?{param_idx}"));
            param_values.push(Box::new(date_from));
            param_idx += 1;
        }

        if let Some(date_to) = filter.date_to {
            sql.push_str(&format!(" AND created_at <= ?{param_idx}"));
            param_values.push(Box::new(date_to));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        let limit = filter.limit.unwrap_or(50);
        sql.push_str(&format!(" LIMIT ?{param_idx}"));
        param_values.push(Box::new(limit));
        param_idx += 1;

        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET ?{param_idx}"));
            param_values.push(Box::new(offset));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("failed to prepare query: {e}"))?;

        let entries = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(TranscriptionEntry {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    raw_text: row.get(2)?,
                    final_text: row.get(3)?,
                    stt_engine: row.get(4)?,
                    stt_model: row.get(5)?,
                    language: row.get(6)?,
                    audio_duration_ms: row.get(7)?,
                    stt_duration_ms: row.get(8)?,
                    polish_duration_ms: row.get(9)?,
                    total_duration_ms: row.get(10)?,
                    polish_applied: row.get::<_, i32>(11)? != 0,
                    polish_engine: row.get(12)?,
                    is_cloud: row.get::<_, i32>(13)? != 0,
                    audio_path: row.get(14)?,
                    status: row.get::<_, String>(15)?,
                    error: row.get(16)?,
                    source_kind: row.get(17)?,
                    source_path: row.get(18)?,
                    translation_target: row.get(19)?,
                    timed_segments: serde_json::from_str::<Vec<super::models::TimedSegment>>(
                        row.get::<_, String>(20)?.as_str(),
                    )
                    .unwrap_or_default(),
                    delivery_status: row.get(21)?,
                })
            })
            .map_err(|e| format!("failed to query history: {e}"))?
            .collect::<SqlResult<Vec<_>>>()
            .map_err(|e| format!("failed to collect history: {e}"))?;

        Ok(entries)
    }

    pub fn get_latest_successful_transcription(
        &self,
    ) -> Result<Option<LatestTranscriptionText>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, final_text FROM transcription_history \
                 WHERE status = 'success' ORDER BY created_at DESC, rowid DESC",
            )
            .map_err(|error| {
                format!("failed to prepare latest successful transcription query: {error}")
            })?;
        let mut rows = stmt
            .query([])
            .map_err(|error| format!("failed to query latest successful transcription: {error}"))?;

        while let Some(row) = rows
            .next()
            .map_err(|error| format!("failed to read latest successful transcription: {error}"))?
        {
            let id = row.get(0).map_err(|error| {
                format!("failed to read latest successful transcription id: {error}")
            })?;
            let final_text: String = row.get(1).map_err(|error| {
                format!("failed to read latest successful transcription text: {error}")
            })?;
            if !final_text.trim().is_empty() {
                return Ok(Some(LatestTranscriptionText { id, final_text }));
            }
        }

        Ok(None)
    }

    pub fn delete_entry(&self, id: &str) -> Result<(), String> {
        let audio_path = self.get_audio_path(id)?;
        if let Some(path) = audio_path.as_deref() {
            Self::remove_audio_file(path)?;
        }

        let mut conn = self.conn.lock();
        let tx = conn
            .transaction()
            .map_err(|e| format!("failed to start history deletion: {e}"))?;
        tx.execute(
            "DELETE FROM transcription_history WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("failed to delete history entry: {e}"))?;
        if let Some(path) = audio_path {
            tx.execute("DELETE FROM retained_audio WHERE path = ?1", params![path])
                .map_err(|e| format!("failed to delete retained audio record: {e}"))?;
        }
        tx.commit()
            .map_err(|e| format!("failed to commit history deletion: {e}"))?;
        Ok(())
    }

    /// Get a single entry by ID.
    pub fn get_entry(&self, id: &str) -> Result<Option<TranscriptionEntry>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, created_at, raw_text, final_text, stt_engine, \
                 stt_model, language, audio_duration_ms, stt_duration_ms, polish_duration_ms, \
                 total_duration_ms, polish_applied, polish_engine, is_cloud, audio_path, status, error, \
                 source_kind, source_path, translation_target, timed_segments, delivery_status \
                 FROM transcription_history WHERE id = ?1",
            )
            .map_err(|e| format!("failed to prepare query: {e}"))?;

        let result = stmt.query_row(params![id], |row| {
            Ok(TranscriptionEntry {
                id: row.get(0)?,
                created_at: row.get(1)?,
                raw_text: row.get(2)?,
                final_text: row.get(3)?,
                stt_engine: row.get(4)?,
                stt_model: row.get(5)?,
                language: row.get(6)?,
                audio_duration_ms: row.get(7)?,
                stt_duration_ms: row.get(8)?,
                polish_duration_ms: row.get(9)?,
                total_duration_ms: row.get(10)?,
                polish_applied: row.get::<_, i32>(11)? != 0,
                polish_engine: row.get(12)?,
                is_cloud: row.get::<_, i32>(13)? != 0,
                audio_path: row.get(14)?,
                status: row.get::<_, String>(15)?,
                error: row.get(16)?,
                source_kind: row.get(17)?,
                source_path: row.get(18)?,
                translation_target: row.get(19)?,
                timed_segments: serde_json::from_str::<Vec<super::models::TimedSegment>>(
                    row.get::<_, String>(20)?.as_str(),
                )
                .unwrap_or_default(),
                delivery_status: row.get(21)?,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("failed to get entry: {e}")),
        }
    }

    /// Get just the audio_path for an entry.
    pub fn get_audio_path(&self, id: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock();
        let result: Result<Option<String>, rusqlite::Error> = conn.query_row(
            "SELECT audio_path FROM transcription_history WHERE id = ?1",
            params![id],
            |row| row.get(0),
        );

        match result {
            Ok(path) => Ok(path),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("failed to get audio path: {e}")),
        }
    }

    /// Update an entry after successful retry.
    pub fn update_entry(&self, id: &str, updates: EntryUpdates) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE transcription_history SET \
             raw_text = ?1, final_text = ?2, stt_engine = ?3, stt_model = ?4, \
             language = ?5, stt_duration_ms = ?6, polish_duration_ms = ?7, polish_applied = ?8, \
             polish_engine = ?9, is_cloud = ?10, status = 'success', error = NULL \
             WHERE id = ?11",
            params![
                updates.raw_text,
                updates.final_text,
                updates.stt_engine,
                updates.stt_model,
                updates.language,
                updates.stt_duration_ms,
                updates.polish_duration_ms,
                updates.polish_applied as i32,
                updates.polish_engine,
                updates.is_cloud as i32,
                id,
            ],
        )
        .map_err(|e| format!("failed to update entry: {e}"))?;
        Ok(())
    }

    pub fn update_workbench_entry(
        &self,
        id: &str,
        updates: WorkbenchEntryUpdates,
    ) -> Result<(), String> {
        let timed_segments = serde_json::to_string(&updates.timed_segments)
            .map_err(|error| format!("failed to serialize timed segments: {error}"))?;
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "UPDATE transcription_history SET \
                 raw_text = ?1, final_text = ?2, stt_engine = ?3, stt_model = ?4, \
                 language = ?5, audio_duration_ms = ?6, stt_duration_ms = ?7, \
                 polish_duration_ms = ?8, total_duration_ms = ?9, polish_applied = ?10, \
                 polish_engine = ?11, is_cloud = ?12, translation_target = ?13, \
                 timed_segments = ?14, status = 'success', error = NULL \
                 WHERE id = ?15",
                params![
                    updates.raw_text,
                    updates.final_text,
                    updates.stt_engine,
                    updates.stt_model,
                    updates.language,
                    updates.audio_duration_ms,
                    updates.stt_duration_ms,
                    updates.polish_duration_ms,
                    updates.total_duration_ms,
                    updates.polish_applied as i32,
                    updates.polish_engine,
                    updates.is_cloud as i32,
                    updates.translation_target,
                    timed_segments,
                    id,
                ],
            )
            .map_err(|error| format!("failed to update workbench entry: {error}"))?;
        if changed == 0 {
            return Err(format!("History entry not found: {id}"));
        }
        Ok(())
    }

    pub fn update_repolished_text(
        &self,
        id: &str,
        final_text: &str,
        polish_duration_ms: i64,
        polish_engine: &str,
        translation_target: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "UPDATE transcription_history SET final_text = ?1, polish_duration_ms = ?2, \
                 total_duration_ms = COALESCE(stt_duration_ms, 0) + ?2, polish_applied = 1, \
                 polish_engine = ?3, translation_target = ?4, status = 'success', error = NULL \
                 WHERE id = ?5",
                params![
                    final_text,
                    polish_duration_ms,
                    polish_engine,
                    translation_target,
                    id
                ],
            )
            .map_err(|error| format!("failed to update polished history text: {error}"))?;
        if changed == 0 {
            return Err(format!("History entry not found: {id}"));
        }
        Ok(())
    }

    pub fn update_delivery_status(&self, id: &str, status: &str) -> Result<(), String> {
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "UPDATE transcription_history SET delivery_status = ?1 WHERE id = ?2",
                params![status, id],
            )
            .map_err(|error| format!("failed to update delivery status: {error}"))?;
        if changed == 0 {
            return Err(format!("History entry not found: {id}"));
        }
        Ok(())
    }

    /// Mark an entry as failed.
    pub fn mark_error(&self, id: &str, error: &str) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE transcription_history SET status = 'error', error = ?1 WHERE id = ?2",
            params![error, id],
        )
        .map_err(|e| format!("failed to mark entry as error: {e}"))?;
        Ok(())
    }

    pub fn clear_all(&self) -> Result<(), String> {
        let paths = self.retained_audio_paths(None)?;
        for path in paths {
            Self::remove_audio_file(&path)?;
            self.forget_audio_asset(&path)?;
        }

        let mut conn = self.conn.lock();
        let tx = conn
            .transaction()
            .map_err(|e| format!("failed to start history clear: {e}"))?;
        tx.execute("DELETE FROM transcription_history", [])
            .map_err(|e| format!("failed to clear history: {e}"))?;
        tx.execute("DELETE FROM retained_audio", [])
            .map_err(|e| format!("failed to clear retained audio: {e}"))?;
        tx.commit()
            .map_err(|e| format!("failed to commit history clear: {e}"))?;
        Ok(())
    }

    pub fn get_count(&self, filter: &HistoryFilter) -> Result<i64, String> {
        let mut sql = String::from("SELECT COUNT(*) FROM transcription_history WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref search) = filter.search {
            sql.push_str(&format!(" AND final_text LIKE ?{param_idx}"));
            param_values.push(Box::new(format!("%{search}%")));
            param_idx += 1;
        }

        if let Some(ref engine) = filter.engine {
            if engine == "local" {
                sql.push_str(" AND is_cloud = 0");
            } else if engine == "cloud" {
                sql.push_str(" AND is_cloud = 1");
            } else {
                sql.push_str(&format!(" AND stt_engine = ?{param_idx}"));
                param_values.push(Box::new(engine.clone()));
                param_idx += 1;
            }
        }

        if let Some(ref status) = filter.status {
            sql.push_str(&format!(" AND status = ?{param_idx}"));
            param_values.push(Box::new(status.clone()));
            param_idx += 1;
        }

        if let Some(date_from) = filter.date_from {
            sql.push_str(&format!(" AND created_at >= ?{param_idx}"));
            param_values.push(Box::new(date_from));
            param_idx += 1;
        }

        if let Some(date_to) = filter.date_to {
            sql.push_str(&format!(" AND created_at <= ?{param_idx}"));
            param_values.push(Box::new(date_to));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let conn = self.conn.lock();
        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(|e| format!("failed to count history: {e}"))?;

        Ok(count)
    }

    pub fn get_history_statistics(
        &self,
        period: StatisticsPeriod,
    ) -> Result<HistoryStatistics, String> {
        self.get_history_statistics_at(period, chrono::Utc::now().timestamp_millis())
    }

    fn get_history_statistics_at(
        &self,
        period: StatisticsPeriod,
        now_ms: i64,
    ) -> Result<HistoryStatistics, String> {
        use chrono::{Duration, Local, TimeZone};

        let now = Local.timestamp_millis_opt(now_ms).single().ok_or_else(|| {
            format!("statistics end time is outside the supported range: {now_ms}")
        })?;
        let today = now.date_naive();
        let range_start_date = match period {
            StatisticsPeriod::SevenDays => Some(today - Duration::days(6)),
            StatisticsPeriod::ThirtyDays => Some(today - Duration::days(29)),
            StatisticsPeriod::All => None,
        };
        let requested_range_start_ms = range_start_date
            .map(|date| {
                date.and_hms_opt(0, 0, 0)
                    .ok_or_else(|| format!("failed to build local statistics date: {date}"))?
                    .and_local_timezone(Local)
                    .earliest()
                    .map(|date_time| date_time.timestamp_millis())
                    .ok_or_else(|| format!("local statistics date has no valid midnight: {date}"))
            })
            .transpose()?;

        let mut trend = BTreeMap::<chrono::NaiveDate, DailyStatistics>::new();
        if let Some(start) = range_start_date {
            let mut date = start;
            while date <= today {
                trend.insert(date, Self::empty_daily_statistics(date));
                date = date
                    .succ_opt()
                    .ok_or_else(|| "statistics date range exceeds supported dates".to_string())?;
            }
        }

        let conn = self.conn.lock();
        let mut statement = conn
            .prepare(
                "SELECT created_at, final_text, audio_duration_ms, is_cloud \
                 FROM transcription_history \
                 WHERE status = 'success' AND source_kind = 'recording' \
                   AND created_at <= ?1 AND (?2 IS NULL OR created_at >= ?2) \
                 ORDER BY created_at ASC, rowid ASC",
            )
            .map_err(|error| format!("failed to prepare history statistics query: {error}"))?;
        let rows = statement
            .query_map(params![now_ms, requested_range_start_ms], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, i32>(3)? != 0,
                ))
            })
            .map_err(|error| format!("failed to query history statistics: {error}"))?;

        let mut statistics = HistoryStatistics {
            period,
            range_start_ms: requested_range_start_ms,
            range_end_ms: now_ms,
            word_count: 0,
            dictation_count: 0,
            audio_duration_ms: 0,
            active_days: 0,
            local_dictation_count: 0,
            cloud_dictation_count: 0,
            trend: Vec::new(),
        };

        for row in rows {
            let (created_at, final_text, audio_duration_ms, is_cloud) =
                row.map_err(|error| format!("failed to read history statistics row: {error}"))?;
            let created_date = Local
                .timestamp_millis_opt(created_at)
                .single()
                .ok_or_else(|| {
                    format!("history entry time is outside the supported range: {created_at}")
                })?
                .date_naive();
            let words = u64::try_from(final_text.unicode_words().count()).unwrap_or(u64::MAX);
            let audio_ms = audio_duration_ms
                .and_then(|duration| u64::try_from(duration).ok())
                .unwrap_or(0);
            let point = trend
                .entry(created_date)
                .or_insert_with(|| Self::empty_daily_statistics(created_date));

            point.word_count = point.word_count.saturating_add(words);
            point.dictation_count = point.dictation_count.saturating_add(1);
            point.audio_duration_ms = point.audio_duration_ms.saturating_add(audio_ms);
            statistics.word_count = statistics.word_count.saturating_add(words);
            statistics.dictation_count = statistics.dictation_count.saturating_add(1);
            statistics.audio_duration_ms = statistics.audio_duration_ms.saturating_add(audio_ms);
            if is_cloud {
                point.cloud_dictation_count = point.cloud_dictation_count.saturating_add(1);
                statistics.cloud_dictation_count =
                    statistics.cloud_dictation_count.saturating_add(1);
            } else {
                point.local_dictation_count = point.local_dictation_count.saturating_add(1);
                statistics.local_dictation_count =
                    statistics.local_dictation_count.saturating_add(1);
            }
            if statistics.range_start_ms.is_none() {
                statistics.range_start_ms = Some(created_at);
            }
        }
        drop(statement);
        drop(conn);

        statistics.active_days = u64::try_from(
            trend
                .values()
                .filter(|point| point.dictation_count > 0)
                .count(),
        )
        .unwrap_or(u64::MAX);
        statistics.trend = trend.into_values().collect();
        Ok(statistics)
    }

    fn empty_daily_statistics(date: chrono::NaiveDate) -> DailyStatistics {
        DailyStatistics {
            date: date.format("%Y-%m-%d").to_string(),
            word_count: 0,
            dictation_count: 0,
            audio_duration_ms: 0,
            local_dictation_count: 0,
            cloud_dictation_count: 0,
        }
    }

    pub fn register_audio_asset(&self, path: &str, created_at: i64) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO retained_audio (path, created_at) VALUES (?1, ?2)",
            params![path, created_at],
        )
        .map_err(|e| format!("failed to register retained audio: {e}"))?;
        Ok(())
    }

    pub fn cleanup_retention(
        &self,
        text_policy: RetentionPolicy,
        audio_policy: RetentionPolicy,
    ) -> Result<RetentionCleanupReport, String> {
        self.cleanup_retention_at(
            text_policy,
            audio_policy,
            chrono::Utc::now().timestamp_millis(),
        )
    }

    fn cleanup_retention_at(
        &self,
        text_policy: RetentionPolicy,
        audio_policy: RetentionPolicy,
        now_ms: i64,
    ) -> Result<RetentionCleanupReport, String> {
        let mut report = RetentionCleanupReport::default();
        let all_audio_paths = self.retained_audio_paths(None)?;
        for path in all_audio_paths {
            if !Path::new(&path).exists() {
                self.forget_audio_asset(&path)?;
                report.missing_audio_references_cleared += 1;
            }
        }

        let mut deletion_errors = Vec::new();
        if audio_policy != RetentionPolicy::Forever {
            let cutoff = Self::retention_cutoff(audio_policy, now_ms);
            for path in self.retained_audio_paths(cutoff)? {
                match Self::remove_audio_file(&path) {
                    Ok(()) => {
                        self.forget_audio_asset(&path)?;
                        report.audio_files_deleted += 1;
                    }
                    Err(error) => deletion_errors.push(error),
                }
            }
        }

        report.text_entries_deleted = self.cleanup_text_entries(text_policy, now_ms)?;

        if deletion_errors.is_empty() {
            Ok(report)
        } else {
            tracing::warn!(
                failures = deletion_errors.len(),
                "retention_audio_cleanup_incomplete"
            );
            Err(deletion_errors.join("; "))
        }
    }

    pub fn cleanup_orphaned_audio_files(&self, recordings_dir: &Path) -> Result<u64, String> {
        if !recordings_dir.exists() {
            return Ok(0);
        }

        let tracked = self
            .orphan_cleanup_protected_paths()?
            .into_iter()
            .collect::<HashSet<_>>();
        let mut deleted = 0_u64;
        let entries = std::fs::read_dir(recordings_dir)
            .map_err(|e| format!("failed to read recordings directory: {e}"))?;

        for entry in entries {
            let path = entry
                .map_err(|e| format!("failed to read recordings entry: {e}"))?
                .path();
            let is_wav = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"));
            if path.is_file() && is_wav && !tracked.contains(path.to_string_lossy().as_ref()) {
                Self::remove_audio_file(path.to_string_lossy().as_ref())?;
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    pub fn get_retention_status(&self) -> Result<RetentionStatus, String> {
        let conn = self.conn.lock();
        let text_entries = conn
            .query_row("SELECT COUNT(*) FROM transcription_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| format!("failed to count retained text entries: {e}"))?;
        let paths = {
            let mut statement = conn
                .prepare("SELECT path FROM retained_audio")
                .map_err(|e| format!("failed to prepare retained audio status: {e}"))?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| format!("failed to query retained audio status: {e}"))?;
            rows.collect::<SqlResult<Vec<_>>>()
                .map_err(|e| format!("failed to read retained audio status: {e}"))?
        };
        drop(conn);

        let mut audio_files = 0_u64;
        let mut audio_bytes = 0_u64;
        for path in paths {
            if let Ok(metadata) = std::fs::metadata(path) {
                if metadata.is_file() {
                    audio_files += 1;
                    audio_bytes = audio_bytes.saturating_add(metadata.len());
                }
            }
        }

        Ok(RetentionStatus {
            text_entries: u64::try_from(text_entries).unwrap_or(0),
            audio_files,
            audio_bytes,
        })
    }

    fn cleanup_text_entries(&self, policy: RetentionPolicy, now_ms: i64) -> Result<u64, String> {
        if policy == RetentionPolicy::Forever {
            return Ok(0);
        }

        let conn = self.conn.lock();
        let deleted = match Self::retention_cutoff(policy, now_ms) {
            Some(cutoff) => conn.execute(
                "DELETE FROM transcription_history WHERE created_at < ?1",
                params![cutoff],
            ),
            None => conn.execute("DELETE FROM transcription_history", []),
        }
        .map_err(|e| format!("failed to cleanup retained text: {e}"))?;
        Ok(deleted as u64)
    }

    fn retention_cutoff(policy: RetentionPolicy, now_ms: i64) -> Option<i64> {
        match policy.max_age_days() {
            Some(0) => None,
            Some(days) => Some(
                now_ms.saturating_sub(
                    i64::try_from(days)
                        .unwrap_or(i64::MAX)
                        .saturating_mul(24 * 60 * 60 * 1_000),
                ),
            ),
            None => None,
        }
    }

    fn retained_audio_paths(&self, cutoff_ms: Option<i64>) -> Result<Vec<String>, String> {
        let conn = self.conn.lock();
        let query = match cutoff_ms {
            Some(_) => "SELECT path FROM retained_audio WHERE created_at < ?1",
            None => "SELECT path FROM retained_audio",
        };
        let mut statement = conn
            .prepare(query)
            .map_err(|e| format!("failed to prepare retained audio query: {e}"))?;
        let paths = match cutoff_ms {
            Some(cutoff) => statement
                .query_map(params![cutoff], |row| row.get::<_, String>(0))
                .map_err(|e| format!("failed to query retained audio: {e}"))?
                .collect::<SqlResult<Vec<_>>>(),
            None => statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| format!("failed to query retained audio: {e}"))?
                .collect::<SqlResult<Vec<_>>>(),
        };
        paths.map_err(|e| format!("failed to read retained audio: {e}"))
    }

    fn orphan_cleanup_protected_paths(&self) -> Result<Vec<String>, String> {
        let conn = self.conn.lock();
        let mut statement = conn
            .prepare(
                "SELECT path FROM retained_audio \
                 UNION \
                 SELECT source_path FROM transcription_history \
                 WHERE source_path IS NOT NULL AND source_path <> ''",
            )
            .map_err(|e| format!("failed to prepare protected audio path query: {e}"))?;
        let paths = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("failed to query protected audio paths: {e}"))?
            .collect::<SqlResult<Vec<_>>>();
        paths.map_err(|e| format!("failed to read protected audio paths: {e}"))
    }

    fn forget_audio_asset(&self, path: &str) -> Result<(), String> {
        let mut conn = self.conn.lock();
        let tx = conn
            .transaction()
            .map_err(|e| format!("failed to start retained audio cleanup: {e}"))?;
        tx.execute(
            "UPDATE transcription_history SET audio_path = NULL WHERE audio_path = ?1",
            params![path],
        )
        .map_err(|e| format!("failed to clear history audio reference: {e}"))?;
        tx.execute("DELETE FROM retained_audio WHERE path = ?1", params![path])
            .map_err(|e| format!("failed to delete retained audio record: {e}"))?;
        tx.commit()
            .map_err(|e| format!("failed to commit retained audio cleanup: {e}"))?;
        Ok(())
    }

    fn remove_audio_file(path: &str) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("failed to delete audio file '{path}': {error}")),
        }
    }

    #[cfg(test)]
    pub(crate) fn retained_audio_count(&self) -> Result<u64, String> {
        let conn = self.conn.lock();
        let count = conn
            .query_row("SELECT COUNT(*) FROM retained_audio", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|e| format!("failed to count retained audio: {e}"))?;
        Ok(u64::try_from(count).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::RetentionPolicy;
    use chrono::{Duration, Local};
    use std::fs;

    fn test_store() -> HistoryStore {
        HistoryStore::from_connection(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn timestamp_for_day_offset(days_ago: i64, hour: u32) -> i64 {
        let date = Local::now().date_naive() - Duration::days(days_ago);
        date.and_hms_opt(hour, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    struct TestEntry<'a> {
        id: &'a str,
        created_at: i64,
        final_text: &'a str,
        audio_duration_ms: Option<i64>,
        stt_duration_ms: Option<i64>,
        polish_applied: bool,
        is_cloud: bool,
        stt_engine: &'static str,
        source_kind: &'static str,
    }

    impl<'a> TestEntry<'a> {
        fn new(id: &'a str, created_at: i64, final_text: &'a str) -> Self {
            Self {
                id,
                created_at,
                final_text,
                audio_duration_ms: None,
                stt_duration_ms: None,
                polish_applied: false,
                is_cloud: false,
                stt_engine: "Whisper",
                source_kind: "recording",
            }
        }

        fn with_timings(mut self, audio_ms: i64, stt_ms: i64) -> Self {
            self.audio_duration_ms = Some(audio_ms);
            self.stt_duration_ms = Some(stt_ms);
            self
        }

        fn from_file(mut self) -> Self {
            self.source_kind = "file";
            self
        }

        fn from_cloud(mut self) -> Self {
            self.is_cloud = true;
            self
        }
    }

    fn insert_entry(store: &HistoryStore, entry: TestEntry<'_>) {
        let TestEntry {
            id,
            created_at,
            final_text,
            audio_duration_ms,
            stt_duration_ms,
            polish_applied,
            is_cloud,
            stt_engine,
            source_kind,
        } = entry;
        let conn = store.conn.lock();
        conn.execute(
            "INSERT INTO transcription_history \
             (id, created_at, raw_text, final_text, stt_engine, stt_model, language, \
              audio_duration_ms, stt_duration_ms, polish_duration_ms, total_duration_ms, \
              polish_applied, polish_engine, is_cloud, source_kind) \
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7, NULL, NULL, ?8, NULL, ?9, ?10)",
            params![
                id,
                created_at,
                final_text,
                final_text,
                stt_engine,
                audio_duration_ms,
                stt_duration_ms,
                polish_applied as i32,
                is_cloud as i32,
                source_kind,
            ],
        )
        .unwrap();
    }

    #[test]
    fn statistics_count_only_successful_microphone_dictations() {
        let store = test_store();
        let now = timestamp_for_day_offset(0, 18);
        insert_entry(
            &store,
            TestEntry::new("local", timestamp_for_day_offset(0, 9), "Hello, world!")
                .with_timings(1_500, 100),
        );
        insert_entry(
            &store,
            TestEntry::new("cloud", timestamp_for_day_offset(0, 10), "bonjour à tous").from_cloud(),
        );
        insert_entry(
            &store,
            TestEntry::new("file", timestamp_for_day_offset(0, 11), "imported words")
                .with_timings(99_000, 200)
                .from_file(),
        );
        insert_entry(
            &store,
            TestEntry::new("failed", timestamp_for_day_offset(0, 12), "failed words")
                .with_timings(5_000, 300),
        );
        store.mark_error("failed", "network error").unwrap();

        let stats = store
            .get_history_statistics_at(super::super::models::StatisticsPeriod::SevenDays, now)
            .unwrap();

        assert_eq!(stats.word_count, 5);
        assert_eq!(stats.dictation_count, 2);
        assert_eq!(stats.audio_duration_ms, 1_500);
        assert_eq!(stats.active_days, 1);
        assert_eq!(stats.local_dictation_count, 1);
        assert_eq!(stats.cloud_dictation_count, 1);
    }

    #[test]
    fn seven_day_statistics_use_local_calendar_boundaries_and_dense_trend() {
        let store = test_store();
        let now = timestamp_for_day_offset(0, 18);
        insert_entry(
            &store,
            TestEntry::new("today", timestamp_for_day_offset(0, 9), "today"),
        );
        insert_entry(
            &store,
            TestEntry::new("six-days", timestamp_for_day_offset(6, 9), "included"),
        );
        insert_entry(
            &store,
            TestEntry::new("seven-days", timestamp_for_day_offset(7, 9), "excluded"),
        );

        let stats = store
            .get_history_statistics_at(super::super::models::StatisticsPeriod::SevenDays, now)
            .unwrap();

        assert_eq!(stats.dictation_count, 2);
        assert_eq!(stats.active_days, 2);
        assert_eq!(stats.trend.len(), 7);
        assert_eq!(stats.trend.first().unwrap().dictation_count, 1);
        assert_eq!(stats.trend.last().unwrap().dictation_count, 1);
        assert_eq!(
            stats
                .trend
                .iter()
                .filter(|point| point.dictation_count == 0)
                .count(),
            5
        );
    }

    #[test]
    fn all_time_statistics_return_sparse_daily_trend() {
        let store = test_store();
        let now = timestamp_for_day_offset(0, 18);
        insert_entry(
            &store,
            TestEntry::new("old", timestamp_for_day_offset(400, 9), "old entry"),
        );
        insert_entry(
            &store,
            TestEntry::new("today", timestamp_for_day_offset(0, 9), "new entry"),
        );

        let stats = store
            .get_history_statistics_at(super::super::models::StatisticsPeriod::All, now)
            .unwrap();

        assert_eq!(stats.dictation_count, 2);
        assert_eq!(stats.trend.len(), 2);
        assert!(stats.trend.iter().all(|point| point.dictation_count == 1));
        assert_eq!(stats.range_start_ms, Some(timestamp_for_day_offset(400, 9)));
    }

    #[test]
    fn empty_thirty_day_statistics_return_zero_filled_local_dates() {
        let store = test_store();
        let now = timestamp_for_day_offset(0, 18);

        let stats = store
            .get_history_statistics_at(super::super::models::StatisticsPeriod::ThirtyDays, now)
            .unwrap();

        assert!(stats.range_start_ms.is_some());
        assert_eq!(stats.range_end_ms, now);
        assert_eq!(stats.word_count, 0);
        assert_eq!(stats.dictation_count, 0);
        assert_eq!(stats.audio_duration_ms, 0);
        assert_eq!(stats.active_days, 0);
        assert_eq!(stats.trend.len(), 30);
        assert!(stats.trend.iter().all(|point| point.word_count == 0
            && point.dictation_count == 0
            && point.audio_duration_ms == 0));
    }

    #[test]
    fn latest_successful_transcription_skips_newer_failed_and_blank_entries() {
        let store = test_store();
        insert_entry(&store, TestEntry::new("older", 1_000, "older result"));
        insert_entry(
            &store,
            TestEntry::new("latest-usable", 2_000, "latest result"),
        );
        insert_entry(&store, TestEntry::new("blank", 3_000, "   \n"));
        insert_entry(&store, TestEntry::new("failed", 4_000, "failed result"));
        store
            .mark_error("failed", "provider rejected audio")
            .unwrap();

        let latest = store
            .get_latest_successful_transcription()
            .unwrap()
            .expect("a usable successful transcription should be selected");

        assert_eq!(latest.id, "latest-usable");
        assert_eq!(latest.final_text, "latest result");
    }

    #[test]
    fn latest_successful_transcription_uses_insertion_order_to_break_timestamp_ties() {
        let store = test_store();
        insert_entry(&store, TestEntry::new("first", 1_000, "first result"));
        insert_entry(&store, TestEntry::new("second", 1_000, "second result"));

        let latest = store
            .get_latest_successful_transcription()
            .unwrap()
            .expect("a successful transcription should be selected");

        assert_eq!(latest.id, "second");
        assert_eq!(latest.final_text, "second result");
    }

    #[test]
    fn latest_successful_transcription_returns_none_when_history_has_no_usable_text() {
        let store = test_store();
        insert_entry(&store, TestEntry::new("blank", 1_000, "\t"));
        insert_entry(&store, TestEntry::new("failed", 2_000, "failed result"));
        store.mark_error("failed", "model unavailable").unwrap();

        assert!(store
            .get_latest_successful_transcription()
            .unwrap()
            .is_none());
    }

    /// Helper to insert an entry with error state for retry tests
    fn insert_error_entry(
        store: &HistoryStore,
        id: &str,
        created_at: i64,
        audio_path: Option<&str>,
    ) {
        let conn = store.conn.lock();
        conn.execute(
            "INSERT INTO transcription_history \
             (id, created_at, raw_text, final_text, stt_engine, audio_path, status, error) \
             VALUES (?1, ?2, '', '', 'Whisper', ?3, 'error', 'Initial failure')",
            params![id, created_at, audio_path],
        )
        .unwrap();
    }

    #[test]
    fn mark_error_sets_status_and_error_message() {
        let store = test_store();
        insert_entry(
            &store,
            TestEntry::new("entry-1", timestamp_for_day_offset(0, 10), "original text")
                .with_timings(10_000, 500),
        );

        // Mark as error
        store
            .mark_error("entry-1", "Transcription failed: empty result")
            .unwrap();

        // Verify status changed to error
        let entry = store.get_entry("entry-1").unwrap().unwrap();
        assert_eq!(entry.status, "error");
        assert_eq!(
            entry.error,
            Some("Transcription failed: empty result".to_string())
        );
    }

    #[test]
    fn update_entry_clears_error_and_sets_success() {
        let store = test_store();
        insert_error_entry(
            &store,
            "entry-1",
            timestamp_for_day_offset(0, 10),
            Some("/tmp/audio.wav"),
        );

        // Verify initial state
        let before = store.get_entry("entry-1").unwrap().unwrap();
        assert_eq!(before.status, "error");
        assert_eq!(before.error, Some("Initial failure".to_string()));

        // Update after successful retry
        let updates = EntryUpdates {
            raw_text: "retry result".to_string(),
            final_text: "Retry Result".to_string(),
            stt_engine: "Whisper".to_string(),
            stt_model: Some("base".to_string()),
            language: Some("en-US".to_string()),
            stt_duration_ms: Some(450),
            polish_duration_ms: Some(100),
            polish_applied: true,
            polish_engine: Some("cloud".to_string()),
            is_cloud: false,
        };
        store.update_entry("entry-1", updates).unwrap();

        // Verify status changed to success
        let after = store.get_entry("entry-1").unwrap().unwrap();
        assert_eq!(after.status, "success");
        assert_eq!(after.error, None);
        assert_eq!(after.raw_text, "retry result");
        assert_eq!(after.final_text, "Retry Result");
        assert_eq!(after.stt_duration_ms, Some(450));
    }

    #[test]
    fn get_entry_returns_none_for_nonexistent_id() {
        let store = test_store();
        let result = store.get_entry("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_entry_includes_audio_path_for_retry() {
        let store = test_store();
        insert_error_entry(
            &store,
            "entry-1",
            timestamp_for_day_offset(0, 10),
            Some("/path/to/audio.wav"),
        );

        let entry = store.get_entry("entry-1").unwrap().unwrap();
        assert_eq!(entry.audio_path, Some("/path/to/audio.wav".to_string()));
        assert_eq!(entry.status, "error");
    }

    #[test]
    fn migration_from_v2_registers_existing_audio_paths_and_reaches_latest_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            format!(
                "{CREATE_TABLE_SQL};
                 {CREATE_INDEX_SQL};
                 INSERT INTO transcription_history
                    (id, created_at, raw_text, final_text, stt_engine, audio_path)
                 VALUES ('legacy', 1234, 'raw', 'final', 'Whisper', '/tmp/legacy.wav');
                 PRAGMA user_version = 2;"
            )
            .as_str(),
        )
        .unwrap();

        let store = HistoryStore::from_connection(conn).unwrap();
        let conn = store.conn.lock();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        let registered: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM retained_audio WHERE path = '/tmp/legacy.wav' AND created_at = 1234",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, 4);
        assert_eq!(registered, 1);
    }

    #[test]
    fn migration_v4_adds_workbench_metadata_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            format!(
                "{CREATE_TABLE_SQL};
                 {CREATE_INDEX_SQL};
                 PRAGMA user_version = 3;"
            )
            .as_str(),
        )
        .unwrap();

        let store = HistoryStore::from_connection(conn).unwrap();
        let conn = store.conn.lock();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        let columns = [
            "source_kind",
            "source_path",
            "translation_target",
            "timed_segments",
            "delivery_status",
        ];

        assert_eq!(version, 4);
        for column in columns {
            let present: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('transcription_history') WHERE name = ?1",
                    params![column],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(present, 1, "missing workbench column: {column}");
        }
    }

    #[test]
    fn text_never_removes_rows_but_preserves_retained_audio() {
        let store = test_store();
        let audio = tempfile::NamedTempFile::new().unwrap();
        let path = audio.path().display().to_string();
        insert_error_entry(&store, "entry-1", 1_000, Some(&path));
        store.register_audio_asset(&path, 1_000).unwrap();

        let report = store
            .cleanup_retention_at(RetentionPolicy::Never, RetentionPolicy::Forever, 10_000)
            .unwrap();

        assert_eq!(report.text_entries_deleted, 1);
        assert_eq!(report.audio_files_deleted, 0);
        assert!(audio.path().exists());
        assert_eq!(store.retained_audio_count().unwrap(), 1);
        assert!(store.get_entry("entry-1").unwrap().is_none());
    }

    #[test]
    fn audio_expiry_deletes_file_and_clears_history_reference() {
        let store = test_store();
        let audio = tempfile::NamedTempFile::new().unwrap();
        let path = audio.path().display().to_string();
        let now_ms = 40_i64 * 24 * 60 * 60 * 1_000;
        insert_error_entry(&store, "entry-1", 1_000, Some(&path));
        store.register_audio_asset(&path, 1_000).unwrap();

        let report = store
            .cleanup_retention_at(RetentionPolicy::Forever, RetentionPolicy::Days30, now_ms)
            .unwrap();

        assert_eq!(report.audio_files_deleted, 1);
        assert!(!audio.path().exists());
        assert_eq!(store.retained_audio_count().unwrap(), 0);
        assert_eq!(
            store.get_entry("entry-1").unwrap().unwrap().audio_path,
            None
        );
    }

    #[test]
    fn failed_audio_deletion_keeps_database_references_for_retry() {
        let store = test_store();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().display().to_string();
        insert_error_entry(&store, "entry-1", 1_000, Some(&path));
        store.register_audio_asset(&path, 1_000).unwrap();

        let result =
            store.cleanup_retention_at(RetentionPolicy::Forever, RetentionPolicy::Never, 10_000);

        assert!(result.is_err());
        assert_eq!(store.retained_audio_count().unwrap(), 1);
        assert_eq!(
            store.get_entry("entry-1").unwrap().unwrap().audio_path,
            Some(path)
        );
    }

    #[test]
    fn orphan_cleanup_deletes_only_unregistered_wav_files() {
        let store = test_store();
        let directory = tempfile::tempdir().unwrap();
        let tracked = directory.path().join("tracked.wav");
        let orphan = directory.path().join("orphan.wav");
        let unrelated = directory.path().join("note.txt");
        fs::write(&tracked, b"tracked").unwrap();
        fs::write(&orphan, b"orphan").unwrap();
        fs::write(&unrelated, b"note").unwrap();
        store
            .register_audio_asset(&tracked.display().to_string(), 1_000)
            .unwrap();

        let deleted = store
            .cleanup_orphaned_audio_files(directory.path())
            .unwrap();

        assert_eq!(deleted, 1);
        assert!(tracked.exists());
        assert!(!orphan.exists());
        assert!(unrelated.exists());
    }

    #[test]
    fn orphan_cleanup_preserves_imported_source_inside_recordings_directory() {
        let store = test_store();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("user-import.wav");
        let orphan = directory.path().join("orphan.wav");
        fs::write(&source, b"user source").unwrap();
        fs::write(&orphan, b"orphan").unwrap();
        store
            .insert(NewTranscriptionEntry {
                raw_text: "raw".to_string(),
                final_text: "final".to_string(),
                stt_engine: "whisper".to_string(),
                stt_model: Some("model".to_string()),
                language: Some("en".to_string()),
                audio_duration_ms: Some(1_000),
                stt_duration_ms: Some(100),
                polish_duration_ms: None,
                total_duration_ms: Some(100),
                polish_applied: false,
                polish_engine: None,
                is_cloud: false,
                audio_path: None,
                status: "success".to_string(),
                error: None,
                source_kind: "file".to_string(),
                source_path: Some(source.to_string_lossy().into_owned()),
                translation_target: None,
                timed_segments: Vec::new(),
                delivery_status: "not_delivered".to_string(),
            })
            .unwrap();

        let deleted = store
            .cleanup_orphaned_audio_files(directory.path())
            .unwrap();

        assert_eq!(deleted, 1);
        assert!(source.exists());
        assert!(!orphan.exists());
    }

    #[test]
    fn retention_status_reports_existing_local_text_and_audio() {
        let store = test_store();
        insert_entry(&store, TestEntry::new("entry-1", 1_000, "retained text"));
        let directory = tempfile::tempdir().unwrap();
        let audio_path = directory.path().join("retained.wav");
        fs::write(&audio_path, b"12345678").unwrap();
        store
            .register_audio_asset(audio_path.to_string_lossy().as_ref(), 1_000)
            .unwrap();

        let status = store.get_retention_status().unwrap();

        assert_eq!(status.text_entries, 1);
        assert_eq!(status.audio_files, 1);
        assert_eq!(status.audio_bytes, 8);
    }

    #[test]
    fn explicit_history_clear_removes_registered_audio() {
        let store = test_store();
        let audio = tempfile::NamedTempFile::new().unwrap();
        let path = audio.path().display().to_string();
        insert_error_entry(&store, "entry-1", 1_000, Some(&path));
        store.register_audio_asset(&path, 1_000).unwrap();

        store.clear_all().unwrap();

        assert!(!audio.path().exists());
        assert!(store.get_entry("entry-1").unwrap().is_none());
        assert_eq!(store.retained_audio_count().unwrap(), 0);
    }

    #[test]
    fn deleting_file_import_history_never_deletes_user_source() {
        let store = test_store();
        let source = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let source_path = source.path().display().to_string();
        let id = store
            .insert(NewTranscriptionEntry {
                raw_text: "raw".to_string(),
                final_text: "final".to_string(),
                stt_engine: "whisper".to_string(),
                stt_model: Some("model".to_string()),
                language: Some("en".to_string()),
                audio_duration_ms: Some(1_000),
                stt_duration_ms: Some(100),
                polish_duration_ms: None,
                total_duration_ms: Some(100),
                polish_applied: false,
                polish_engine: None,
                is_cloud: false,
                audio_path: None,
                status: "success".to_string(),
                error: None,
                source_kind: "file".to_string(),
                source_path: Some(source_path.clone()),
                translation_target: None,
                timed_segments: Vec::new(),
                delivery_status: "not_delivered".to_string(),
            })
            .unwrap();

        store.delete_entry(&id).unwrap();

        assert!(source.path().is_file());
        assert!(store.get_entry(&id).unwrap().is_none());
        assert_eq!(store.retained_audio_count().unwrap(), 0);
    }

    #[test]
    fn workbench_metadata_round_trips_through_history() {
        let store = test_store();
        let id = store
            .insert(NewTranscriptionEntry {
                raw_text: "bonjour".to_string(),
                final_text: "hello".to_string(),
                stt_engine: "whisper".to_string(),
                stt_model: Some("model".to_string()),
                language: Some("fr".to_string()),
                audio_duration_ms: Some(2_000),
                stt_duration_ms: Some(200),
                polish_duration_ms: Some(50),
                total_duration_ms: Some(250),
                polish_applied: true,
                polish_engine: Some("qwen".to_string()),
                is_cloud: false,
                audio_path: None,
                status: "success".to_string(),
                error: None,
                source_kind: "file".to_string(),
                source_path: Some("/tmp/source.wav".to_string()),
                translation_target: Some("en".to_string()),
                timed_segments: vec![super::super::models::TimedSegment {
                    start_ms: 0,
                    end_ms: 2_000,
                    text: "hello".to_string(),
                }],
                delivery_status: "inserted_keyboard".to_string(),
            })
            .unwrap();

        let entry = store.get_entry(&id).unwrap().unwrap();

        assert_eq!(entry.source_kind, "file");
        assert_eq!(entry.source_path.as_deref(), Some("/tmp/source.wav"));
        assert_eq!(entry.translation_target.as_deref(), Some("en"));
        assert_eq!(entry.timed_segments.len(), 1);
        assert_eq!(entry.timed_segments[0].end_ms, 2_000);
        assert_eq!(entry.delivery_status, "inserted_keyboard");
    }
}
