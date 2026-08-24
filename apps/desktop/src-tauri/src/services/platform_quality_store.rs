use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use rusqlite::{params, params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use super::{summarize_quality, QualityEvent, QualityEventKind, QualitySummary};

const CREATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS quality_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    application_id TEXT,
    stt_ms INTEGER,
    polish_ms INTEGER,
    total_ms INTEGER,
    is_cloud INTEGER,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_quality_created_at ON quality_events(created_at_ms);
CREATE INDEX IF NOT EXISTS idx_quality_application ON quality_events(application_id);
";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityQuery {
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub application_id: Option<String>,
    pub kind: Option<QualityEventKind>,
    pub is_cloud: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct QualityExport<'a> {
    schema_version: u8,
    exported_at_ms: i64,
    query: &'a QualityQuery,
    events: &'a [QualityEvent],
    summary: &'a QualitySummary,
}

pub struct QualityStore {
    connection: Mutex<Connection>,
}

impl QualityStore {
    pub fn new() -> Result<Self, String> {
        let path = crate::utils::AppPaths::data_dir().join("quality_metrics.db");
        Self::open(&path)
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create quality metrics directory: {error}"))?;
        }
        let connection = Connection::open(path)
            .map_err(|error| format!("Failed to open quality metrics database: {error}"))?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|error| format!("Failed to configure quality metrics database: {error}"))?;
        migrate(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn record(&self, event: &QualityEvent) -> Result<(), String> {
        let application_id = event
            .application_id
            .as_deref()
            .and_then(clean_application_id);
        let connection = self.connection.lock();
        connection
            .execute(
                "INSERT INTO quality_events
                 (kind, application_id, stt_ms, polish_ms, total_ms, is_cloud, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    kind_to_str(event.kind),
                    application_id,
                    optional_millis(event.stt_ms)?,
                    optional_millis(event.polish_ms)?,
                    optional_millis(event.total_ms)?,
                    event.is_cloud.map(i64::from),
                    event.created_at_ms,
                ],
            )
            .map_err(|error| format!("Failed to record quality event: {error}"))?;
        Ok(())
    }

    pub fn query(&self, query: &QualityQuery) -> Result<Vec<QualityEvent>, String> {
        let mut sql = String::from(
            "SELECT kind, application_id, stt_ms, polish_ms, total_ms, is_cloud, created_at_ms
             FROM quality_events WHERE 1=1",
        );
        let mut values = Vec::<rusqlite::types::Value>::new();
        if let Some(since_ms) = query.since_ms {
            sql.push_str(" AND created_at_ms >= ?");
            values.push(since_ms.into());
        }
        if let Some(until_ms) = query.until_ms {
            sql.push_str(" AND created_at_ms <= ?");
            values.push(until_ms.into());
        }
        if let Some(application_id) = query
            .application_id
            .as_deref()
            .and_then(clean_application_id)
        {
            sql.push_str(" AND application_id = ?");
            values.push(application_id.into());
        }
        if let Some(kind) = query.kind {
            sql.push_str(" AND kind = ?");
            values.push(kind_to_str(kind).to_string().into());
        }
        if let Some(is_cloud) = query.is_cloud {
            sql.push_str(" AND is_cloud = ?");
            values.push(i64::from(is_cloud).into());
        }
        sql.push_str(" ORDER BY created_at_ms ASC, id ASC");

        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| format!("Failed to prepare quality query: {error}"))?;
        let rows = statement
            .query_map(params_from_iter(values), |row| {
                let kind: String = row.get(0)?;
                Ok((
                    kind,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(|error| format!("Failed to query quality events: {error}"))?;

        let mut events = Vec::new();
        for row in rows {
            let (kind, application_id, stt_ms, polish_ms, total_ms, is_cloud, created_at_ms) =
                row.map_err(|error| format!("Failed to read quality event: {error}"))?;
            events.push(QualityEvent {
                kind: parse_kind(&kind)?,
                application_id,
                stt_ms: optional_unsigned(stt_ms, "stt_ms")?,
                polish_ms: optional_unsigned(polish_ms, "polish_ms")?,
                total_ms: optional_unsigned(total_ms, "total_ms")?,
                is_cloud: is_cloud.map(|value| value != 0),
                created_at_ms,
            });
        }
        Ok(events)
    }

    pub fn summary(&self, query: &QualityQuery) -> Result<QualitySummary, String> {
        self.query(query).map(|events| summarize_quality(&events))
    }

    pub fn export_json(&self, query: &QualityQuery) -> Result<String, String> {
        let events = self.query(query)?;
        let summary = summarize_quality(&events);
        serde_json::to_string_pretty(&QualityExport {
            schema_version: 1,
            exported_at_ms: chrono::Utc::now().timestamp_millis(),
            query,
            events: &events,
            summary: &summary,
        })
        .map_err(|error| format!("Failed to serialize quality metrics: {error}"))
    }

    pub fn export_to_file(
        &self,
        path: &Path,
        query: &QualityQuery,
        overwrite: bool,
    ) -> Result<PathBuf, String> {
        if path.exists() && path.is_dir() {
            return Err("Quality export path must be a file".to_string());
        }
        if path.exists() && !overwrite {
            return Err("Quality export destination already exists".to_string());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create quality export directory: {error}"))?;
        }
        std::fs::write(path, self.export_json(query)?)
            .map_err(|error| format!("Failed to write quality export: {error}"))?;
        Ok(path.to_path_buf())
    }

    pub fn clear(&self) -> Result<usize, String> {
        self.connection
            .lock()
            .execute("DELETE FROM quality_events", [])
            .map_err(|error| format!("Failed to clear quality metrics: {error}"))
    }
}

fn migrate(connection: &Connection) -> Result<(), String> {
    let version = connection
        .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
        .unwrap_or(0);
    if version == 0 {
        connection
            .execute_batch(&format!(
                "BEGIN; {CREATE_TABLE} PRAGMA user_version = 2; COMMIT;"
            ))
            .map_err(|error| format!("Quality metrics migration v2 failed: {error}"))?;
    } else if version < 2 {
        let has_source = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('quality_events') WHERE name='is_cloud'",
                [],
                |row| row.get::<_, u32>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_source {
            connection
                .execute("ALTER TABLE quality_events ADD COLUMN is_cloud INTEGER", [])
                .map_err(|error| format!("Quality metrics migration v2 failed: {error}"))?;
        }
        connection
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_quality_created_at ON quality_events(created_at_ms);
                 CREATE INDEX IF NOT EXISTS idx_quality_application ON quality_events(application_id);
                 PRAGMA user_version = 2;",
            )
            .map_err(|error| format!("Quality metrics migration v2 failed: {error}"))?;
    }
    Ok(())
}

fn kind_to_str(kind: QualityEventKind) -> &'static str {
    match kind {
        QualityEventKind::TranscriptionSuccess => "transcription_success",
        QualityEventKind::TranscriptionFailure => "transcription_failure",
        QualityEventKind::InjectionFailure => "injection_failure",
        QualityEventKind::Correction => "correction",
    }
}

fn parse_kind(value: &str) -> Result<QualityEventKind, String> {
    match value {
        "transcription_success" => Ok(QualityEventKind::TranscriptionSuccess),
        "transcription_failure" => Ok(QualityEventKind::TranscriptionFailure),
        "injection_failure" => Ok(QualityEventKind::InjectionFailure),
        "correction" => Ok(QualityEventKind::Correction),
        _ => Err(format!("Unknown quality event kind: {value}")),
    }
}

fn clean_application_id(value: &str) -> Option<String> {
    let value = value
        .chars()
        .filter(|character| !matches!(character, '\r' | '\n' | '\0'))
        .take(256)
        .collect::<String>();
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn optional_millis(value: Option<u64>) -> Result<Option<i64>, String> {
    value
        .map(|value| i64::try_from(value).map_err(|_| "Quality duration is too large".to_string()))
        .transpose()
}

fn optional_unsigned(value: Option<i64>, field: &str) -> Result<Option<u64>, String> {
    value
        .map(|value| {
            u64::try_from(value).map_err(|_| format!("Quality metric {field} cannot be negative"))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_persists_filters_summarizes_exports_and_clears_content_free_events() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quality.db");
        let store = QualityStore::open(&path).unwrap();
        let mut first = QualityEvent::success_with_source(Some("editor"), 90, 10, 110, false);
        first.created_at_ms = 1_000;
        let mut second = QualityEvent::injection_failure(Some("terminal"), 140);
        second.created_at_ms = 2_000;

        store.record(&first).unwrap();
        store.record(&second).unwrap();

        let events = store
            .query(&QualityQuery {
                since_ms: Some(1_500),
                until_ms: None,
                application_id: None,
                kind: Some(QualityEventKind::InjectionFailure),
                is_cloud: None,
            })
            .unwrap();
        assert_eq!(events, vec![second]);

        let summary = store.summary(&QualityQuery::default()).unwrap();
        assert_eq!(summary.total_transcriptions, 1);
        assert_eq!(summary.injection_failures, 1);

        let export = store.export_json(&QualityQuery::default()).unwrap();
        assert!(export.contains("\"application_id\": \"editor\""));
        assert!(!export.contains("raw_text"));
        assert!(!export.contains("final_text"));

        let export_path = directory.path().join("quality.json");
        std::fs::write(&export_path, "keep").unwrap();
        assert!(store
            .export_to_file(&export_path, &QualityQuery::default(), false)
            .unwrap_err()
            .contains("already exists"));
        store
            .export_to_file(&export_path, &QualityQuery::default(), true)
            .unwrap();
        assert!(!std::fs::read_to_string(&export_path)
            .unwrap()
            .contains("keep"));

        assert_eq!(store.clear().unwrap(), 2);
        assert!(store.query(&QualityQuery::default()).unwrap().is_empty());
    }

    #[test]
    fn migration_adds_cloud_source_to_an_existing_quality_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quality.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE quality_events (\
                    id INTEGER PRIMARY KEY AUTOINCREMENT,\
                    kind TEXT NOT NULL,\
                    application_id TEXT,\
                    stt_ms INTEGER,\
                    polish_ms INTEGER,\
                    total_ms INTEGER,\
                    created_at_ms INTEGER NOT NULL\
                );\
                PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);

        let store = QualityStore::open(&path).unwrap();
        store
            .record(&QualityEvent::transcription_failure(None, 120, true))
            .unwrap();
        let events = store.query(&QualityQuery::default()).unwrap();
        assert_eq!(events[0].is_cloud, Some(true));
    }
}
