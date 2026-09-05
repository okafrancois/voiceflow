use tracing::warn;

use crate::state::app_state::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizeResult {
    DeliverText {
        text: String,
        history_entry_id: Option<String>,
    },
    OutputHandled {
        text: String,
        history_entry_id: Option<String>,
        disposition: DeliveryDisposition,
    },
    TransitionToIdle,
    TransitionToErrorThenIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryDisposition {
    Insert,
    Preview,
    Copied,
    CopyFailed,
}

impl DeliveryDisposition {
    fn initial_history_status(self) -> &'static str {
        match self {
            Self::Insert => "pending_insertion",
            Self::Preview => "not_delivered",
            Self::Copied => "copied",
            Self::CopyFailed => "copy_failed",
        }
    }

    fn is_already_delivered(self) -> bool {
        self != Self::Insert
    }
}

fn cleanup_audio_file(audio_path: Option<&str>) {
    if let Some(path) = audio_path {
        if let Err(error) = std::fs::remove_file(path) {
            warn!(error = %error, path = %path, "audio_cleanup_failed");
        }
    }
}

pub fn finalize_successful_transcription(
    state: &AppState,
    raw_text: &str,
    final_text: &str,
    polish_time_ms: u64,
    audio_path: Option<String>,
) -> FinalizeResult {
    finalize_successful_transcription_for_output(
        state,
        raw_text,
        final_text,
        polish_time_ms,
        audio_path,
        DeliveryDisposition::Insert,
    )
}

pub fn finalize_successful_transcription_for_output(
    state: &AppState,
    raw_text: &str,
    final_text: &str,
    polish_time_ms: u64,
    audio_path: Option<String>,
    disposition: DeliveryDisposition,
) -> FinalizeResult {
    let history_entry_id = crate::history::commands::save_to_history_with_delivery_status(
        state,
        crate::history::commands::HistorySaveRequest {
            raw_text,
            final_text,
            stt_duration_ms: None,
            polish_duration_ms: (polish_time_ms > 0).then_some(polish_time_ms as i64),
            polish_applied: polish_time_ms > 0,
            audio_path,
            delivery_status: disposition.initial_history_status(),
        },
    );

    if disposition.is_already_delivered() {
        FinalizeResult::OutputHandled {
            text: final_text.to_string(),
            history_entry_id,
            disposition,
        }
    } else {
        FinalizeResult::DeliverText {
            text: final_text.to_string(),
            history_entry_id,
        }
    }
}

pub fn finalize_silent_recording(audio_path: Option<String>) -> FinalizeResult {
    cleanup_audio_file(audio_path.as_deref());
    FinalizeResult::TransitionToIdle
}

pub fn finalize_empty_transcription(
    state: &AppState,
    audio_path: Option<String>,
) -> FinalizeResult {
    crate::history::commands::save_failed_history(state, audio_path, "Empty transcription result");
    FinalizeResult::TransitionToIdle
}

pub fn finalize_failed_transcription(
    state: &AppState,
    audio_path: Option<String>,
    error: &str,
) -> FinalizeResult {
    crate::history::commands::save_failed_history(state, audio_path, error);
    FinalizeResult::TransitionToErrorThenIdle
}

#[cfg(test)]
mod tests {
    use super::{
        finalize_empty_transcription, finalize_failed_transcription, finalize_silent_recording,
        finalize_successful_transcription, finalize_successful_transcription_for_output,
        DeliveryDisposition, FinalizeResult,
    };
    use crate::history::models::HistoryFilter;
    use crate::history::RetentionPolicy;
    use crate::state::app_state::AppState;
    use std::sync::atomic::Ordering;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::NamedTempFile;

