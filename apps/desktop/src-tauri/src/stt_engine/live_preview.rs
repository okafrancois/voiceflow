use crate::stt_engine::buffering_engine::WindowDecoder;
use crate::stt_engine::traits::{PartialResult, PartialResultCallback};
use parking_lot::{Mutex, ReentrantMutex};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::warn;

const MIN_NEW_SAMPLES: usize = 16_000 * 2;
const MAX_PREVIEW_SAMPLES: usize = 16_000 * 30;
const INFERENCE_TIMEOUT: Duration = Duration::from_secs(15);
const FINISH_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Default)]
struct PreviewBuffer {
    samples: VecDeque<f32>,
    received: usize,
    requested: usize,
    closed: bool,
}

struct PreviewShared {
    buffer: Mutex<PreviewBuffer>,
    publication: ReentrantMutex<RefCell<Option<PartialResultCallback>>>,
    changed: Notify,
}

impl PreviewShared {
    fn publish(&self, text: String) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let publication = self.publication.lock();
        let callback = publication.borrow().clone();
        if let Some(callback) = callback {
            callback(PartialResult {
                text: text.to_string(),
                is_definite: false,
                is_final: false,
            });
        }
    }

    fn close(&self) {
        self.publication.lock().borrow_mut().take();
        self.buffer.lock().closed = true;
        self.changed.notify_one();
    }
}

struct PreviewTask {
    handle: JoinHandle<()>,
}

impl Drop for PreviewTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub(super) struct LivePreview {
    shared: Arc<PreviewShared>,
    task: Mutex<Option<PreviewTask>>,
}

impl LivePreview {
    pub(super) fn start(decoder: Arc<dyn WindowDecoder>, callback: PartialResultCallback) -> Self {
        let shared = Arc::new(PreviewShared {
            buffer: Mutex::new(PreviewBuffer::default()),
            publication: ReentrantMutex::new(RefCell::new(Some(callback))),
            changed: Notify::new(),
        });
        let task_shared = Arc::clone(&shared);
        let handle = tokio::spawn(async move {
            loop {
                let changed = task_shared.changed.notified();
                let snapshot = {
                    let mut buffer = task_shared.buffer.lock();
                    if buffer.closed {
                        return;
                    }
                    if buffer.received - buffer.requested >= MIN_NEW_SAMPLES {
                        buffer.requested = buffer.received;
                        buffer
                            .samples
                            .iter()
                            .any(|sample| *sample != 0.0)
                            .then(|| buffer.samples.iter().copied().collect())
                    } else {
                        None
                    }
                };
                let Some(snapshot) = snapshot else {
                    changed.await;
                    continue;
                };
                match tokio::time::timeout(INFERENCE_TIMEOUT, decoder.decode(snapshot)).await {
                    Ok(Ok(text)) => task_shared.publish(text),
                    Ok(Err(error)) => {
                        warn!(%error, "mlx_live_preview_failed-final_decode_preserved")
                    }
                    Err(_) => warn!("mlx_live_preview_timed_out-final_decode_preserved"),
                }
            }
        });
        Self {
            shared,
            task: Mutex::new(Some(PreviewTask { handle })),
        }
    }

    pub(super) fn replace_callback(&self, callback: PartialResultCallback) {
        let publication = self.shared.publication.lock();
        let mut current = publication.borrow_mut();
        if current.is_some() {
            *current = Some(callback);
        }
    }

    pub(super) fn push_samples(&self, samples: &[f32]) {
        let mut buffer = self.shared.buffer.lock();
        if buffer.closed || samples.is_empty() {
            return;
        }
        buffer.received += samples.len();
        let retained = samples.len().min(MAX_PREVIEW_SAMPLES);
        let discarded = (buffer.samples.len() + retained).saturating_sub(MAX_PREVIEW_SAMPLES);
        buffer.samples.drain(..discarded);
        buffer
            .samples
            .extend(samples[samples.len() - retained..].iter().copied());
        drop(buffer);
        self.shared.changed.notify_one();
    }

