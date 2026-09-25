//! Safe wrappers over the Swift SpeechAnalyzer bridge (`swift/AppleSpeech`).
//!
//! Every function except [`AppleSpeechSession::feed`] blocks until the Swift
//! side finishes its asynchronous work: call them from blocking threads.

use serde::Deserialize;

/// Availability of the Apple engine for one locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppleSpeechStatus {
    Ready,
    AssetsMissing,
    LocaleUnsupported,
    OsUnsupported,
}

impl AppleSpeechStatus {
    #[cfg_attr(not(apple_speech), allow(dead_code))]
    fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Ready,
            1 => Self::AssetsMissing,
            2 => Self::LocaleUnsupported,
            _ => Self::OsUnsupported,
        }
    }
}

#[derive(Deserialize)]
#[cfg_attr(not(apple_speech), allow(dead_code))]
struct FinishPayload {
    text: Option<String>,
    error: Option<String>,
}

#[cfg_attr(not(apple_speech), allow(dead_code))]
fn parse_finish_payload(payload: &str) -> Result<String, String> {
    let parsed: FinishPayload =
        serde_json::from_str(payload).map_err(|e| format!("Invalid Apple speech result: {e}"))?;
    match (parsed.text, parsed.error) {
        (_, Some(error)) => Err(error),
        (Some(text), None) => Ok(text),
        (None, None) => Err("Apple speech returned no result".to_string()),
    }
}

#[cfg(apple_speech)]
mod ffi {
    use std::ffi::{c_char, CStr, CString};

    extern "C" {
        fn vf_apple_speech_status(locale: *const c_char) -> i32;
        fn vf_apple_speech_install(locale: *const c_char) -> *mut c_char;
        fn vf_apple_speech_start(locale: *const c_char, error: *mut *mut c_char) -> i64;
        fn vf_apple_speech_feed(id: i64, samples: *const i16, count: isize);
        fn vf_apple_speech_finish(id: i64) -> *mut c_char;
        fn vf_apple_speech_cancel(id: i64);
        fn vf_apple_speech_free(pointer: *mut c_char);
    }

    fn locale_arg(locale: &str) -> CString {
        CString::new(locale).unwrap_or_default()
    }

    /// Takes ownership of a string allocated by the bridge.
    fn take_string(pointer: *mut c_char) -> String {
        if pointer.is_null() {
            return String::new();
        }
        // SAFETY: the bridge returns NUL-terminated strings from `strdup`,
        // which stay valid until released with `vf_apple_speech_free`.
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: `pointer` came from the bridge and is released exactly once.
        unsafe { vf_apple_speech_free(pointer) };
        text
    }

    pub fn status(locale: &str) -> i32 {
        let locale = locale_arg(locale);
        // SAFETY: `locale` is a valid NUL-terminated string for the call.
        unsafe { vf_apple_speech_status(locale.as_ptr()) }
    }

    pub fn install(locale: &str) -> String {
        let locale = locale_arg(locale);
        // SAFETY: `locale` is a valid NUL-terminated string for the call.
        take_string(unsafe { vf_apple_speech_install(locale.as_ptr()) })
    }

    pub fn start(locale: &str) -> Result<i64, String> {
        let locale = locale_arg(locale);
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: `locale` is valid for the call and `error` points to a
        // writable pointer the bridge may set to an owned string.
        let id = unsafe { vf_apple_speech_start(locale.as_ptr(), &mut error) };
        if id > 0 {
            Ok(id)
        } else {
            let message = take_string(error);
            Err(if message.is_empty() {
                "Apple speech session failed to start".to_string()
            } else {
                message
            })
        }
    }

    pub fn feed(id: i64, samples: &[i16]) {
        // SAFETY: the pointer and length describe `samples`, which outlives
        // the call; the bridge copies the samples before returning.
        unsafe { vf_apple_speech_feed(id, samples.as_ptr(), samples.len() as isize) };
    }

    pub fn finish(id: i64) -> String {
        // SAFETY: plain value argument; the result is an owned bridge string.
        take_string(unsafe { vf_apple_speech_finish(id) })
    }

    pub fn cancel(id: i64) {
        // SAFETY: plain value argument; unknown ids are ignored by the bridge.
        unsafe { vf_apple_speech_cancel(id) };
    }
}

/// Reports whether the engine can transcribe `locale` (`auto` = system locale).
#[cfg(apple_speech)]
pub fn status(locale: &str) -> AppleSpeechStatus {
    AppleSpeechStatus::from_code(ffi::status(locale))
}

#[cfg(not(apple_speech))]
pub fn status(_locale: &str) -> AppleSpeechStatus {
    AppleSpeechStatus::OsUnsupported
}