    fn set_recording_long_enough(state: &AppState) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        state
            .recording_start_time
            .store(now_ms.saturating_sub(800), Ordering::SeqCst);
    }

    #[test]
    fn finalize_successful_transcription_requests_text_delivery() {
        let state = AppState::new();
        let audio = NamedTempFile::new().unwrap();
        let audio_path = audio.path().to_path_buf();
        let final_text = format!("default retention {}", uuid::Uuid::new_v4());

        let action = finalize_successful_transcription(
            &state,
            "raw text",
            &final_text,
            123,
            Some(audio_path.display().to_string()),
        );

        let history_entry_id = match action {
            FinalizeResult::DeliverText {
                text,
                history_entry_id,
            } => {
                assert_eq!(text, final_text);
                history_entry_id.expect("retained history entry")
            }
            other => panic!("expected delivery action, got {other:?}"),
        };
        assert!(!audio_path.exists());
        let entries = state
            .history_store
            .lock()
            .get_history(&HistoryFilter {
                search: Some(final_text),
                engine: None,
                status: Some("success".to_string()),
                date_from: None,
                date_to: None,
                limit: Some(5),
                offset: Some(0),
            })
            .unwrap();
        let entry = entries
            .iter()
            .find(|entry| entry.id == history_entry_id)
            .expect("saved entry");
        assert!(entry.audio_path.is_none());
        assert_eq!(entry.delivery_status, "pending_insertion");
    }

    #[test]
    fn preview_and_copy_have_distinct_delivery_statuses() {
        for (disposition, expected) in [
            (DeliveryDisposition::Preview, "not_delivered"),
            (DeliveryDisposition::Copied, "copied"),
            (DeliveryDisposition::CopyFailed, "copy_failed"),
        ] {
            let state = AppState::new();
            let final_text = format!("delivery status {expected} {}", uuid::Uuid::new_v4());

            let action = finalize_successful_transcription_for_output(
                &state,
                "raw text",
                &final_text,
                0,
                None,
                disposition,
            );
            let history_entry_id = match action {
                FinalizeResult::OutputHandled {
                    history_entry_id, ..
                } => history_entry_id.expect("retained history entry"),
                other => panic!("expected non-inserting action, got {other:?}"),
            };
            let entry = state
                .history_store
                .lock()
                .get_entry(&history_entry_id)
                .unwrap()
                .expect("saved entry");
            assert_eq!(entry.delivery_status, expected);
        }
    }

    #[test]
    fn successful_transcription_retains_audio_only_when_policy_allows_it() {
        let state = AppState::new();
        state.settings.lock().audio_retention = RetentionPolicy::Days30;
        let audio = NamedTempFile::new().unwrap();
        let audio_path = audio.path().to_path_buf();

        finalize_successful_transcription(
            &state,
            "raw text",
            "final text",
            0,
            Some(audio_path.display().to_string()),
        );

        assert!(audio_path.exists());
        let entries = state
            .history_store
            .lock()
            .get_history(&HistoryFilter {
                search: Some("final text".to_string()),
                engine: None,
                status: None,
                date_from: None,
                date_to: None,
                limit: Some(5),
                offset: Some(0),
            })
            .unwrap();
        assert!(entries.iter().any(
            |entry| entry.audio_path.as_deref() == Some(audio_path.to_string_lossy().as_ref())
        ));
    }

    #[test]
    fn successful_transcription_can_retain_audio_without_text() {
        let state = AppState::new();
        {
            let mut settings = state.settings.lock();
            settings.text_retention = RetentionPolicy::Never;
            settings.audio_retention = RetentionPolicy::Days30;
        }
        let retained_before = state.history_store.lock().retained_audio_count().unwrap();
        let audio = NamedTempFile::new().unwrap();
        let audio_path = audio.path().to_path_buf();

        finalize_successful_transcription(
            &state,
            "secret raw text",
            "secret final text",
            0,
            Some(audio_path.display().to_string()),
        );

        assert!(audio_path.exists());
        let store = state.history_store.lock();
        assert!(store
            .get_history(&HistoryFilter {
                search: None,
                engine: None,
                status: None,
                date_from: None,
                date_to: None,
                limit: Some(50),
                offset: Some(0),
            })
            .unwrap()
            .iter()
            .all(|entry| entry.raw_text != "secret raw text"
                && entry.final_text != "secret final text"));
        assert_eq!(store.retained_audio_count().unwrap(), retained_before + 1);
    }

    #[test]
    fn finalize_empty_transcription_saves_failed_history_and_returns_idle() {
        let state = AppState::new();
        set_recording_long_enough(&state);
        let audio = NamedTempFile::new().unwrap();

        let action = finalize_empty_transcription(&state, Some(audio.path().display().to_string()));

        assert_eq!(action, FinalizeResult::TransitionToIdle);

        let entries = state
            .history_store
            .lock()
            .get_history(&HistoryFilter {
                search: None,
                engine: None,
                status: Some("error".to_string()),
                date_from: None,
                date_to: None,
                limit: Some(5),
                offset: Some(0),
            })
            .unwrap();
        assert!(!entries.is_empty());
        assert!(entries
            .iter()
            .any(|entry| entry.error.as_deref() == Some("Empty transcription result")));
    }

    #[test]
    fn finalize_silent_recording_cleans_up_without_writing_history() {
        let state = AppState::new();
        set_recording_long_enough(&state);
        let audio = NamedTempFile::new().unwrap();
        let audio_path = audio.path().to_path_buf();
        let history_before = state
            .history_store
            .lock()
            .get_history(&HistoryFilter {
                search: None,
                engine: None,
                status: None,
                date_from: None,
                date_to: None,
                limit: Some(5),
                offset: Some(0),
            })
            .unwrap()
            .len();

        let action = finalize_silent_recording(Some(audio_path.display().to_string()));

        assert_eq!(action, FinalizeResult::TransitionToIdle);
        assert!(!audio_path.exists());

        let entries = state
            .history_store
            .lock()
            .get_history(&HistoryFilter {
                search: None,
                engine: None,
                status: None,
                date_from: None,
                date_to: None,
                limit: Some(5),
                offset: Some(0),
            })
            .unwrap();
        assert_eq!(entries.len(), history_before);
    }

    #[test]
    fn finalize_failed_transcription_saves_failed_history_and_returns_error() {
        let state = AppState::new();
        set_recording_long_enough(&state);

        let action = finalize_failed_transcription(&state, None, "network failed");

        assert_eq!(action, FinalizeResult::TransitionToErrorThenIdle);

        let entries = state
            .history_store
            .lock()
            .get_history(&HistoryFilter {
                search: None,
                engine: None,
                status: Some("error".to_string()),
                date_from: None,
                date_to: None,
                limit: Some(5),
                offset: Some(0),
            })
            .unwrap();
        assert!(!entries.is_empty());
        assert!(entries
            .iter()
            .any(|entry| entry.error.as_deref() == Some("network failed")));
    }
}