    pub(super) async fn finish(&self) {
        self.shared.close();
        let task = self.task.lock().take();
        if let Some(mut task) = task {
            match tokio::time::timeout(FINISH_DRAIN_TIMEOUT, &mut task.handle).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    warn!(%error, "mlx_live_preview_task_failed-final_decode_preserved")
                }
                Err(_) => {
                    task.handle.abort();
                    if let Err(error) = (&mut task.handle).await {
                        if !error.is_cancelled() {
                            warn!(%error, "mlx_live_preview_cleanup_failed-final_decode_preserved");
                        }
                    }
                }
            }
        }
    }
}

impl Drop for LivePreview {
    fn drop(&mut self) {
        self.shared.close();
    }
}

#[cfg(test)]
mod tests {
    use crate::stt_engine::buffering_engine::cancellation_test_support::ControlledDecoder;
    use crate::stt_engine::buffering_engine::{BufferingConsumer, WindowDecoder};
    use crate::stt_engine::traits::{EngineType, PartialResult, RecordingConsumer};
    use async_trait::async_trait;
    use futures_util::FutureExt;
    use parking_lot::Mutex;
    use std::collections::VecDeque;
    use std::sync::{Arc, Weak};
    use std::time::Duration;
    use tokio::sync::{mpsc, Notify, Semaphore};

    const SAMPLE_RATE: usize = 16_000;
    const WAIT: Duration = Duration::from_secs(1);

    struct SnapshotDecoder {
        samples: Mutex<Vec<Vec<f32>>>,
        replies: Mutex<VecDeque<Result<String, String>>>,
        permits: Semaphore,
        slot: Semaphore,
        started: Notify,
        stopped: Notify,
    }

    impl SnapshotDecoder {
        fn new(replies: Vec<Result<String, String>>, permits: usize) -> Arc<Self> {
            Arc::new(Self {
                samples: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.into()),
                permits: Semaphore::new(permits),
                slot: Semaphore::new(1),
                started: Notify::new(),
                stopped: Notify::new(),
            })
        }

        async fn wait_calls(&self, expected: usize) {
            tokio::time::timeout(WAIT, async {
                loop {
                    let notified = self.started.notified();
                    if self.samples.lock().len() >= expected {
                        return;
                    }
                    notified.await;
                }
            })
            .await
            .expect("expected genuine preview decoding before finish");
        }

