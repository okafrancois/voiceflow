use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

/// Task-owned audio survives removal of the active recording state at stop.
pub struct RecordingAudio {
    path: Option<PathBuf>,
    samples: Arc<Mutex<Vec<i16>>>,
}

impl RecordingAudio {
    pub fn new(path: Option<PathBuf>, samples: Arc<Mutex<Vec<i16>>>) -> Self {
        Self { path, samples }
    }

    /// Save captured samples after recording stops, if retention is enabled.
    pub fn save(&self, sample_rate: u32, channels: u16) -> Option<String> {
        let path = self.path.as_ref()?;
        let buffer = self.samples.lock();

        if buffer.is_empty() {
            warn!("raw_audio_buffer_empty-no_file_saved");
            return None;
        }

        if sample_rate == 0 || channels == 0 {
            warn!(
                sample_rate,
                channels, "audio_params_not_set-cannot_save_file"
            );
            return None;
        }

        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        match hound::WavWriter::create(path, spec) {
            Ok(mut writer) => {
                for sample in buffer.iter() {
                    if let Err(e) = writer.write_sample(*sample) {
                        warn!(error = %e, "wav_write_failed");
                        return None;
                    }
                }
                if let Err(e) = writer.finalize() {
                    warn!(error = %e, "wav_finalize_failed");
                    return None;
                }
                info!(path = %path.display(), samples = buffer.len(), sample_rate, channels, "raw_audio_saved");
                Some(path.to_string_lossy().to_string())
            }
            Err(e) => {
                warn!(error = %e, path = %path.display(), "wav_create_failed");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[test]
    fn retained_audio_survives_recording_state_removal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dictation.wav");
        let samples = Arc::new(Mutex::new(vec![10, 20]));
        let audio = RecordingAudio::new(Some(path.clone()), Arc::clone(&samples));
        // The recording's final callback runs before its state is dropped.
        samples.lock().extend_from_slice(&[30, 40]);
        drop(samples);

        assert_eq!(
            audio.save(48_000, 2),
            Some(path.to_string_lossy().into_owned())
        );
        let mut reader = hound::WavReader::open(path).unwrap();
        assert_eq!(reader.spec().sample_rate, 48_000);
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(
            reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![10, 20, 30, 40]
        );
    }

    #[test]
    fn never_retention_does_not_save_audio() {
        let audio = RecordingAudio::new(None, Arc::new(Mutex::new(vec![10; 160])));
        assert_eq!(audio.save(16_000, 1), None);
    }

    #[test]
    fn empty_audio_and_invalid_formats_do_not_create_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dictation.wav");
        for (samples, rate, channels) in
            [(vec![], 16_000, 1), (vec![10], 0, 1), (vec![10], 16_000, 0)]
        {
            let audio = RecordingAudio::new(Some(path.clone()), Arc::new(Mutex::new(samples)));
            assert_eq!(audio.save(rate, channels), None);
            assert!(!path.exists());
        }
    }
}
