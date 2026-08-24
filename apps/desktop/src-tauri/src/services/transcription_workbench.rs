use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

pub use crate::history::models::TimedSegment;

const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "flac", "ogg"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "webm"];
const MAX_HISTORY_AUDIO_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTranscriptionRequest {
    pub path: PathBuf,
    pub profile_id: Option<String>,
    pub translation_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTranscriptionResult {
    pub history_entry_id: Option<String>,
    pub raw_text: String,
    pub final_text: String,
    pub source_path: PathBuf,
    pub translation_target: Option<String>,
    pub output_action: String,
    pub delivery_status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Audio,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedMedia {
    pub path: PathBuf,
    pub kind: MediaKind,
    pub extension: String,
}

pub fn validate_media_path(path: &Path) -> Result<ValidatedMedia, String> {
    if !path.exists() {
        return Err(format!("Media file does not exist: {}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("Media path is not a file: {}", path.display()));
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "Media file has no extension".to_string())?;
    let kind = if AUDIO_EXTENSIONS.contains(&extension.as_str()) {
        MediaKind::Audio
    } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        MediaKind::Video
    } else {
        return Err(format!("Unsupported media extension: {extension}"));
    };
    let path = path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve media path: {error}"))?;
    Ok(ValidatedMedia {
        path,
        kind,
        extension,
    })
}

pub fn media_mime_type(path: &Path) -> Result<&'static str, String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "Media file has no extension".to_string())?;
    match extension.as_str() {
        "wav" => Ok("audio/wav"),
        "mp3" => Ok("audio/mpeg"),
        "m4a" => Ok("audio/mp4"),
        "flac" => Ok("audio/flac"),
        "ogg" => Ok("audio/ogg"),
        "mp4" => Ok("video/mp4"),
        "mov" => Ok("video/quicktime"),
        "webm" => Ok("video/webm"),
        _ => Err(format!("Unsupported media extension: {extension}")),
    }
}

pub fn validate_playback_audio(path: &Path) -> Result<&'static str, String> {
    let mime = media_mime_type(path)?;
    if mime.starts_with("video/") {
        return Err(
            "Video playback is unavailable in history; retranscription and export remain available"
                .to_string(),
        );
    }
    let size = std::fs::metadata(path)
        .map_err(|error| format!("Failed to inspect history audio: {error}"))?
        .len();
    if size > MAX_HISTORY_AUDIO_BYTES {
        return Err(format!(
            "History audio is too large for in-app playback ({size} bytes; limit {MAX_HISTORY_AUDIO_BYTES})"
        ));
    }
    Ok(mime)
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub fn decode_media_to_mono_16k(path: &Path) -> Result<DecodedAudio, String> {
    let media = validate_media_path(path)?;
    if media.extension == "wav" {
        return decode_wav_to_mono_16k(&media.path);
    }

    let temporary = transcode_media_to_wav(&media)?;
    decode_wav_to_mono_16k(temporary.wav_path())
}

pub fn decode_wav_to_mono_16k(path: &Path) -> Result<DecodedAudio, String> {
    let reader = hound::WavReader::open(path)
        .map_err(|error| format!("Failed to open WAV file: {error}"))?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err("Audio file has invalid channel or sample-rate metadata".to_string());
    }

    let interleaved = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to decode WAV samples: {error}"))?,
        (hound::SampleFormat::Int, bits) if bits <= 16 => {
            let scale = (1_u64 << bits.saturating_sub(1)) as f32;
            reader
                .into_samples::<i16>()
                .map(|sample| {
                    sample
                        .map(|sample| sample as f32 / scale)
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Failed to decode WAV samples: {error}"))?
        }
        (hound::SampleFormat::Int, bits) if bits <= 32 => {
            let scale = (1_u64 << bits.saturating_sub(1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|sample| {
                    sample
                        .map(|sample| sample as f32 / scale)
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Failed to decode WAV samples: {error}"))?
        }
        _ => {
            return Err(format!(
                "Unsupported WAV sample format: {:?} {}-bit",
                spec.sample_format, spec.bits_per_sample
            ))
        }
    };

    if interleaved.is_empty() {
        return Err("Audio file is empty".to_string());
    }
    let channel_count = usize::from(spec.channels);
    let mono = interleaved
        .chunks(channel_count)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect::<Vec<_>>();
    let samples = resample_linear(&mono, spec.sample_rate, 16_000)?;
    Ok(DecodedAudio {
        samples,
        sample_rate: 16_000,
    })
}

fn resample_linear(
    samples: &[f32],
    source_rate: u32,
    target_rate: u32,
) -> Result<Vec<f32>, String> {
    if samples.is_empty() || source_rate == 0 || target_rate == 0 {
        return Err("Audio resampling requires non-empty samples and valid rates".to_string());
    }
    if source_rate == target_rate {
        return Ok(samples.to_vec());
    }

    let target_len = ((samples.len() as u128 * u128::from(target_rate))
        .div_ceil(u128::from(source_rate))) as usize;
    let ratio = source_rate as f64 / target_rate as f64;
    let mut output = Vec::with_capacity(target_len);
    for index in 0..target_len {
        let source_position = index as f64 * ratio;
        let left_index = source_position.floor() as usize;
        let right_index = (left_index + 1).min(samples.len() - 1);
        let fraction = (source_position - left_index as f64) as f32;
        let left = samples[left_index.min(samples.len() - 1)];
        let right = samples[right_index];
        output.push(left + (right - left) * fraction);
    }
    Ok(output)
}

struct TemporaryMediaFiles {
    wav: PathBuf,
    intermediates: Vec<PathBuf>,
}

impl TemporaryMediaFiles {
    fn new() -> Self {
        let stem = format!("voice-flow-import-{}", uuid::Uuid::new_v4());
        Self {
            wav: std::env::temp_dir().join(format!("{stem}.wav")),
            intermediates: Vec::new(),
        }
    }

    fn wav_path(&self) -> &Path {
        &self.wav
    }
}

impl Drop for TemporaryMediaFiles {
    fn drop(&mut self) {
        for path in self.intermediates.iter().chain(std::iter::once(&self.wav)) {
            if path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn transcode_media_to_wav(media: &ValidatedMedia) -> Result<TemporaryMediaFiles, String> {
    let mut temporary = TemporaryMediaFiles::new();

    #[cfg(target_os = "macos")]
    {
        let audio_source = if media.kind == MediaKind::Video {
            let intermediate = temporary.wav.with_extension("m4a");
            let output = Command::new("/usr/bin/avconvert")
                .arg("--source")
                .arg(&media.path)
                .arg("--preset")
                .arg("PresetAppleM4A")
                .arg("--output")
                .arg(&intermediate)
                .arg("--replace")
                .output()
                .map_err(|error| format!("Failed to start macOS media converter: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "macOS media conversion failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            temporary.intermediates.push(intermediate.clone());
            intermediate
        } else {
            media.path.clone()
        };

        let output = Command::new("/usr/bin/afconvert")
            .arg(&audio_source)
            .arg(&temporary.wav)
            .arg("-f")
            .arg("WAVE")
            .arg("-d")
            .arg("LEI16@16000")
            .arg("-c")
            .arg("1")
            .output()
            .map_err(|error| format!("Failed to start macOS audio converter: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "macOS audio conversion failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let output = Command::new("ffmpeg")
            .arg("-nostdin")
            .arg("-y")
            .arg("-i")
            .arg(&media.path)
            .arg("-vn")
            .arg("-ac")
            .arg("1")
            .arg("-ar")
            .arg("16000")
            .arg("-f")
            .arg("wav")
            .arg(&temporary.wav)
            .output()
            .map_err(|error| {
                format!("Failed to start ffmpeg. Install or bundle ffmpeg to import this format: {error}")
            })?;
        if !output.status.success() {
            return Err(format!(
                "ffmpeg media conversion failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }

    if !temporary.wav.is_file() {
        return Err("Media converter did not create a WAV output".to_string());
    }
    Ok(temporary)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Txt,
    Markdown,
    Srt,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Markdown => "md",
            Self::Srt => "srt",
        }
    }
}

pub fn render_export(
    format: ExportFormat,
    raw_text: &str,
    final_text: &str,
    timed_segments: &[TimedSegment],
    duration_ms: Option<i64>,
) -> Result<String, String> {
    let raw_text = raw_text.trim();
    let final_text = final_text.trim();
    if final_text.is_empty() && raw_text.is_empty() {
        return Err("Cannot export an empty transcription".to_string());
    }
    let final_text = if final_text.is_empty() {
        raw_text
    } else {
        final_text
    };

    match format {
        ExportFormat::Txt => Ok(format!("{final_text}\n")),
        ExportFormat::Markdown => {
            let mut output =
                format!("# Voice Flow transcription\n\n## Final text\n\n{final_text}\n");
            if !raw_text.is_empty() && raw_text != final_text {
                output.push_str(&format!("\n## Raw transcription\n\n{raw_text}\n"));
            }
            Ok(output)
        }
        ExportFormat::Srt => render_srt(final_text, timed_segments, duration_ms),
    }
}

fn render_srt(
    final_text: &str,
    timed_segments: &[TimedSegment],
    duration_ms: Option<i64>,
) -> Result<String, String> {
    let fallback;
    let segments = if timed_segments.is_empty() {
        fallback = vec![TimedSegment {
            start_ms: 0,
            end_ms: duration_ms.unwrap_or(1_000).max(1),
            text: final_text.to_string(),
        }];
        fallback.as_slice()
    } else {
        timed_segments
    };

    let mut previous_end = 0;
    let mut output = String::new();
    for (index, segment) in segments.iter().enumerate() {
        if segment.start_ms < 0
            || segment.end_ms <= segment.start_ms
            || (index > 0 && segment.start_ms < previous_end)
        {
            return Err(format!(
                "Subtitle segment {} has an invalid time range",
                index + 1
            ));
        }
        let text = segment.text.trim().replace('\r', "");
        if text.is_empty() {
            return Err(format!("Subtitle segment {} is empty", index + 1));
        }
        output.push_str(&format!(
            "{}\n{} --> {}\n{}\n",
            index + 1,
            format_srt_timestamp(segment.start_ms),
            format_srt_timestamp(segment.end_ms),
            text
        ));
        if index + 1 < segments.len() {
            output.push('\n');
        }
        previous_end = segment.end_ms;
    }
    Ok(output)
}

fn format_srt_timestamp(milliseconds: i64) -> String {
    let milliseconds = milliseconds.max(0);
    let hours = milliseconds / 3_600_000;
    let minutes = (milliseconds % 3_600_000) / 60_000;
    let seconds = (milliseconds % 60_000) / 1_000;
    let remainder = milliseconds % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{remainder:03}")
}

pub fn write_export_file(path: &Path, contents: &str, overwrite: bool) -> Result<PathBuf, String> {
    if path.is_dir() {
        return Err(format!("Export path is a directory: {}", path.display()));
    }
    if path.exists() && !overwrite {
        return Err(format!(
            "Export file already exists and overwrite was not confirmed: {}",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| "Export path has no parent directory".to_string())?;
    if !parent.is_dir() {
        return Err(format!(
            "Export parent directory does not exist: {}",
            parent.display()
        ));
    }

    std::fs::write(path, contents)
        .map_err(|error| format!("Failed to write export file: {error}"))?;
    path.canonicalize()
        .map_err(|error| format!("Failed to resolve export file: {error}"))
}

pub fn pick_media_file() -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "macos")]
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(
            "POSIX path of (choose file with prompt \"Choose audio or video to transcribe\" of type {\"public.audio\", \"public.movie\"})",
        )
        .output()
        .map_err(|error| format!("Failed to open media picker: {error}"))?;

    #[cfg(target_os = "windows")]
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Filter = 'Media|*.wav;*.mp3;*.m4a;*.flac;*.ogg;*.mp4;*.mov;*.webm'; if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write($d.FileName) }",
        ])
        .output()
        .map_err(|error| format!("Failed to open media picker: {error}"))?;

    #[cfg(target_os = "linux")]
    let output = Command::new("zenity")
        .args([
            "--file-selection",
            "--title=Choose audio or video to transcribe",
            "--file-filter=Media | *.wav *.mp3 *.m4a *.flac *.ogg *.mp4 *.mov *.webm",
        ])
        .output()
        .map_err(|error| format!("Failed to open media picker: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        if stderr.contains("cancel") || output.status.code() == Some(1) {
            return Ok(None);
        }
        return Err(format!(
            "Media picker failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Ok(None);
    }
    Ok(Some(validate_media_path(Path::new(&path))?.path))
}

pub fn pick_export_file(format: ExportFormat) -> Result<Option<PathBuf>, String> {
    let extension = format.extension();
    let default_name = format!("voice-flow-transcription.{extension}");

    #[cfg(target_os = "macos")]
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(format!(
            "POSIX path of (choose file name with prompt \"Export transcription\" default name \"{default_name}\")"
        ))
        .output()
        .map_err(|error| format!("Failed to open export picker: {error}"))?;

    #[cfg(target_os = "windows")]
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            format!(
                "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.SaveFileDialog; $d.FileName = '{default_name}'; $d.Filter = 'Transcription|*.{extension}'; if ($d.ShowDialog() -eq 'OK') {{ [Console]::Out.Write($d.FileName) }}"
            )
            .as_str(),
        ])
        .output()
        .map_err(|error| format!("Failed to open export picker: {error}"))?;

    #[cfg(target_os = "linux")]
    let output = Command::new("zenity")
        .args([
            "--file-selection",
            "--save",
            "--confirm-overwrite",
            "--title=Export transcription",
            format!("--filename={default_name}").as_str(),
        ])
        .output()
        .map_err(|error| format!("Failed to open export picker: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        if stderr.contains("cancel") || output.status.code() == Some(1) {
            return Ok(None);
        }
        return Err(format!(
            "Export picker failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(path)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageMode {
    PreserveSource,
    Translate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePolicy {
    pub mode: LanguageMode,
    pub source_language: Option<String>,
    pub target_language: Option<String>,
    pub instruction: String,
}

pub fn build_language_policy(
    source_language: Option<&str>,
    translation_target: Option<&str>,
) -> Result<LanguagePolicy, String> {
    let source_language = clean_language(source_language);
    let target_language = clean_language(translation_target);

    match target_language {
        None => Ok(LanguagePolicy {
            mode: LanguageMode::PreserveSource,
            source_language,
            target_language: None,
            instruction:
                "Keep the result in the same language as the source text. Do not translate it."
                    .to_string(),
        }),
        Some(target_language) => {
            if source_language
                .as_deref()
                .zip(Some(target_language.as_str()))
                .is_some_and(|(source, target)| {
                    primary_language(source) == primary_language(target)
                })
            {
                return Err("Translation target must differ from the source language".to_string());
            }
            Ok(LanguagePolicy {
                mode: LanguageMode::Translate,
                source_language,
                instruction: format!(
                    "Translate the source text to {target_language}. Preserve meaning, names, numbers, and structure."
                ),
                target_language: Some(target_language),
            })
        }
    }
}

fn clean_language(language: Option<&str>) -> Option<String> {
    language
        .map(str::trim)
        .filter(|language| !language.is_empty() && *language != "auto")
        .map(str::to_string)
}

fn primary_language(language: &str) -> String {
    language
        .split(['-', '_'])
        .next()
        .unwrap_or(language)
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_validation_accepts_declared_audio_and_video_extensions() {
        for extension in ["wav", "MP3", "m4a", "flac", "ogg", "mp4", "mov", "webm"] {
            let file = tempfile::Builder::new()
                .suffix(format!(".{extension}").as_str())
                .tempfile()
                .unwrap();

            let media = validate_media_path(file.path()).unwrap();

            assert_eq!(media.path, file.path().canonicalize().unwrap());
        }
    }

    #[test]
    fn media_validation_rejects_missing_and_unsupported_files() {
        let missing = std::env::temp_dir().join("voice-flow-missing-media.exe");
        assert!(validate_media_path(&missing)
            .unwrap_err()
            .contains("does not exist"));

        let file = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
        assert_eq!(
            validate_media_path(file.path()).unwrap_err(),
            "Unsupported media extension: txt"
        );
    }

    #[test]
    fn media_mime_types_cover_every_supported_extension() {
        let cases = [
            ("recording.wav", "audio/wav"),
            ("recording.mp3", "audio/mpeg"),
            ("recording.m4a", "audio/mp4"),
            ("recording.flac", "audio/flac"),
            ("recording.ogg", "audio/ogg"),
            ("recording.mp4", "video/mp4"),
            ("recording.mov", "video/quicktime"),
            ("recording.webm", "video/webm"),
        ];
        for (path, mime) in cases {
            assert_eq!(media_mime_type(Path::new(path)).unwrap(), mime);
        }
    }

    #[test]
    fn history_playback_rejects_video_and_oversized_audio_before_reading() {
        let video = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
        assert!(validate_playback_audio(video.path())
            .unwrap_err()
            .contains("Video playback"));

        let audio = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        audio
            .as_file()
            .set_len(MAX_HISTORY_AUDIO_BYTES + 1)
            .unwrap();
        assert!(validate_playback_audio(audio.path())
            .unwrap_err()
            .contains("too large"));
    }

    #[test]
    fn srt_export_formats_ordered_timed_segments() {
        let segments = vec![
            TimedSegment {
                start_ms: 0,
                end_ms: 1_250,
                text: "First line".to_string(),
            },
            TimedSegment {
                start_ms: 61_001,
                end_ms: 62_345,
                text: "Second line".to_string(),
            },
        ];

        let srt = render_export(
            ExportFormat::Srt,
            "First line Second line",
            "First line Second line",
            &segments,
            Some(62_345),
        )
        .unwrap();

        assert_eq!(
            srt,
            "1\n00:00:00,000 --> 00:00:01,250\nFirst line\n\n2\n00:01:01,001 --> 00:01:02,345\nSecond line\n"
        );
    }

    #[test]
    fn srt_export_falls_back_to_one_duration_bounded_segment() {
        let srt = render_export(ExportFormat::Srt, "raw", "Final text", &[], Some(3_500)).unwrap();

        assert_eq!(srt, "1\n00:00:00,000 --> 00:00:03,500\nFinal text\n");
    }

    #[test]
    fn markdown_export_keeps_raw_and_final_versions() {
        let markdown = render_export(
            ExportFormat::Markdown,
            "raw words",
            "Final words.",
            &[],
            None,
        )
        .unwrap();

        assert!(markdown.contains("## Final text\n\nFinal words."));
        assert!(markdown.contains("## Raw transcription\n\nraw words"));
    }

    #[test]
    fn translation_policy_is_explicit_and_rejects_same_language() {
        let normal = build_language_policy(Some("fr"), None).unwrap();
        assert_eq!(normal.mode, LanguageMode::PreserveSource);
        assert!(normal.instruction.contains("same language"));

        let translated = build_language_policy(Some("fr-FR"), Some("en-US")).unwrap();
        assert_eq!(translated.mode, LanguageMode::Translate);
        assert_eq!(translated.target_language.as_deref(), Some("en-US"));

        assert_eq!(
            build_language_policy(Some("fr-FR"), Some("fr")).unwrap_err(),
            "Translation target must differ from the source language"
        );
    }

    #[test]
    fn invalid_timed_segments_are_rejected() {
        let segments = vec![TimedSegment {
            start_ms: 2_000,
            end_ms: 1_000,
            text: "broken".to_string(),
        }];

        assert_eq!(
            render_export(ExportFormat::Srt, "raw", "final", &segments, None).unwrap_err(),
            "Subtitle segment 1 has an invalid time range"
        );
    }

    #[test]
    fn wav_decoder_downmixes_and_resamples_to_sixteen_kilohertz() {
        let file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(file.path(), spec).unwrap();
        for _ in 0..80 {
            writer.write_sample(16_384_i16).unwrap();
            writer.write_sample(-16_384_i16).unwrap();
        }
        writer.finalize().unwrap();

        let decoded = decode_wav_to_mono_16k(file.path()).unwrap();

        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.samples.len(), 160);
        assert!(decoded.samples.iter().all(|sample| sample.abs() < 0.001));
    }

    #[test]
    fn empty_wav_is_rejected() {
        let file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let writer = hound::WavWriter::create(
            file.path(),
            hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        writer.finalize().unwrap();

        assert_eq!(
            decode_wav_to_mono_16k(file.path()).unwrap_err(),
            "Audio file is empty"
        );
    }

    #[test]
    fn export_write_requires_explicit_overwrite_confirmation() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("transcript.txt");
        std::fs::write(&output, "existing").unwrap();

        assert_eq!(
            write_export_file(&output, "replacement", false).unwrap_err(),
            format!(
                "Export file already exists and overwrite was not confirmed: {}",
                output.display()
            )
        );
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "existing");

        write_export_file(&output, "replacement", true).unwrap();
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "replacement");
    }

    #[test]
    fn export_write_rejects_directories_and_missing_parents() {
        let directory = tempfile::tempdir().unwrap();
        assert!(write_export_file(directory.path(), "text", true)
            .unwrap_err()
            .contains("directory"));

        let output = directory.path().join("missing").join("transcript.srt");
        assert!(write_export_file(&output, "text", false)
            .unwrap_err()
            .contains("does not exist"));
    }
}