        fn decoded_samples(&self) -> Vec<Vec<i16>> {
            self.samples
                .lock()
                .iter()
                .map(|window| {
                    window
                        .iter()
                        .map(|sample| (sample * 32768.0).round() as i16)
                        .collect()
                })
                .collect()
        }
    }

    struct DecodeLease<'a>(&'a Notify);

    impl Drop for DecodeLease<'_> {
        fn drop(&mut self) {
            self.0.notify_one();
        }
    }

    #[async_trait]
    impl WindowDecoder for SnapshotDecoder {
        async fn decode(&self, samples: Vec<f32>) -> Result<String, String> {
            let _slot = self.slot.acquire().await.unwrap();
            let _lease = DecodeLease(&self.stopped);
            let call_number = {
                let mut calls = self.samples.lock();
                calls.push(samples);
                calls.len()
            };
            let reply = self
                .replies
                .lock()
                .pop_front()
                .unwrap_or_else(|| Ok(format!("decoded {call_number}")));
            self.started.notify_one();
            self.permits.acquire().await.unwrap().forget();
            reply
        }
    }

    fn recording(samples: usize) -> Vec<i16> {
        (0..samples)
            .map(|index| ((index % 20_000) as i16) - 10_000)
            .collect()
    }

    fn consumer_with_partials(
        decoder: Arc<dyn WindowDecoder>,
        engine_type: EngineType,
    ) -> (BufferingConsumer, mpsc::UnboundedReceiver<PartialResult>) {
        let mut consumer = BufferingConsumer::with_decoder(decoder, engine_type);
        let (sender, receiver) = mpsc::unbounded_channel();
        consumer.set_partial_callback(Arc::new(move |partial| {
            sender.send(partial).unwrap();
        }));
        (consumer, receiver)
    }

    async fn partial(receiver: &mut mpsc::UnboundedReceiver<PartialResult>) -> PartialResult {
        tokio::time::timeout(WAIT, receiver.recv())
            .await
            .expect("MLX must publish a genuine partial before finish")
            .expect("partial publication should remain open while recording")
    }

    #[tokio::test]
    async fn mlx_live_partial_before_finish_replaces_text_and_preserves_full_tail() {
        let decoder = SnapshotDecoder::new(
            vec![
                Ok(" bonjour ".to_string()),
                Ok("bonjour encore".to_string()),
                Ok("authoritative final".to_string()),
            ],
            3,
        );
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        let samples = recording(SAMPLE_RATE * 4 + 317);
        consumer
            .send_chunk(samples[..SAMPLE_RATE * 2].to_vec())
            .await
            .unwrap();
        let first = partial(&mut partials).await;
        assert_eq!(first.text, "bonjour");
        assert!(!first.is_definite);
        assert!(!first.is_final);
        consumer
            .send_chunk(samples[SAMPLE_RATE * 2..SAMPLE_RATE * 4].to_vec())
            .await
            .unwrap();
        let replacement = partial(&mut partials).await;
        assert_eq!(replacement.text, "bonjour encore");
        assert!(!replacement.is_definite);
        assert!(!replacement.is_final);
        consumer
            .send_chunk(samples[SAMPLE_RATE * 4..].to_vec())
            .await
            .unwrap();
        assert_eq!(consumer.finish().await.unwrap(), "authoritative final");
        assert_eq!(decoder.decoded_samples().last(), Some(&samples));
        assert_eq!(decoder.samples.lock().len(), 3);
        assert!(partials.try_recv().is_err());
    }

    #[tokio::test]
    async fn mlx_preview_requires_two_seconds_of_new_audio_for_each_snapshot() {
        let decoder = SnapshotDecoder::new(Vec::new(), 3);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2 - 1))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(decoder.samples.lock().is_empty());
        consumer.send_chunk(vec![7]).await.unwrap();
        partial(&mut partials).await;
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2 - 1))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(decoder.samples.lock().len(), 1);
        consumer.send_chunk(vec![9]).await.unwrap();
        partial(&mut partials).await;
        assert_eq!(decoder.samples.lock().len(), 2);
        consumer.finish().await.unwrap();
    }

    #[tokio::test]
    async fn mlx_slow_preview_coalesces_latest_bounded_snapshot_without_blocking_ingestion() {
        let decoder = SnapshotDecoder::new(Vec::new(), 0);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        let samples = recording(SAMPLE_RATE * 72 + 1013);
        consumer
            .send_chunk(samples[..SAMPLE_RATE * 2].to_vec())
            .await
            .unwrap();
        decoder.wait_calls(1).await;
        tokio::time::timeout(Duration::from_millis(500), async {
            for chunk in samples[SAMPLE_RATE * 2..].chunks(SAMPLE_RATE / 2) {
                consumer.send_chunk(chunk.to_vec()).await.unwrap();
            }
        })
        .await
        .expect("audio ingestion must not await the slow preview decoder");
        assert_eq!(decoder.samples.lock().len(), 1);
        decoder.permits.add_permits(1);
        partial(&mut partials).await;
        decoder.wait_calls(2).await;
        assert_eq!(
            decoder.decoded_samples()[1],
            samples[samples.len() - SAMPLE_RATE * 30..]
        );
        decoder.permits.add_permits(1);
        partial(&mut partials).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(decoder.samples.lock().len(), 2);
        decoder.permits.add_permits(1);
        consumer.finish().await.unwrap();
        assert_eq!(decoder.decoded_samples().last(), Some(&samples));
        assert_eq!(decoder.samples.lock().len(), 3);
    }

    #[tokio::test]
    async fn mlx_finish_closes_publication_before_graceful_preview_drain() {
        let decoder = SnapshotDecoder::new(Vec::new(), 0);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        let samples = recording(SAMPLE_RATE * 2 + 211);
        consumer.send_chunk(samples.clone()).await.unwrap();
        decoder.wait_calls(1).await;
        let mut finishing = Box::pin(consumer.finish());
        assert!(
            tokio::time::timeout(Duration::from_millis(30), &mut finishing)
                .await
                .is_err()
        );
        decoder.permits.add_permits(1);
        tokio::select! {
            _ = decoder.wait_calls(2) => {},
            result = &mut finishing => panic!("final decode must wait for its permit: {result:?}"),
        }
        assert!(partials.try_recv().is_err());
        decoder.permits.add_permits(1);
        assert_eq!(finishing.await.unwrap(), "decoded 2");
        assert_eq!(decoder.decoded_samples().last(), Some(&samples));
        assert!(partials.try_recv().is_err());
    }

    #[tokio::test]
    async fn mlx_finish_aborts_and_awaits_stuck_preview_before_final_decode() {
        let decoder = SnapshotDecoder::new(Vec::new(), 0);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2))
            .await
            .unwrap();
        decoder.wait_calls(1).await;
        let finishing = tokio::spawn(async move { consumer.finish().await });
        tokio::time::timeout(Duration::from_millis(2400), async {
            loop {
                let notified = decoder.started.notified();
                if decoder.samples.lock().len() == 2 {
                    break;
                }
                notified.await;
            }
        })
        .await
        .expect("finish must release the preview slot after at most two seconds of drain");
        assert!(partials.try_recv().is_err());
        decoder.permits.add_permits(1);
        assert_eq!(finishing.await.unwrap().unwrap(), "decoded 2");
        assert_eq!(decoder.slot.available_permits(), 1);
    }

    async fn assert_decoder_reusable(decoder: Arc<ControlledDecoder>) {
        tokio::time::timeout(WAIT, decoder.stopped.notified())
            .await
            .expect("cancellation must drop the owned preview decoder future");
        assert_eq!(decoder.slot.available_permits(), 1);
        let next = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Mlx);
        next.send_chunk(recording(SAMPLE_RATE)).await.unwrap();
        decoder.release.add_permits(1);
        assert_eq!(
            tokio::time::timeout(WAIT, next.finish())
                .await
                .unwrap()
                .unwrap(),
            "controlled transcript"
        );
    }

    #[tokio::test]
    async fn mlx_dropping_consumer_cancels_preview_and_releases_decoder_slot() {
        let decoder = ControlledDecoder::new();
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2))
            .await
            .unwrap();
        tokio::time::timeout(WAIT, decoder.started.notified())
            .await
            .expect("preview decoder should start during recording");
        drop(consumer);
        assert_decoder_reusable(decoder).await;
        assert!(partials.try_recv().is_err());
    }

    #[tokio::test]
    async fn mlx_canceling_finish_during_drain_cancels_owned_preview() {
        let decoder = ControlledDecoder::new();
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2))
            .await
            .unwrap();
        tokio::time::timeout(WAIT, decoder.started.notified())
            .await
            .expect("preview decoder should start during recording");
        let consumer = Arc::new(consumer);
        let finishing = {
            let consumer = Arc::clone(&consumer);
            tokio::spawn(async move { consumer.finish().await })
        };
        tokio::time::sleep(Duration::from_millis(30)).await;
        finishing.abort();
        assert!(finishing.await.unwrap_err().is_cancelled());
        assert_decoder_reusable(decoder).await;
        assert!(partials.try_recv().is_err());
        drop(consumer);
    }

    #[tokio::test]
    async fn mlx_preview_failure_does_not_prevent_authoritative_final_attempt() {
        let decoder = SnapshotDecoder::new(
            vec![
                Err("preview unavailable".to_string()),
                Ok("final".to_string()),
            ],
            2,
        );
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        let samples = recording(SAMPLE_RATE * 2 + 37);
        consumer.send_chunk(samples.clone()).await.unwrap();
        decoder.wait_calls(1).await;
        tokio::time::timeout(WAIT, decoder.stopped.notified())
            .await
            .unwrap();
        assert!(partials.try_recv().is_err());
        assert_eq!(consumer.finish().await.unwrap(), "final");
        assert_eq!(decoder.decoded_samples().last(), Some(&samples));
    }

    #[tokio::test]
    async fn mlx_empty_preview_is_not_published_and_final_error_is_preserved() {
        let decoder = SnapshotDecoder::new(
            vec![Ok(" \n ".to_string()), Err("no speech".to_string())],
            2,
        );
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2))
            .await
            .unwrap();
        decoder.wait_calls(1).await;
        tokio::time::timeout(WAIT, decoder.stopped.notified())
            .await
            .unwrap();
        assert!(partials.try_recv().is_err());
        assert_eq!(consumer.finish().await, Err("no speech".to_string()));
        assert!(partials.try_recv().is_err());
    }

    #[tokio::test]
    async fn mlx_all_zero_snapshots_skip_preview_but_keep_final_silence_policy() {
        let decoder = SnapshotDecoder::new(vec![Err("silent recording".to_string())], 1);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        let samples = vec![0; SAMPLE_RATE * 3];
        consumer.send_chunk(samples.clone()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(decoder.samples.lock().is_empty());
        assert!(partials.try_recv().is_err());
        assert_eq!(consumer.finish().await, Err("silent recording".to_string()));
        assert_eq!(decoder.decoded_samples(), vec![samples]);
    }

    #[tokio::test]
    async fn mlx_preview_preserves_empty_short_and_minimum_duration_final_policy() {
        for sample_count in [0, 5599, 5600] {
            let decoder = SnapshotDecoder::new(vec![Err("final failure".to_string())], 1);
            let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
            let samples = recording(sample_count);
            consumer.send_chunk(samples.clone()).await.unwrap();
            let result = consumer.finish().await;
            if sample_count < 5600 {
                assert_eq!(result, Ok(String::new()));
                assert!(decoder.samples.lock().is_empty());
            } else {
                assert_eq!(result, Err("final failure".to_string()));
                assert_eq!(decoder.decoded_samples(), vec![samples]);
            }
            assert!(partials.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn mlx_without_partial_callback_remains_final_only() {
        let decoder = SnapshotDecoder::new(Vec::new(), 1);
        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Mlx);
        let samples = recording(SAMPLE_RATE * 35 + 11);
        consumer.send_chunk(samples.clone()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(decoder.samples.lock().is_empty());
        consumer.finish().await.unwrap();
        assert_eq!(decoder.decoded_samples(), vec![samples]);
    }

    #[tokio::test]
    async fn native_consumers_ignore_partial_callbacks_and_preserve_final_audio() {
        for engine_type in [
            EngineType::Whisper,
            EngineType::SenseVoice,
            EngineType::Qwen3Asr,
        ] {
            let decoder = SnapshotDecoder::new(Vec::new(), 1);
            let (consumer, mut partials) = consumer_with_partials(decoder.clone(), engine_type);
            let samples = recording(SAMPLE_RATE * 4 + 13);
            consumer.send_chunk(samples.clone()).await.unwrap();
            tokio::time::sleep(Duration::from_millis(30)).await;
            assert!(decoder.samples.lock().is_empty());
            consumer.finish().await.unwrap();
            assert_eq!(decoder.decoded_samples(), vec![samples]);
            assert!(partials.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn mlx_replacing_callback_keeps_one_owned_preview_task() {
        let decoder = SnapshotDecoder::new(Vec::new(), 0);
        let (mut consumer, mut previous) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2))
            .await
            .unwrap();
        decoder.wait_calls(1).await;
        let (sender, mut replacement) = mpsc::unbounded_channel();
        consumer.set_partial_callback(Arc::new(move |partial| {
            sender.send(partial).unwrap();
        }));
        decoder.permits.add_permits(1);
        assert_eq!(partial(&mut replacement).await.text, "decoded 1");
        assert!(previous.try_recv().is_err());
        assert_eq!(decoder.samples.lock().len(), 1);
        decoder.permits.add_permits(1);
        consumer.finish().await.unwrap();
    }

    #[tokio::test]
    async fn mlx_drop_before_preview_task_is_polled_does_not_start_inference() {
        let decoder = SnapshotDecoder::new(Vec::new(), 0);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        consumer
            .send_chunk(recording(SAMPLE_RATE * 2))
            .await
            .unwrap();
        drop(consumer);
        tokio::task::yield_now().await;
        assert!(decoder.samples.lock().is_empty());
        assert!(partials.try_recv().is_err());
    }

    #[tokio::test]
    async fn mlx_preview_timeout_drops_decoder_future_and_preserves_final_attempt() {
        let decoder = SnapshotDecoder::new(Vec::new(), 0);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        let samples = recording(SAMPLE_RATE * 2);
        consumer.send_chunk(samples.clone()).await.unwrap();
        decoder.wait_calls(1).await;
        tokio::time::timeout(Duration::from_secs(16), decoder.stopped.notified())
            .await
            .expect("preview inference must time out and release the manager slot");
        assert_eq!(decoder.slot.available_permits(), 1);
        assert_eq!(decoder.samples.lock().len(), 1);
        assert!(partials.try_recv().is_err());
        decoder.permits.add_permits(1);
        assert_eq!(consumer.finish().await.unwrap(), "decoded 2");
        assert_eq!(decoder.decoded_samples().last(), Some(&samples));
    }

    #[tokio::test]
    async fn mlx_oversized_input_chunk_keeps_only_recent_preview_audio() {
        let decoder = SnapshotDecoder::new(Vec::new(), 2);
        let (consumer, mut partials) = consumer_with_partials(decoder.clone(), EngineType::Mlx);
        let samples = recording(SAMPLE_RATE * 40 + 71);
        consumer.send_chunk(samples.clone()).await.unwrap();
        partial(&mut partials).await;
        assert_eq!(
            decoder.decoded_samples()[0],
            samples[samples.len() - SAMPLE_RATE * 30..]
        );
        consumer.finish().await.unwrap();
        assert_eq!(decoder.decoded_samples().last(), Some(&samples));
    }

    #[tokio::test]
    async fn mlx_callback_can_reenter_audio_ingestion_without_a_buffer_lock() {
        let decoder = SnapshotDecoder::new(Vec::new(), 2);
        let consumer_slot: Arc<Mutex<Option<Weak<BufferingConsumer>>>> = Arc::new(Mutex::new(None));
        let mut consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Mlx);
        let (sender, mut partials) = mpsc::unbounded_channel();
        let callback_slot = Arc::clone(&consumer_slot);
        consumer.set_partial_callback(Arc::new(move |partial| {
            let consumer = callback_slot.lock().as_ref().unwrap().upgrade().unwrap();
            consumer
                .send_chunk(vec![7; 13])
                .now_or_never()
                .expect("reentrant ingestion must not await preview work")
                .unwrap();
            sender.send(partial).unwrap();
        }));
        let consumer = Arc::new(consumer);
        *consumer_slot.lock() = Some(Arc::downgrade(&consumer));
        let mut samples = recording(SAMPLE_RATE * 2);
        consumer.send_chunk(samples.clone()).await.unwrap();
        partial(&mut partials).await;
        samples.extend([7; 13]);
        consumer.finish().await.unwrap();
        assert_eq!(decoder.decoded_samples().last(), Some(&samples));
    }
}
