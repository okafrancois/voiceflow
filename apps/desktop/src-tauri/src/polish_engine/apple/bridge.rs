//! Safe wrappers over the Foundation Models part of the Swift bridge.
//! [`generate`] blocks until the model answers: call it from a blocking thread.

use serde::Deserialize;

/// Whether the on-device language model can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppleLlmStatus {
    Available,
    /// Apple Intelligence disabled, model not ready, or device not eligible.
    Unavailable,
    OsUnsupported,
}

impl AppleLlmStatus {
    #[cfg_attr(not(apple_bridge), allow(dead_code))]
    fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Available,
            1 => Self::Unavailable,
            _ => Self::OsUnsupported,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppleLlmErrorKind {
    /// Apple's content filter refused the text.
    Refused,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppleLlmError {
    pub kind: AppleLlmErrorKind,
    pub message: String,
}

impl std::fmt::Display for AppleLlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Deserialize)]
#[cfg_attr(not(apple_bridge), allow(dead_code))]
struct GeneratePayload {
    text: Option<String>,
    error: Option<String>,
    kind: Option<String>,
}

#[cfg_attr(not(apple_bridge), allow(dead_code))]
fn parse_generate_payload(payload: &str) -> Result<String, AppleLlmError> {
    let parsed: GeneratePayload = serde_json::from_str(payload).map_err(|e| AppleLlmError {
        kind: AppleLlmErrorKind::Failed,
        message: format!("Invalid Apple Intelligence result: {e}"),
    })?;
    match (parsed.text, parsed.error) {
        (_, Some(message)) => Err(AppleLlmError {
            kind: match parsed.kind.as_deref() {
                Some("refused") => AppleLlmErrorKind::Refused,
                Some("unavailable") => AppleLlmErrorKind::Unavailable,
                _ => AppleLlmErrorKind::Failed,
            },
            message,
        }),
        (Some(text), None) => Ok(text),
        (None, None) => Err(AppleLlmError {
            kind: AppleLlmErrorKind::Failed,
            message: "Apple Intelligence returned no result".to_string(),
        }),
    }
}

#[cfg(apple_bridge)]
mod ffi {
    use std::ffi::{c_char, CStr, CString};

    extern "C" {
        fn vf_apple_llm_status() -> i32;
        fn vf_apple_llm_generate(instructions: *const c_char, prompt: *const c_char)
            -> *mut c_char;
        fn vf_apple_bridge_free(pointer: *mut c_char);
    }

    pub fn status() -> i32 {
        // SAFETY: no arguments; reads the system model availability.
        unsafe { vf_apple_llm_status() }
    }

    pub fn generate(instructions: &str, prompt: &str) -> String {
        let instructions = CString::new(instructions.replace('\0', "")).unwrap_or_default();
        let prompt = CString::new(prompt.replace('\0', "")).unwrap_or_default();
        // SAFETY: both arguments are valid NUL-terminated strings for the call.
        let pointer = unsafe { vf_apple_llm_generate(instructions.as_ptr(), prompt.as_ptr()) };
        if pointer.is_null() {
            return String::new();
        }
        // SAFETY: the bridge returns a NUL-terminated string from `strdup`.
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: `pointer` came from the bridge and is released exactly once.
        unsafe { vf_apple_bridge_free(pointer) };
        text
    }
}

#[cfg(apple_bridge)]
pub fn status() -> AppleLlmStatus {
    AppleLlmStatus::from_code(ffi::status())
}

#[cfg(not(apple_bridge))]
pub fn status() -> AppleLlmStatus {
    AppleLlmStatus::OsUnsupported
}

/// Runs one generation with `instructions` as the system prompt.
#[cfg(apple_bridge)]
pub fn generate(instructions: &str, prompt: &str) -> Result<String, AppleLlmError> {
    parse_generate_payload(&ffi::generate(instructions, prompt))
}

#[cfg(not(apple_bridge))]
pub fn generate(_instructions: &str, _prompt: &str) -> Result<String, AppleLlmError> {
    Err(AppleLlmError {
        kind: AppleLlmErrorKind::Unavailable,
        message: "Apple Intelligence is not available in this build".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_payload_maps_text_and_error_kinds() {
        assert_eq!(
            parse_generate_payload(r#"{"text":"Bonjour."}"#),
            Ok("Bonjour.".to_string())
        );
        let refused = parse_generate_payload(r#"{"error":"blocked","kind":"refused"}"#)
            .expect_err("a refusal is an error");
        assert_eq!(refused.kind, AppleLlmErrorKind::Refused);
        assert_eq!(refused.message, "blocked");
        assert_eq!(
            parse_generate_payload(r#"{"error":"off","kind":"unavailable"}"#)
                .expect_err("unavailable is an error")
                .kind,
            AppleLlmErrorKind::Unavailable
        );
        assert_eq!(
            parse_generate_payload("{}")
                .expect_err("empty payload is an error")
                .kind,
            AppleLlmErrorKind::Failed
        );
        assert!(parse_generate_payload("not json").is_err());
    }

    #[test]
    fn status_codes_map_to_variants() {
        assert_eq!(AppleLlmStatus::from_code(0), AppleLlmStatus::Available);
        assert_eq!(AppleLlmStatus::from_code(1), AppleLlmStatus::Unavailable);
        assert_eq!(AppleLlmStatus::from_code(3), AppleLlmStatus::OsUnsupported);
    }

    /// Calls into the linked bridge; any status proves it links and answers.
    #[test]
    fn bridge_answers_a_status_query() {
        let _ = status();
    }
}
