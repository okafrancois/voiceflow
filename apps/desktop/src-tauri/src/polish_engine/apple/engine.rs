use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tracing::info;

use super::bridge::{self, AppleLlmError};
use super::prompt;
use crate::polish_engine::traits::{PolishEngine, PolishEngineType, PolishRequest, PolishResult};

/// Runs one generation: (system prompt, user turn) → text. Blocking.
pub type TextGenerator = Arc<dyn Fn(&str, &str) -> Result<String, AppleLlmError> + Send + Sync>;

/// Polishes with Apple's on-device Foundation Models. Long transcripts are
/// polished in sentence chunks that fit the model's context window.
pub struct ApplePolishEngine {
    generator: TextGenerator,
}

impl ApplePolishEngine {
    pub fn new() -> Self {
        Self::with_generator(Arc::new(bridge::generate))
    }

    pub fn with_generator(generator: TextGenerator) -> Self {
        Self { generator }
    }
}

impl Default for ApplePolishEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PolishEngine for ApplePolishEngine {
    fn engine_type(&self) -> PolishEngineType {
        PolishEngineType::Apple
    }

    async fn polish(&self, request: PolishRequest) -> Result<PolishResult, String> {
        let started = Instant::now();
        let system_prompt = prompt::system_prompt(&request.system_context, &request.language);
        let chunks = prompt::chunks(&request.text, prompt::MAX_CHUNK_WORDS);
        if chunks.len() > 1 {
            info!(chunk_count = chunks.len(), "apple_polish_chunked");
        }

        let generator = Arc::clone(&self.generator);
        let polished = tokio::task::spawn_blocking(move || {
            chunks
                .iter()
                .map(|chunk| {
                    generator(&system_prompt, &prompt::user_turn(&chunk.text))
                        .map(|output| prompt::unwrap_output(&output))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|texts| prompt::join(&texts, &chunks))
        })
        .await
        .map_err(|e| format!("Apple Intelligence task failed: {e}"))?
        .map_err(|e| {
            info!(kind = ?e.kind, "apple_polish_failed");
            format!("Apple Intelligence polish failed: {e}")
        })?;

        let total_ms = started.elapsed().as_millis() as u64;
        info!(chars = polished.len(), total_ms, "apple_polish_completed");
        Ok(PolishResult::new(
            polished,
            PolishEngineType::Apple,
            total_ms,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polish_engine::apple::bridge::AppleLlmErrorKind;
    use parking_lot::Mutex;

    fn recording_generator(
        calls: Arc<Mutex<Vec<(String, String)>>>,
        reply: impl Fn(&str) -> Result<String, AppleLlmError> + Send + Sync + 'static,
    ) -> TextGenerator {
        Arc::new(move |system: &str, user: &str| {
            calls.lock().push((system.to_string(), user.to_string()));
            reply(user)
        })
    }

    #[tokio::test]
    async fn short_transcript_is_polished_in_one_framed_request() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let engine = ApplePolishEngine::with_generator(recording_generator(calls.clone(), |_| {
            Ok("<<<TRANSCRIPT\nBonjour, tout va bien.\nTRANSCRIPT>>>".to_string())
        }));

        let result = engine
            .polish(PolishRequest::new(
                "bonjour tout va bien",
                "Keep it short.",
                "fr",
            ))
            .await
            .expect("polish should succeed");

        assert_eq!(result.text, "Bonjour, tout va bien.");
        assert_eq!(result.engine, PolishEngineType::Apple);
        let calls = calls.lock();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains("TRANSCRIPT FRAMING"));
        assert!(calls[0].0.contains("Keep it short."));
        assert_eq!(
            calls[0].1,
            "<<<TRANSCRIPT\nbonjour tout va bien\nTRANSCRIPT>>>"
        );
    }

    #[tokio::test]
    async fn long_transcript_is_polished_per_chunk_and_joined_in_order() {
        let sentence = "mot ".repeat(99) + "fin.";
        let transcript = format!("{sentence} {sentence} {sentence}");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let counter = Arc::new(Mutex::new(0));
        let engine = ApplePolishEngine::with_generator(recording_generator(calls.clone(), {
            let counter = counter.clone();
            move |_| {
                let mut counter = counter.lock();
                *counter += 1;
                Ok(format!("Partie {}.", *counter))
            }
        }));

        let result = engine
            .polish(PolishRequest::new(transcript, "", "fr"))
            .await
            .expect("polish should succeed");

        assert_eq!(calls.lock().len(), 2);
        assert_eq!(result.text, "Partie 1. Partie 2.");
    }

    /// Needs macOS 26 with Apple Intelligence enabled. A dictated question must
    /// come back rewritten, not answered.
    #[tokio::test]
    #[ignore = "needs Apple Intelligence; run manually"]
    async fn real_model_rewrites_a_dictated_question_instead_of_answering_it() {
        let transcript = "euh est-ce que tu peux me dire à quoi sert le bouton retour sur la page ma cuisine parce que il est même pas sur la maquette et je comprends pas pourquoi on l'a ajouté";
        let result = ApplePolishEngine::new()
            .polish(PolishRequest::new(transcript, "", "fr"))
            .await
            .expect("Apple Intelligence polish should succeed");

        println!("{} ms: {}", result.total_ms, result.text);
        let words = |text: &str| text.split_whitespace().count() as f64;
        assert!(words(&result.text) / words(transcript) >= 0.6);
        assert!(result.text.contains("bouton retour"));
    }

    #[tokio::test]
    async fn a_refused_generation_fails_the_polish() {
        let engine = ApplePolishEngine::with_generator(Arc::new(|_: &str, _: &str| {
            Err(AppleLlmError {
                kind: AppleLlmErrorKind::Refused,
                message: "blocked by the content filter".to_string(),
            })
        }));

        let error = engine
            .polish(PolishRequest::new("texte", "", "fr"))
            .await
            .expect_err("a refusal must fail the polish");

        assert!(error.contains("blocked by the content filter"));
    }
}