/// Downloads and installs the on-device speech assets for `locale`.
#[cfg(apple_speech)]
pub fn install_assets(locale: &str) -> Result<(), String> {
    let error = ffi::install(locale);
    if error.is_empty() {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(not(apple_speech))]
pub fn install_assets(_locale: &str) -> Result<(), String> {
    Err("Apple speech recognition is not available in this build".to_string())
}

/// One analysis session. Dropping an unfinished session cancels it.
pub struct AppleSpeechSession {
    #[cfg_attr(not(apple_speech), allow(dead_code))]
    id: i64,
    finished: bool,
}

impl AppleSpeechSession {
    #[cfg(apple_speech)]
    pub fn start(locale: &str) -> Result<Self, String> {
        let id = ffi::start(locale)?;
        Ok(Self {
            id,
            finished: false,
        })
    }

    #[cfg(not(apple_speech))]
    pub fn start(_locale: &str) -> Result<Self, String> {
        Err("Apple speech recognition is not available in this build".to_string())
    }

    /// Queues 16 kHz mono samples. Returns without waiting for recognition.
    #[cfg(apple_speech)]
    pub fn feed(&self, samples: &[i16]) {
        ffi::feed(self.id, samples);
    }

    #[cfg(not(apple_speech))]
    pub fn feed(&self, _samples: &[i16]) {}

    /// Ends the input and waits for the final transcript.
    #[cfg(apple_speech)]
    pub fn finish(mut self) -> Result<String, String> {
        self.finished = true;
        parse_finish_payload(&ffi::finish(self.id))
    }

    #[cfg(not(apple_speech))]
    pub fn finish(mut self) -> Result<String, String> {
        self.finished = true;
        Err("Apple speech recognition is not available in this build".to_string())
    }
}

impl Drop for AppleSpeechSession {
    fn drop(&mut self) {
        if !self.finished {
            #[cfg(apple_speech)]
            ffi::cancel(self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_payload_returns_text_or_error() {
        assert_eq!(
            parse_finish_payload(r#"{"text":"bonjour"}"#),
            Ok("bonjour".to_string())
        );
        assert_eq!(
            parse_finish_payload(r#"{"error":"no speech"}"#),
            Err("no speech".to_string())
        );
        assert!(parse_finish_payload("{}").is_err());
        assert!(parse_finish_payload("not json").is_err());
    }

    #[test]
    fn status_codes_map_to_variants() {
        assert_eq!(AppleSpeechStatus::from_code(0), AppleSpeechStatus::Ready);
        assert_eq!(
            AppleSpeechStatus::from_code(1),
            AppleSpeechStatus::AssetsMissing
        );
        assert_eq!(
            AppleSpeechStatus::from_code(2),
            AppleSpeechStatus::LocaleUnsupported
        );
        assert_eq!(
            AppleSpeechStatus::from_code(3),
            AppleSpeechStatus::OsUnsupported
        );
    }

    /// Calls into the linked Swift bridge. Any status is valid: this proves
    /// the bridge links and answers without crashing on the host.
    #[test]
    fn bridge_answers_a_status_query() {
        let status = status("auto");
        assert!(matches!(
            status,
            AppleSpeechStatus::Ready
                | AppleSpeechStatus::AssetsMissing
                | AppleSpeechStatus::LocaleUnsupported
                | AppleSpeechStatus::OsUnsupported
        ));
    }

    /// Requires macOS 26 with speech assets for the system locale.
    #[test]
    #[ignore = "needs macOS 26 speech assets; run manually"]
    fn silent_audio_transcribes_to_empty_text() {
        let session = AppleSpeechSession::start("auto").expect("session should start");
        session.feed(&vec![0; 16_000 * 2]);
        let text = session.finish().expect("finish should succeed");
        assert!(text.trim().is_empty(), "unexpected text: {text}");
    }

    /// Transcribes a 16 kHz mono WAV named by `VF_APPLE_SPEECH_WAV` in the
    /// locale `VF_APPLE_SPEECH_LOCALE` (default `auto`), fed in 100 ms chunks
    /// like a recording.
    #[test]
    #[ignore = "needs macOS 26 speech assets and a WAV file; run manually"]
    fn wav_file_transcribes_through_the_bridge() {
        let path = std::env::var("VF_APPLE_SPEECH_WAV").expect("set VF_APPLE_SPEECH_WAV");
        let samples: Vec<i16> = hound::WavReader::open(path)
            .expect("WAV should open")
            .into_samples::<i16>()
            .map(|sample| sample.expect("WAV sample should decode"))
            .collect();

        let locale = std::env::var("VF_APPLE_SPEECH_LOCALE").unwrap_or_else(|_| "auto".into());
        let session = AppleSpeechSession::start(&locale).expect("session should start");
        let realtime = std::env::var("VF_APPLE_SPEECH_REALTIME").is_ok();
        for chunk in samples.chunks(1_600) {
            session.feed(chunk);
            if realtime {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        let started = std::time::Instant::now();
        let text = session.finish().expect("finish should succeed");
        println!("finish took {:?}: {text}", started.elapsed());

        assert!(!text.trim().is_empty());
    }
}
