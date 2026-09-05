use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimedSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Status of a transcription entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryStatus {
    Success,
    Error,
}

impl EntryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryStatus::Success => "success",
            EntryStatus::Error => "error",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "error" => EntryStatus::Error,
            _ => EntryStatus::Success,
        }
    }
}

/// A single transcription history entry stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionEntry {
    pub id: String,
    pub created_at: i64,
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
    /// Path to the saved audio file (for retry functionality).
    pub audio_path: Option<String>,
    /// Status of the entry: "success" or "error".
    pub status: String,
    /// Error message if transcription failed.
    pub error: Option<String>,
    /// `recording` for microphone captures and `file` for imported media.
    pub source_kind: String,
    /// Canonical user-owned import path. It is never deleted with history.
    pub source_path: Option<String>,
    /// Explicit translation target, or `None` for same-language output.
    pub translation_target: Option<String>,
    /// Provider timestamps when available. File imports fall back at export time.
    #[serde(default)]
    pub timed_segments: Vec<TimedSegment>,
    /// Last insertion outcome for this entry.
    pub delivery_status: String,
}

/// Parameters for saving a new transcription history entry.
#[derive(Debug, Clone)]
pub struct NewTranscriptionEntry {
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
    /// Path to the saved audio file (for retry functionality).
    pub audio_path: Option<String>,
    /// Status of the entry: "success" or "error".
    pub status: String,
    /// Error message if transcription failed.
    pub error: Option<String>,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub translation_target: Option<String>,
    pub timed_segments: Vec<TimedSegment>,
    pub delivery_status: String,
}

/// Filter parameters for querying history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFilter {
    pub search: Option<String>,
    pub engine: Option<String>,
    /// Filter by status: "success", "error", or None for all.
    pub status: Option<String>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatisticsPeriod {
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "all")]
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyStatistics {
    pub date: String,
    pub word_count: u64,
    pub dictation_count: u64,
    pub audio_duration_ms: u64,
    pub local_dictation_count: u64,
    pub cloud_dictation_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStatistics {
    pub period: StatisticsPeriod,
    pub range_start_ms: Option<i64>,
    pub range_end_ms: i64,
    pub word_count: u64,
    pub dictation_count: u64,
    pub audio_duration_ms: u64,
    pub active_days: u64,
    pub local_dictation_count: u64,
    pub cloud_dictation_count: u64,
    pub trend: Vec<DailyStatistics>,
}
