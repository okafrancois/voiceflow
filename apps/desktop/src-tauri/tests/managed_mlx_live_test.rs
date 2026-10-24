use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use voiceflow_lib::stt_engine::buffering_engine::BufferingConsumer;
use voiceflow_lib::stt_engine::traits::{PartialResult, RecordingConsumer, SttContext};
use voiceflow_lib::stt_engine::{EngineType, UnifiedEngineManager};

struct ManagedSession(Arc<UnifiedEngineManager>);

impl Drop for ManagedSession {
    fn drop(&mut self) {
        self.0.unload_model();
    }
}

#[tokio::test]
#[ignore = "requires isolated installed MLX checkpoints; set VOICEFLOW_MLX_TEST_ROOT and VOICEFLOW_MLX_TEST_WAV"]
async fn managed_mlx_emits_real_french_partials_while_audio_is_still_arriving() {
    let root = std::path::PathBuf::from(
        std::env::var("VOICEFLOW_MLX_TEST_ROOT").expect("set an isolated VOICEFLOW_MLX_TEST_ROOT"),
    );
    assert!(root.starts_with(std::env::temp_dir()) || root.starts_with("/tmp"));
    let wav = std::env::var("VOICEFLOW_MLX_TEST_WAV").expect("set VOICEFLOW_MLX_TEST_WAV");
    let decoded =
        voiceflow_lib::services::transcription_workbench::decode_media_to_mono_16k(Path::new(&wav))
            .unwrap();
    let recording: Vec<i16> = decoded
        .samples
        .iter()
        .map(|sample| (sample * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect::<Vec<_>>()
        .repeat(3);
    assert!(recording.len() > 4 * 16_000);
    let manager = Arc::new(UnifiedEngineManager::new(root.join("models")));
    let _session = ManagedSession(manager.clone());

    for model in ["mlx-whisper-turbo", "mlx-parakeet-v3", "mlx-qwen3-asr-0.6b"] {
        assert!(manager.is_model_downloaded(EngineType::Mlx, model));
        manager.preload_model(EngineType::Mlx, model).await.unwrap();
        let received = Arc::new(AtomicUsize::new(0));
        let recording_open = Arc::new(AtomicBool::new(true));
        let partials = Arc::new(Mutex::new(Vec::new()));
        let started = Instant::now();
        let mut consumer = BufferingConsumer::new(
            manager.clone(),
            model.to_string(),
            "fr".to_string(),
            None,
            SttContext {
                domain: Some("it".to_string()),
                subdomain: Some("general".to_string()),
                ..SttContext::default()
            },
        );
        let received_in_callback = received.clone();
        let open_in_callback = recording_open.clone();
        let partials_in_callback = partials.clone();
        consumer.set_partial_callback(Arc::new(move |partial: PartialResult| {
            assert!(open_in_callback.load(Ordering::SeqCst));
            assert!(!partial.is_final);
            assert!(!partial.is_definite);
            partials_in_callback.lock().push((
                received_in_callback.load(Ordering::SeqCst),
                started.elapsed().as_millis(),
                partial.text,
            ));
        }));

        let mut clock = tokio::time::interval(Duration::from_millis(200));
        for chunk in recording.chunks(3_200) {
            clock.tick().await;
            received.fetch_add(chunk.len(), Ordering::SeqCst);
            consumer.send_chunk(chunk.to_vec()).await.unwrap();
        }
        let before_finish = partials.lock().clone();
        assert!(
            before_finish
                .iter()
                .any(|(samples, _, text)| *samples < recording.len() && !text.is_empty()),
            "{model} returned no genuine partial while subsequent audio was arriving"
        );
        assert!(
            before_finish
                .iter()
                .any(|(_, _, text)| text.to_lowercase().contains("bonjour")),
            "{model} live preview did not preserve the French fixture: {before_finish:?}"
        );
        let final_text = consumer.finish().await.unwrap();
        recording_open.store(false, Ordering::SeqCst);
        let count_after_finish = partials.lock().len();
        assert_eq!(count_after_finish, before_finish.len());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(partials.lock().len(), count_after_finish);
        assert!(
            final_text.to_lowercase().contains("voix française"),
            "{model} final French transcription: {final_text}"
        );
        println!(
            "{}",
            serde_json::json!({
                "model": model,
                "audio_samples": recording.len(),
                "partials": before_finish,
                "final": final_text,
                "elapsed_ms": started.elapsed().as_millis(),
            })
        );
        drop(consumer);
        manager.unload_model();
    }
}
