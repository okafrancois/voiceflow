use async_trait::async_trait;
use parking_lot::Mutex;
use tracing::info;

use super::bridge::AppleSpeechSession;
use crate::stt_engine::traits::{PartialResultCallback, RecordingConsumer};

/// Streams recording chunks to SpeechAnalyzer as they arrive, so only the end
/// of the analysis remains when the recording stops.
pub struct AppleStreamingConsumer {
    session: Mutex<Option<AppleSpeechSession>>,
}

impl AppleStreamingConsumer {
    /// Opens the analysis session. Blocks a worker thread, not the runtime.
    pub async fn start(locale: String) -> Result<Self, String> {
        let session = tokio::task::spawn_blocking(move || AppleSpeechSession::start(&locale))
            .await
            .map_err(|e| format!("Apple speech task failed: {e}"))??;
        info!("apple_speech_session_started");
        Ok(Self {
            session: Mutex::new(Some(session)),
        })
    }
}

#[async_trait]
impl RecordingConsumer for AppleStreamingConsumer {
    async fn send_chunk(&self, pcm_data: Vec<i16>) -> Result<(), String> {
        match self.session.lock().as_ref() {
            Some(session) => {
                session.feed(&pcm_data);
                Ok(())
            }
            None => Err("Apple speech session already finished".to_string()),
        }
    }

    async fn finish(&self) -> Result<String, String> {
        let session = self
            .session
            .lock()
            .take()
            .ok_or_else(|| "Apple speech session already finished".to_string())?;
        tokio::task::spawn_blocking(move || session.finish())
            .await
            .map_err(|e| format!("Apple speech task failed: {e}"))?
    }

    fn set_partial_callback(&mut self, _callback: PartialResultCallback) {
        // No partial display in this version.
    }
}
