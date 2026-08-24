use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[path = "platform_quality_store.rs"]
pub mod store;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub total_memory_mb: Option<u64>,
    pub logical_cpu_count: usize,
    pub architecture: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRecommendation {
    pub model_name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicrophoneCheck {
    pub ready: bool,
    pub device_name: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub peak_level: Option<f32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencySample {
    pub stt_ms: u64,
    pub polish_ms: Option<u64>,
    pub total_ms: u64,
    pub model_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticInput {
    pub microphone: MicrophoneCheck,
    pub hardware: HardwareSnapshot,
    pub has_cloud_credentials: bool,
    pub latency: Option<LatencySample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub microphone: MicrophoneCheck,
    pub hardware: HardwareSnapshot,
    pub recommended_model: ModelRecommendation,
    pub recommended_preset: SetupPreset,
    pub recommendation_reason: String,
    pub latency: Option<LatencySample>,
}

pub fn build_diagnostic_report(input: DiagnosticInput) -> DiagnosticReport {
    let recommended_model = recommend_local_model(&input.hardware);
    let recommended_preset = if input.microphone.ready && !input.has_cloud_credentials {
        SetupPreset::Private
    } else if input.microphone.ready {
        SetupPreset::Balanced
    } else {
        SetupPreset::MaximumAccuracy
    };
    let recommendation_reason = match recommended_preset {
        SetupPreset::Private => format!(
            "The microphone is ready and {} fits this hardware, so transcription can stay on-device without cloud credentials.",
            recommended_model.model_name
        ),
        SetupPreset::Balanced => format!(
            "The microphone is ready and {} fits this hardware; cloud services remain available as an explicit choice.",
            recommended_model.model_name
        ),
        SetupPreset::MaximumAccuracy => {
            "The microphone is not ready; configure it before measuring a local model, or use an explicitly configured cloud provider."
                .to_string()
        }
    };

    DiagnosticReport {
        microphone: input.microphone,
        hardware: input.hardware,
        recommended_model,
        recommended_preset,
        recommendation_reason,
        latency: input.latency,
    }
}

pub fn recommend_local_model(hardware: &HardwareSnapshot) -> ModelRecommendation {
    let memory = hardware.total_memory_mb.unwrap_or(0);
    let cpu = hardware.logical_cpu_count;
    let (model_name, reason) = if memory < 4 * 1_024 || cpu < 4 {
        (
            "whisper-tiny",
            "Limited memory or CPU capacity favors the smallest reliable multilingual model.",
        )
    } else if memory < 8 * 1_024 {
        (
            "whisper-base",
            "Available memory fits the balanced Whisper Base model.",
        )
    } else if memory < 16 * 1_024 {
        (
            "qwen3-asr-0.6b-int8",
            "Available memory and CPU capacity fit the high-accuracy compact Qwen3-ASR model.",
        )
    } else if memory < 24 * 1_024 {
        (
            "whisper-turbo",
            "Available memory supports Whisper Turbo for high accuracy with moderate latency.",
        )
    } else {
        (
            "whisper-large-v3",
            "Available memory and CPU capacity support maximum local accuracy.",
        )
    };
    ModelRecommendation {
        model_name: model_name.to_string(),
        reason: reason.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupPreset {
    Private,
    Balanced,
    MaximumAccuracy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetContract {
    pub cloud_stt_enabled: bool,
    pub window_context_enabled: bool,
    pub clipboard_context_enabled: bool,
    pub ocr_fallback_enabled: bool,
    pub correction_memory_enabled: bool,
    pub text_retention: String,
    pub audio_retention: String,
}

pub fn preset_contract(preset: SetupPreset) -> PresetContract {
    match preset {
        SetupPreset::Private => PresetContract {
            cloud_stt_enabled: false,
            window_context_enabled: false,
            clipboard_context_enabled: false,
            ocr_fallback_enabled: false,
            correction_memory_enabled: true,
            text_retention: "days30".to_string(),
            audio_retention: "never".to_string(),
        },
        SetupPreset::Balanced => PresetContract {
            cloud_stt_enabled: false,
            window_context_enabled: true,
            clipboard_context_enabled: false,
            ocr_fallback_enabled: false,
            correction_memory_enabled: true,
            text_retention: "days90".to_string(),
            audio_retention: "never".to_string(),
        },
        SetupPreset::MaximumAccuracy => PresetContract {
            cloud_stt_enabled: true,
            window_context_enabled: true,
            clipboard_context_enabled: false,
            ocr_fallback_enabled: false,
            correction_memory_enabled: true,
            text_retention: "forever".to_string(),
            audio_retention: "days7".to_string(),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LastTextVersion {
    Raw,
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum BridgeRequest {
    Start {
        profile_id: Option<String>,
    },
    Stop,
    Cancel,
    Status,
    TranscribeFile {
        path: String,
        profile_id: Option<String>,
    },
    Insert {
        text: String,
    },
    CopyLast {
        version: LastTextVersion,
    },
    ReinsertLast {
        version: LastTextVersion,
    },
    Submit,
    SetCodeContext {
        context: CodeContext,
    },
    ClearCodeContext,
    FormatCode {
        text: String,
        language: Option<String>,
    },
}

pub fn parse_bridge_url(url: &str) -> Result<BridgeRequest, String> {
    let remainder = url
        .strip_prefix("voiceflow://")
        .ok_or_else(|| "Unsupported URL scheme".to_string())?;
    let (command, query) = remainder
        .split_once('?')
        .map_or((remainder, ""), |(command, query)| (command, query));
    let command = command.trim_matches('/');
    if command.is_empty() {
        return Err("Bridge command is missing".to_string());
    }
    let query = parse_query(query)?;

    match command {
        "start" => Ok(BridgeRequest::Start {
            profile_id: query.get("profile").cloned(),
        }),
        "stop" => Ok(BridgeRequest::Stop),
        "cancel" => Ok(BridgeRequest::Cancel),
        "status" => Ok(BridgeRequest::Status),
        "transcribe-file" => Ok(BridgeRequest::TranscribeFile {
            path: required_query(&query, "path", "transcribe-file requires a path")?,
            profile_id: query.get("profile").cloned(),
        }),
        "insert" => Ok(BridgeRequest::Insert {
            text: required_query(&query, "text", "insert requires text")?,
        }),
        "copy-last" => Ok(BridgeRequest::CopyLast {
            version: parse_last_version(query.get("version").map(String::as_str))?,
        }),
        "reinsert-last" => Ok(BridgeRequest::ReinsertLast {
            version: parse_last_version(query.get("version").map(String::as_str))?,
        }),
        "submit" => Ok(BridgeRequest::Submit),
        "code-context" => Ok(BridgeRequest::SetCodeContext {
            context: CodeContext {
                language: query.get("language").cloned(),
                file_path: query.get("file").cloned(),
                symbol: query.get("symbol").cloned(),
                editor_id: query.get("editor").cloned(),
            },
        }),
        "clear-code-context" => Ok(BridgeRequest::ClearCodeContext),
        _ => Err(format!("Unknown bridge command: {command}")),
    }
}

pub fn bridge_urls_from_args(args: &[String]) -> Vec<&str> {
    args.iter()
        .map(String::as_str)
        .filter(|argument| argument.starts_with("voiceflow://"))
        .collect()
}

pub fn should_show_main_for_args(args: &[String]) -> bool {
    bridge_urls_from_args(args).is_empty()
}

pub fn bridge_request_name(request: &BridgeRequest) -> &'static str {
    match request {
        BridgeRequest::Start { .. } => "start",
        BridgeRequest::Stop => "stop",
        BridgeRequest::Cancel => "cancel",
        BridgeRequest::Status => "status",
        BridgeRequest::TranscribeFile { .. } => "transcribe-file",
        BridgeRequest::Insert { .. } => "insert",
        BridgeRequest::CopyLast { .. } => "copy-last",
        BridgeRequest::ReinsertLast { .. } => "reinsert-last",
        BridgeRequest::Submit => "submit",
        BridgeRequest::SetCodeContext { .. } => "set-code-context",
        BridgeRequest::ClearCodeContext => "clear-code-context",
        BridgeRequest::FormatCode { .. } => "format-code",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeEndpoint {
    pub protocol_version: u8,
    pub address: String,
    pub token: String,
    pub process_id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeEnvelope {
    pub token: String,
    pub request: BridgeRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub ok: bool,
    pub command: String,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl BridgeResponse {
    pub fn success(request: &BridgeRequest, data: Option<serde_json::Value>) -> Self {
        Self {
            ok: true,
            command: bridge_request_name(request).to_string(),
            data,
            error: None,
        }
    }

    pub fn failure(request: &BridgeRequest, error: impl Into<String>) -> Self {
        Self {
            ok: false,
            command: bridge_request_name(request).to_string(),
            data: None,
            error: Some(error.into()),
        }
    }
}

pub fn bridge_endpoint_path() -> PathBuf {
    crate::utils::AppPaths::shared_data_dir().join("developer-bridge.json")
}

pub fn read_bridge_endpoint(path: &std::path::Path) -> Result<BridgeEndpoint, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Voice Flow developer bridge is not running: {error}"))?;
    let endpoint = serde_json::from_slice::<BridgeEndpoint>(&bytes)
        .map_err(|error| format!("Developer bridge endpoint is invalid: {error}"))?;
    let address = endpoint
        .address
        .parse::<SocketAddr>()
        .map_err(|_| "Developer bridge address is invalid".to_string())?;
    if !address.ip().is_loopback() {
        return Err("Developer bridge endpoint is not loopback-only".to_string());
    }
    if endpoint.token.len() < 32 {
        return Err("Developer bridge token is invalid".to_string());
    }
    Ok(endpoint)
}

pub fn send_bridge_request(
    endpoint: &BridgeEndpoint,
    request: &BridgeRequest,
) -> Result<BridgeResponse, String> {
    let address = endpoint
        .address
        .parse::<SocketAddr>()
        .map_err(|_| "Developer bridge address is invalid".to_string())?;
    if !address.ip().is_loopback() {
        return Err("Developer bridge endpoint is not loopback-only".to_string());
    }
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|error| format!("Failed to connect to developer bridge: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|error| format!("Failed to configure developer bridge client: {error}"))?;
    let envelope = BridgeEnvelope {
        token: endpoint.token.clone(),
        request: request.clone(),
    };
    write_bridge_envelope(&mut stream, &envelope)?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| format!("Failed to finish developer bridge request: {error}"))?;

    read_bridge_response(BufReader::new(stream))
}

fn write_bridge_envelope(writer: &mut impl Write, envelope: &BridgeEnvelope) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, envelope)
        .map_err(|error| format!("Failed to encode developer bridge request: {error}"))?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|error| format!("Failed to send developer bridge request: {error}"))
}

fn read_bridge_response(reader: impl BufRead) -> Result<BridgeResponse, String> {
    let mut response = String::new();
    reader
        .take(256 * 1_024)
        .read_line(&mut response)
        .map_err(|error| format!("Failed to read developer bridge response: {error}"))?;
    if response.trim().is_empty() {
        return Err("Developer bridge returned an empty response".to_string());
    }
    serde_json::from_str(response.trim())
        .map_err(|error| format!("Developer bridge response is invalid: {error}"))
}

pub fn parse_bridge_cli_args(
    args: &[String],
    stdin_json: Option<&str>,
) -> Result<BridgeRequest, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err("Bridge command is missing".to_string());
    };
    match command {
        "start" => Ok(BridgeRequest::Start {
            profile_id: args.get(1).cloned(),
        }),
        "stop" => Ok(BridgeRequest::Stop),
        "cancel" => Ok(BridgeRequest::Cancel),
        "status" => Ok(BridgeRequest::Status),
        "submit" => Ok(BridgeRequest::Submit),
        "insert" => Ok(BridgeRequest::Insert {
            text: args
                .get(1)
                .filter(|text| !text.is_empty())
                .cloned()
                .ok_or_else(|| "insert requires text".to_string())?,
        }),
        "transcribe-file" => Ok(BridgeRequest::TranscribeFile {
            path: args
                .get(1)
                .filter(|path| !path.is_empty())
                .cloned()
                .ok_or_else(|| "transcribe-file requires a path".to_string())?,
            profile_id: args.get(2).cloned(),
        }),
        "copy-last" => Ok(BridgeRequest::CopyLast {
            version: parse_last_version(args.get(1).map(String::as_str))?,
        }),
        "reinsert-last" => Ok(BridgeRequest::ReinsertLast {
            version: parse_last_version(args.get(1).map(String::as_str))?,
        }),
        "open" => parse_bridge_url(
            args.get(1)
                .ok_or_else(|| "open requires a voiceflow:// URL".to_string())?,
        ),
        "code-context" => {
            let json =
                stdin_json.ok_or_else(|| "code-context requires JSON on stdin".to_string())?;
            let context = serde_json::from_str::<CodeContext>(json)
                .map_err(|error| format!("Invalid code context JSON: {error}"))?;
            Ok(BridgeRequest::SetCodeContext { context })
        }
        "clear-code-context" => Ok(BridgeRequest::ClearCodeContext),
        "format-code" => Ok(BridgeRequest::FormatCode {
            text: stdin_json
                .filter(|text| !text.is_empty())
                .ok_or_else(|| "format-code requires text on stdin".to_string())?
                .to_string(),
            language: args.get(1).cloned(),
        }),
        _ => Err(format!("Unknown bridge command: {command}")),
    }
}

pub fn run_bridge_cli(
    args: &[String],
    stdin: Option<&str>,
    endpoint_path: Option<&std::path::Path>,
) -> Result<BridgeResponse, String> {
    let request = parse_bridge_cli_args(args, stdin)?;
    let default_path;
    let path = if let Some(path) = endpoint_path {
        path
    } else {
        default_path = bridge_endpoint_path();
        &default_path
    };
    let endpoint = read_bridge_endpoint(path)?;
    send_bridge_request(&endpoint, &request)
}

fn parse_query(query: &str) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode(key)?;
        let value = percent_decode(value)?;
        values.insert(key, value);
    }
    Ok(values)
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = decode_hex(bytes[index + 1])?;
                let low = decode_hex(bytes[index + 2])?;
                output.push((high << 4) | low);
                index += 3;
            }
            b'%' => return Err("Malformed percent encoding".to_string()),
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| "URL argument is not valid UTF-8".to_string())
}

fn decode_hex(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("Malformed percent encoding".to_string()),
    }
}

fn required_query(
    query: &BTreeMap<String, String>,
    key: &str,
    error: &str,
) -> Result<String, String> {
    query
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| error.to_string())
}

fn parse_last_version(version: Option<&str>) -> Result<LastTextVersion, String> {
    match version.unwrap_or("final") {
        "raw" => Ok(LastTextVersion::Raw),
        "final" => Ok(LastTextVersion::Final),
        version => Err(format!("Unknown text version: {version}")),
    }
}

pub fn authorize_loopback_bridge(
    peer_ip: &str,
    supplied_token: &str,
    expected_token: &str,
) -> Result<(), String> {
    let peer = peer_ip
        .parse::<IpAddr>()
        .map_err(|_| "Bridge peer address is invalid".to_string())?;
    if !peer.is_loopback() {
        return Err("Bridge requests must originate from loopback".to_string());
    }
    if !constant_time_equal(supplied_token.as_bytes(), expected_token.as_bytes()) {
        return Err("Invalid bridge token".to_string());
    }
    Ok(())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeContext {
    pub language: Option<String>,
    pub file_path: Option<String>,
    pub symbol: Option<String>,
    pub editor_id: Option<String>,
}

static ACTIVE_CODE_CONTEXT: LazyLock<RwLock<Option<CodeContext>>> =
    LazyLock::new(|| RwLock::new(None));

pub fn set_active_code_context(context: CodeContext) -> Result<CodeContext, String> {
    let context = sanitize_code_context(context);
    *ACTIVE_CODE_CONTEXT
        .write()
        .map_err(|_| "Code context store is unavailable".to_string())? = Some(context.clone());
    Ok(context)
}

pub fn get_active_code_context() -> Result<Option<CodeContext>, String> {
    ACTIVE_CODE_CONTEXT
        .read()
        .map(|context| context.clone())
        .map_err(|_| "Code context store is unavailable".to_string())
}

pub fn clear_active_code_context() -> Result<(), String> {
    *ACTIVE_CODE_CONTEXT
        .write()
        .map_err(|_| "Code context store is unavailable".to_string())? = None;
    Ok(())
}

fn sanitize_code_context(context: CodeContext) -> CodeContext {
    CodeContext {
        language: clean_code_context_value(context.language, 64),
        file_path: clean_code_context_value(context.file_path, 1_024),
        symbol: clean_code_context_value(context.symbol, 256),
        editor_id: clean_code_context_value(context.editor_id, 256),
    }
}

fn clean_code_context_value(value: Option<String>, max_chars: usize) -> Option<String> {
    value
        .map(|value| {
            value
                .chars()
                .filter(|character| !matches!(character, '\r' | '\n' | '\0'))
                .take(max_chars)
                .collect::<String>()
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn build_code_aware_instruction(context: &CodeContext) -> String {
    let mut instruction = String::from(
        "Preserve code identifiers and casing, paths, command flags, punctuation, indentation, and line breaks exactly when they are dictated. Do not wrap the result in a Markdown code fence.",
    );
    for (label, value) in [
        ("Language", context.language.as_deref()),
        ("File", context.file_path.as_deref()),
        ("Symbol", context.symbol.as_deref()),
        ("Editor", context.editor_id.as_deref()),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            let value: String = value
                .chars()
                .filter(|character| *character != '\r' && *character != '\n')
                .take(512)
                .collect();
            instruction.push_str(&format!(" {label}: {value}."));
        }
    }
    instruction
}

pub fn apply_code_aware_policy(
    text: &str,
    base_prompt: &str,
    enabled: bool,
    context: &CodeContext,
) -> (String, String) {
    if !enabled {
        return (text.to_string(), base_prompt.to_string());
    }
    let formatted = format_code_aware_transcript(text, context.language.as_deref());
    let instruction = build_code_aware_instruction(context);
    let prompt = if base_prompt.trim().is_empty() {
        instruction
    } else {
        format!("{}\n\n{}", base_prompt.trim(), instruction)
    };
    (formatted, prompt)
}

/// Expands only explicit code punctuation phrases. All other tokens retain their
/// original casing and spelling.
pub fn format_code_aware_transcript(input: &str, _language: Option<&str>) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(format_code_aware_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_code_aware_line(input: &str) -> String {
    let contains_explicit_phrase = input.split_whitespace().any(|word| {
        matches!(
            word.to_ascii_lowercase().as_str(),
            "new"
                | "dash"
                | "slash"
                | "backslash"
                | "underscore"
                | "dot"
                | "colon"
                | "open-paren"
                | "open-parenthesis"
                | "close-paren"
                | "close-parenthesis"
        )
    });
    if !contains_explicit_phrase {
        return input.to_string();
    }
    let indentation_bytes = input
        .char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    let (indentation, body) = input.split_at(indentation_bytes);
    let words = body.split_whitespace().collect::<Vec<_>>();
    let mut output = String::with_capacity(input.len());
    output.push_str(indentation);
    let mut index = 0;
    let mut attach_next = false;
    let mut path_mode = false;

    while index < words.len() {
        let word = words[index];
        let lowered = word.to_ascii_lowercase();
        let next = words.get(index + 1).map(|word| word.to_ascii_lowercase());

        if lowered == "new" && next.as_deref() == Some("line") {
            while output.ends_with(' ') {
                output.pop();
            }
            output.push('\n');
            attach_next = false;
            path_mode = false;
            index += 2;
            continue;
        }

        let (replacement, consumes_next, attaches_after) = match (lowered.as_str(), next.as_deref())
        {
            ("dash", Some("dash")) => (Some("--"), true, true),
            ("slash", _) => (Some("/"), false, true),
            ("backslash", _) => (Some("\\"), false, true),
            ("underscore", _) => (Some("_"), false, true),
            ("dot", _) => (Some("."), false, true),
            ("colon", _) => (Some(":"), false, true),
            ("open-paren", _) | ("open-parenthesis", _) => (Some("("), false, true),
            ("close-paren", _) | ("close-parenthesis", _) => (Some(")"), false, false),
            _ => (None, false, false),
        };

        if let Some(replacement) = replacement {
            if matches!(replacement, "/" | "\\") {
                if !attach_next
                    && !path_mode
                    && !output.is_empty()
                    && !output.ends_with([' ', '\n'])
                {
                    output.push(' ');
                }
                path_mode = true;
            } else if matches!(replacement, "_" | "." | ":" | ")") {
                while output.ends_with(' ') {
                    output.pop();
                }
            } else if !output.is_empty() && !output.ends_with([' ', '\n']) && !attach_next {
                output.push(' ');
            }
            output.push_str(replacement);
            attach_next = attaches_after;
            index += if consumes_next { 2 } else { 1 };
            continue;
        }

        let was_attached = attach_next;
        if !output.is_empty() && !output.ends_with([' ', '\n']) && !was_attached {
            output.push(' ');
            path_mode = false;
        }
        output.push_str(word);
        attach_next = false;
        if !was_attached {
            path_mode = false;
        }
        index += 1;
    }

    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityEventKind {
    TranscriptionSuccess,
    TranscriptionFailure,
    InjectionFailure,
    Correction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityEvent {
    pub kind: QualityEventKind,
    pub application_id: Option<String>,
    pub stt_ms: Option<u64>,
    pub polish_ms: Option<u64>,
    pub total_ms: Option<u64>,
    #[serde(default)]
    pub is_cloud: Option<bool>,
    pub created_at_ms: i64,
}

impl QualityEvent {
    pub fn success(
        application_id: Option<&str>,
        stt_ms: u64,
        polish_ms: u64,
        total_ms: u64,
    ) -> Self {
        Self {
            kind: QualityEventKind::TranscriptionSuccess,
            application_id: application_id.map(str::to_string),
            stt_ms: Some(stt_ms),
            polish_ms: Some(polish_ms),
            total_ms: Some(total_ms),
            is_cloud: None,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn success_with_source(
        application_id: Option<&str>,
        stt_ms: u64,
        polish_ms: u64,
        total_ms: u64,
        is_cloud: bool,
    ) -> Self {
        let mut event = Self::success(application_id, stt_ms, polish_ms, total_ms);
        event.is_cloud = Some(is_cloud);
        event
    }

    pub fn transcription_failure(
        application_id: Option<&str>,
        total_ms: u64,
        is_cloud: bool,
    ) -> Self {
        Self {
            kind: QualityEventKind::TranscriptionFailure,
            application_id: application_id.map(str::to_string),
            stt_ms: None,
            polish_ms: None,
            total_ms: Some(total_ms),
            is_cloud: Some(is_cloud),
            created_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn injection_failure(application_id: Option<&str>, total_ms: u64) -> Self {
        Self {
            kind: QualityEventKind::InjectionFailure,
            application_id: application_id.map(str::to_string),
            stt_ms: None,
            polish_ms: None,
            total_ms: Some(total_ms),
            is_cloud: None,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn correction(application_id: Option<&str>) -> Self {
        Self {
            kind: QualityEventKind::Correction,
            application_id: application_id.map(str::to_string),
            stt_ms: None,
            polish_ms: None,
            total_ms: None,
            is_cloud: None,
            created_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencyPercentiles {
    pub p50: Option<u64>,
    pub p95: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QualitySummary {
    pub total_transcriptions: u64,
    pub transcription_failures: u64,
    pub injection_failures: u64,
    pub corrections: u64,
    pub correction_rate_percent: Option<f64>,
    pub local_transcriptions: u64,
    pub cloud_transcriptions: u64,
    pub stt_latency_ms: LatencyPercentiles,
    pub polish_latency_ms: LatencyPercentiles,
    pub total_latency_ms: LatencyPercentiles,
    pub application_injection_failures: BTreeMap<String, u64>,
}

pub fn summarize_quality(events: &[QualityEvent]) -> QualitySummary {
    let mut summary = QualitySummary::default();
    let mut stt = Vec::new();
    let mut polish = Vec::new();
    let mut total = Vec::new();
    let mut application_failures = HashMap::<String, u64>::new();

    for event in events {
        match event.kind {
            QualityEventKind::TranscriptionSuccess => summary.total_transcriptions += 1,
            QualityEventKind::TranscriptionFailure => summary.transcription_failures += 1,
            QualityEventKind::InjectionFailure => {
                summary.injection_failures += 1;
                if let Some(application_id) = event.application_id.as_deref() {
                    *application_failures
                        .entry(application_id.to_string())
                        .or_default() += 1;
                }
            }
            QualityEventKind::Correction => summary.corrections += 1,
        }
        if matches!(
            event.kind,
            QualityEventKind::TranscriptionSuccess | QualityEventKind::TranscriptionFailure
        ) {
            match event.is_cloud {
                Some(true) => summary.cloud_transcriptions += 1,
                Some(false) => summary.local_transcriptions += 1,
                None => {}
            }
        }
        stt.extend(event.stt_ms);
        polish.extend(event.polish_ms);
        total.extend(event.total_ms);
    }

    summary.stt_latency_ms = percentiles(stt);
    summary.polish_latency_ms = percentiles(polish);
    summary.total_latency_ms = percentiles(total);
    summary.application_injection_failures = application_failures.into_iter().collect();
    if summary.total_transcriptions > 0 {
        summary.correction_rate_percent =
            Some(summary.corrections as f64 * 100.0 / summary.total_transcriptions as f64);
    }
    summary
}

fn percentiles(mut values: Vec<u64>) -> LatencyPercentiles {
    if values.is_empty() {
        return LatencyPercentiles::default();
    }
    values.sort_unstable();
    LatencyPercentiles {
        p50: nearest_rank(&values, 50),
        p95: nearest_rank(&values, 95),
    }
}

fn nearest_rank(values: &[u64], percentile: usize) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let rank = (percentile * values.len()).div_ceil(100).max(1);
    values.get(rank - 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_recommendation_explains_hardware_tradeoff() {
        let low = recommend_local_model(&HardwareSnapshot {
            total_memory_mb: Some(3_000),
            logical_cpu_count: 2,
            architecture: "x86_64".to_string(),
        });
        assert_eq!(low.model_name, "whisper-tiny");
        assert!(low.reason.contains("memory"));

        let capable = recommend_local_model(&HardwareSnapshot {
            total_memory_mb: Some(32 * 1_024),
            logical_cpu_count: 10,
            architecture: "aarch64".to_string(),
        });
        assert_eq!(capable.model_name, "whisper-large-v3");
        assert!(capable.reason.contains("maximum local accuracy"));
    }

    #[test]
    fn setup_presets_have_distinct_documented_privacy_profiles() {
        let private = preset_contract(SetupPreset::Private);
        assert!(!private.cloud_stt_enabled);
        assert!(!private.clipboard_context_enabled);
        assert_eq!(private.audio_retention, "never");

        let balanced = preset_contract(SetupPreset::Balanced);
        assert!(!balanced.cloud_stt_enabled);
        assert!(balanced.correction_memory_enabled);
        assert_eq!(balanced.text_retention, "days90");

        let accurate = preset_contract(SetupPreset::MaximumAccuracy);
        assert!(accurate.cloud_stt_enabled);
        assert!(accurate.window_context_enabled);
        assert!(!accurate.ocr_fallback_enabled);
    }

    #[test]
    fn bridge_url_parser_accepts_known_commands_and_decodes_arguments() {
        assert_eq!(
            parse_bridge_url("voiceflow://start?profile=Code%20Mode").unwrap(),
            BridgeRequest::Start {
                profile_id: Some("Code Mode".to_string())
            }
        );
        assert_eq!(
            parse_bridge_url("voiceflow://transcribe-file?path=%2Ftmp%2Fnote.wav").unwrap(),
            BridgeRequest::TranscribeFile {
                path: "/tmp/note.wav".to_string(),
                profile_id: None,
            }
        );
        assert_eq!(
            parse_bridge_url("voiceflow://copy-last?version=raw").unwrap(),
            BridgeRequest::CopyLast {
                version: LastTextVersion::Raw
            }
        );
    }

    #[test]
    fn single_instance_arguments_expose_only_voiceflow_urls_for_dispatch() {
        let args = vec![
            "/Applications/Voice Flow.app/Contents/MacOS/voiceflow".to_string(),
            "--unrelated".to_string(),
            "voiceflow://status".to_string(),
        ];

        assert_eq!(bridge_urls_from_args(&args), vec!["voiceflow://status"]);
        assert!(!should_show_main_for_args(&args));
        assert!(should_show_main_for_args(&["voiceflow".to_string()]));
    }

    #[test]
    fn bridge_url_parser_rejects_unknown_or_malformed_requests() {
        assert_eq!(
            parse_bridge_url("https://example.com/start").unwrap_err(),
            "Unsupported URL scheme"
        );
        assert_eq!(
            parse_bridge_url("voiceflow://launch-missiles").unwrap_err(),
            "Unknown bridge command: launch-missiles"
        );
        assert_eq!(
            parse_bridge_url("voiceflow://transcribe-file").unwrap_err(),
            "transcribe-file requires a path"
        );
    }

    #[test]
    fn code_aware_instruction_preserves_code_shaped_tokens() {
        let instruction = build_code_aware_instruction(&CodeContext {
            language: Some("rust".to_string()),
            file_path: Some("src/services/mod.rs".to_string()),
            symbol: Some("parse_bridge_url".to_string()),
            editor_id: Some("com.microsoft.VSCode".to_string()),
        });

        assert!(instruction.contains("identifiers and casing"));
        assert!(instruction.contains("src/services/mod.rs"));
        assert!(instruction.contains("parse_bridge_url"));
        assert!(!instruction.contains("```"));
    }

    #[test]
    fn quality_summary_reports_percentiles_and_application_failures_without_content() {
        let events = vec![
            QualityEvent::success(Some("editor"), 100, 20, 130),
            QualityEvent::success(Some("editor"), 200, 40, 250),
            QualityEvent::injection_failure(Some("terminal"), 300),
            QualityEvent::correction(Some("editor")),
        ];

        let summary = summarize_quality(&events);

        assert_eq!(summary.total_transcriptions, 2);
        assert_eq!(summary.injection_failures, 1);
        assert_eq!(summary.corrections, 1);
        assert_eq!(summary.stt_latency_ms.p50, Some(100));
        assert_eq!(summary.stt_latency_ms.p95, Some(200));
        assert_eq!(
            summary.application_injection_failures.get("terminal"),
            Some(&1)
        );
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("raw_text"));
        assert!(!json.contains("final_text"));
        assert!(!json.contains("Final words"));
    }

    #[test]
    fn loopback_bridge_policy_rejects_remote_or_invalid_token() {
        assert!(authorize_loopback_bridge("127.0.0.1", "secret", "secret").is_ok());
        assert_eq!(
            authorize_loopback_bridge("192.168.1.20", "secret", "secret").unwrap_err(),
            "Bridge requests must originate from loopback"
        );
        assert_eq!(
            authorize_loopback_bridge("::1", "wrong", "secret").unwrap_err(),
            "Invalid bridge token"
        );
    }

    #[test]
    fn diagnostic_report_recommends_private_setup_without_cloud_credentials() {
        let report = build_diagnostic_report(DiagnosticInput {
            microphone: MicrophoneCheck {
                ready: true,
                device_name: Some("Built-in Microphone".to_string()),
                sample_rate_hz: Some(48_000),
                channels: Some(1),
                peak_level: Some(0.42),
                error: None,
            },
            hardware: HardwareSnapshot {
                total_memory_mb: Some(16 * 1_024),
                logical_cpu_count: 10,
                architecture: "aarch64".to_string(),
            },
            has_cloud_credentials: false,
            latency: None,
        });

        assert!(report.microphone.ready);
        assert_eq!(report.recommended_preset, SetupPreset::Private);
        assert_eq!(report.recommended_model.model_name, "whisper-turbo");
        assert!(report.recommendation_reason.contains("on-device"));
    }

    #[test]
    fn code_formatter_expands_explicit_shell_tokens_without_changing_identifiers() {
        let formatted = format_code_aware_transcript(
            "cargo test dash dash package VoiceFlow new line cd slash tmp slash MyProject",
            Some("shell"),
        );

        assert_eq!(
            formatted,
            "cargo test --package VoiceFlow\ncd /tmp/MyProject"
        );
    }

    #[test]
    fn code_formatter_preserves_existing_line_breaks_paths_flags_and_casing() {
        let formatted = format_code_aware_transcript(
            "const HTTPServer = VoiceFlow\n\ncargo run dash dash config slash tmp slash MyConfig",
            Some("rust"),
        );

        assert_eq!(
            formatted,
            "const HTTPServer = VoiceFlow\n\ncargo run --config /tmp/MyConfig"
        );
    }

    #[test]
    fn code_policy_formats_input_and_appends_typed_context_to_the_profile_prompt() {
        let (text, prompt) = apply_code_aware_policy(
            "cargo run dash dash release",
            "Correct clear transcription errors.",
            true,
            &CodeContext {
                language: Some("rust".to_string()),
                file_path: Some("src/main.rs".to_string()),
                symbol: Some("HTTPServer".to_string()),
                editor_id: Some("com.microsoft.VSCode".to_string()),
            },
        );

        assert_eq!(text, "cargo run --release");
        assert!(prompt.starts_with("Correct clear transcription errors."));
        assert!(prompt.contains("Language: rust."));
        assert!(prompt.contains("File: src/main.rs."));
        assert!(prompt.contains("Symbol: HTTPServer."));
    }

    #[test]
    fn code_formatter_preserves_existing_indentation_and_spacing_without_explicit_phrases() {
        let input = "    let VoiceFlow = HTTPServer::new();  // keep spacing";
        assert_eq!(format_code_aware_transcript(input, Some("rust")), input);
    }

    #[test]
    fn editor_bridge_parses_json_context_and_sanitizes_control_characters() {
        let request = parse_bridge_cli_args(
            &["code-context".to_string()],
            Some(
                r#"{"language":"rust\nignore","file_path":"src/main.rs","symbol":"HTTPServer","editor_id":"com.microsoft.VSCode"}"#,
            ),
        )
        .unwrap();
        let BridgeRequest::SetCodeContext { context } = request else {
            panic!("expected code context request");
        };
        let stored = set_active_code_context(context).unwrap();
        assert_eq!(stored.language.as_deref(), Some("rustignore"));
        assert_eq!(get_active_code_context().unwrap(), Some(stored));
        clear_active_code_context().unwrap();
        assert_eq!(get_active_code_context().unwrap(), None);
    }

    #[test]
    fn cli_parser_covers_status_file_and_last_result_commands() {
        assert_eq!(
            parse_bridge_cli_args(&["status".to_string()], None).unwrap(),
            BridgeRequest::Status
        );
        assert_eq!(
            parse_bridge_cli_args(
                &[
                    "transcribe-file".to_string(),
                    "/tmp/sample.wav".to_string(),
                    "code".to_string(),
                ],
                None,
            )
            .unwrap(),
            BridgeRequest::TranscribeFile {
                path: "/tmp/sample.wav".to_string(),
                profile_id: Some("code".to_string()),
            }
        );
        assert_eq!(
            parse_bridge_cli_args(&["copy-last".to_string(), "raw".to_string()], None,).unwrap(),
            BridgeRequest::CopyLast {
                version: LastTextVersion::Raw,
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn cli_protocol_uses_a_token_authenticated_loopback_socket() {
        use std::os::unix::net::UnixStream;
        use std::thread;

        let (mut client, server_stream) = UnixStream::pair().unwrap();
        let token = "a".repeat(32);
        let server_token = token.clone();
        let server = thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(server_stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let envelope: BridgeEnvelope = serde_json::from_str(line.trim()).unwrap();
            assert_eq!(envelope.token, server_token);
            assert_eq!(envelope.request, BridgeRequest::Status);
            let response = BridgeResponse::success(
                &envelope.request,
                Some(serde_json::json!({"recording_state": "idle"})),
            );
            let mut writer = server_stream;
            serde_json::to_writer(&mut writer, &response).unwrap();
            writer.write_all(b"\n").unwrap();
        });

        write_bridge_envelope(
            &mut client,
            &BridgeEnvelope {
                token,
                request: BridgeRequest::Status,
            },
        )
        .unwrap();
        let response = read_bridge_response(BufReader::new(client)).unwrap();
        server.join().unwrap();
        assert!(response.ok);
        assert_eq!(
            response.data.unwrap()["recording_state"],
            serde_json::json!("idle")
        );
    }

    #[test]
    fn desktop_bundle_declares_the_voiceflow_url_scheme_for_macos_and_windows() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_dir.join("tauri.conf.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            config["plugins"]["deep-link"]["desktop"]["schemes"],
            serde_json::json!(["voiceflow"])
        );

        let plist = std::fs::read_to_string(manifest_dir.join("Info.plist")).unwrap();
        assert!(plist.contains("CFBundleURLTypes"));
        assert!(plist.contains("<string>voiceflow</string>"));
    }

    #[test]
    fn quality_summary_includes_correction_rate_and_local_cloud_split() {
        let events = vec![
            QualityEvent::success_with_source(Some("editor"), 100, 20, 130, false),
            QualityEvent::success_with_source(Some("browser"), 120, 30, 160, true),
            QualityEvent::transcription_failure(Some("browser"), 90, true),
            QualityEvent::correction(Some("editor")),
        ];

        let summary = summarize_quality(&events);

        assert_eq!(summary.local_transcriptions, 1);
        assert_eq!(summary.cloud_transcriptions, 2);
        assert_eq!(summary.correction_rate_percent, Some(50.0));
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("raw_text"));
        assert!(!json.contains("final_text"));
        assert!(!json.contains("dictated_content"));
    }
}
