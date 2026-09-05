use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::services::platform_quality::CodeContext;

pub const CONTEXT_TTL_MS: i64 = 5 * 60 * 1_000;
pub const MAX_IDENTIFIERS: usize = 64;
pub const MAX_IDENTIFIER_CHARS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedCodeContext {
    pub context: CodeContext,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VibeCodingState {
    Disabled,
    WaitingForEditor,
    Ready,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VibeCodingStatus {
    pub enabled: bool,
    pub context_active: bool,
    pub state: VibeCodingState,
    pub language: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub workspace: Option<String>,
    pub editor: Option<String>,
    pub identifiers: Vec<String>,
    pub updated_at_ms: Option<i64>,
    pub expires_at_ms: Option<i64>,
}

pub fn enrich_context(mut context: CodeContext) -> CodeContext {
    if context.language.is_none() {
        context.language = context
            .file_path
            .as_deref()
            .and_then(infer_language_from_path)
            .map(str::to_string);
    }

    let mut candidates = context.identifiers;
    if let Some(symbol) = context.symbol.as_ref() {
        candidates.push(symbol.clone());
    }
    if let Some(file_stem) = context
        .file_path
        .as_deref()
        .and_then(|path| Path::new(path).file_stem())
        .and_then(|value| value.to_str())
    {
        candidates.push(file_stem.to_string());
    }
    context.identifiers = sanitize_identifiers(candidates);
    context
}

pub fn infer_language_from_path(path: &str) -> Option<&'static str> {
    let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "ts" => Some("typescript"),
        "tsx" => Some("typescriptreact"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "jsx" => Some("javascriptreact"),
        "rs" => Some("rust"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        "c" | "h" => Some("c"),
        "cc" | "cpp" | "cxx" | "hpp" => Some("cpp"),
        "cs" => Some("csharp"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "sh" | "bash" | "zsh" => Some("shellscript"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "md" | "mdx" => Some("markdown"),
        "html" => Some("html"),
        "css" => Some("css"),
        "sql" => Some("sql"),
        _ => None,
    }
}

pub fn recognize_editor(editor_id: Option<&str>) -> Option<&'static str> {
    let id = editor_id?.trim().to_ascii_lowercase();
    if id.contains("cursor") || id.contains("230313mzl4w4u92") {
        Some("Cursor")
    } else if id.contains("windsurf") || id.contains("exafunction") {
        Some("Windsurf")
    } else if id.contains("vscode") || id.contains("visual studio code") {
        Some("VS Code")
    } else if id.contains("zed") {
        Some("Zed")
    } else if id.contains("xcode") || id.contains("com.apple.dt") {
        Some("Xcode")
    } else if [
        "jetbrains",
        "intellij",
        "pycharm",
        "webstorm",
        "rustrover",
        "goland",
        "clion",
    ]
    .iter()
    .any(|candidate| id.contains(candidate))
    {
        Some("JetBrains")
    } else {
        None
    }
}

pub fn context_for_recording(
    enabled: bool,
    active: Option<&TimedCodeContext>,
    application_id: Option<&str>,
    now_ms: i64,
) -> Option<CodeContext> {
    let active = active.filter(|active| {
        enabled
            && !context_is_empty(&active.context)
            && now_ms.saturating_sub(active.updated_at_ms) <= CONTEXT_TTL_MS
            && editor_matches_application(&active.context, application_id)
    })?;
    Some(active.context.clone())
}

pub fn build_status(
    enabled: bool,
    active: Option<&TimedCodeContext>,
    now_ms: i64,
) -> VibeCodingStatus {
    let fresh = active.filter(|value| {
        !context_is_empty(&value.context)
            && now_ms.saturating_sub(value.updated_at_ms) <= CONTEXT_TTL_MS
    });
    let state = if !enabled {
        VibeCodingState::Disabled
    } else if fresh.is_some() {
        VibeCodingState::Ready
    } else if active.is_some() {
        VibeCodingState::Stale
    } else {
        VibeCodingState::WaitingForEditor
    };
    let displayed = fresh.or(active);
    let context = displayed.map(|value| &value.context);
    VibeCodingStatus {
        enabled,
        context_active: enabled && fresh.is_some(),
        state,
        language: context.and_then(|value| value.language.clone()),
        file_path: context.and_then(|value| value.file_path.clone()),
        file_name: context
            .and_then(|value| value.file_path.as_deref())
            .and_then(|path| Path::new(path).file_name())
            .and_then(|value| value.to_str())
            .map(str::to_string),
        workspace: context.and_then(|value| value.workspace.clone()),
        editor: context
            .and_then(|value| recognize_editor(value.editor_id.as_deref()))
            .map(str::to_string),
        identifiers: context
            .map(|value| value.identifiers.clone())
            .unwrap_or_default(),
        updated_at_ms: displayed.map(|value| value.updated_at_ms),
        expires_at_ms: displayed.map(|value| value.updated_at_ms + CONTEXT_TTL_MS),
    }
}

fn sanitize_identifiers(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let cleaned = value
                .chars()
                .filter(|character| !character.is_control())
                .take(MAX_IDENTIFIER_CHARS)
                .collect::<String>();
            let cleaned = cleaned.trim().to_string();
            (!cleaned.is_empty() && seen.insert(cleaned.clone())).then_some(cleaned)
        })
        .take(MAX_IDENTIFIERS)
        .collect()
}

fn context_is_empty(context: &CodeContext) -> bool {
    context.language.is_none()
        && context.file_path.is_none()
        && context.symbol.is_none()
        && context.editor_id.is_none()
        && context.workspace.is_none()
        && context.identifiers.is_empty()
}

fn editor_matches_application(context: &CodeContext, application_id: Option<&str>) -> bool {
    let Some(editor_id) = context.editor_id.as_deref() else {
        return false;
    };
    let Some(application_id) = application_id else {
        return false;
    };
    match (
        recognize_editor(Some(editor_id)),
        recognize_editor(Some(application_id)),
    ) {
        (Some(expected), Some(actual)) => expected == actual,
        _ => editor_id.eq_ignore_ascii_case(application_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::platform_quality::CodeContext;

    fn cursor_context() -> CodeContext {
        CodeContext {
            language: None,
            file_path: Some("src/components/HTTPServer.tsx".to_string()),
            symbol: Some("HTTPServer".to_string()),
            editor_id: Some("Cursor".to_string()),
            workspace: Some("voiceflow".to_string()),
            identifiers: vec![
                "HTTPServer".to_string(),
                "createVoiceFlow".to_string(),
                "HTTPServer".to_string(),
            ],
        }
    }

    #[test]
    fn enrichment_recognizes_language_editor_and_unique_identifiers() {
        let context = enrich_context(cursor_context());

        assert_eq!(context.language.as_deref(), Some("typescriptreact"));
        assert_eq!(
            recognize_editor(context.editor_id.as_deref()),
            Some("Cursor")
        );
        assert_eq!(context.identifiers, vec!["HTTPServer", "createVoiceFlow"]);
    }

    #[test]
    fn scoped_context_expires_and_never_leaks_to_another_application() {
        let context = enrich_context(cursor_context());
        let captured = TimedCodeContext {
            context,
            updated_at_ms: 1_000,
        };

        assert!(context_for_recording(
            true,
            Some(&captured),
            Some("com.todesktop.230313mzl4w4u92"),
            1_000 + CONTEXT_TTL_MS,
        )
        .is_some());
        assert!(
            context_for_recording(true, Some(&captured), Some("com.apple.TextEdit"), 1_001,)
                .is_none()
        );
        assert!(context_for_recording(
            true,
            Some(&captured),
            Some("com.todesktop.230313mzl4w4u92"),
            1_001 + CONTEXT_TTL_MS,
        )
        .is_none());
        assert!(context_for_recording(
            false,
            Some(&captured),
            Some("com.todesktop.230313mzl4w4u92"),
            1_001,
        )
        .is_none());
    }

    #[test]
    fn status_distinguishes_enabled_without_context_from_ready_context() {
        let empty = build_status(true, None, 10_000);
        assert!(empty.enabled);
        assert!(!empty.context_active);
        assert_eq!(empty.state, VibeCodingState::WaitingForEditor);

        let context = TimedCodeContext {
            context: enrich_context(cursor_context()),
            updated_at_ms: 9_000,
        };
        let ready = build_status(true, Some(&context), 10_000);
        assert!(ready.context_active);
        assert_eq!(ready.state, VibeCodingState::Ready);
        assert_eq!(ready.file_name.as_deref(), Some("HTTPServer.tsx"));
        assert_eq!(ready.editor.as_deref(), Some("Cursor"));
    }

    #[test]
    fn identifiers_are_bounded_before_entering_prompts() {
        let mut context = cursor_context();
        context.identifiers = (0..80).map(|index| format!("identifier_{index}")).collect();
        context.identifiers.push("x".repeat(200));

        let context = enrich_context(context);
        assert_eq!(context.identifiers.len(), MAX_IDENTIFIERS);
        assert!(context
            .identifiers
            .iter()
            .all(|identifier| identifier.chars().count() <= MAX_IDENTIFIER_CHARS));
    }
}
