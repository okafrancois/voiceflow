use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

use serde::{Deserialize, Serialize};

pub type WorkflowTriggerMode = crate::shortcut::ShortcutTriggerMode;

const MAX_SELECTION_CHARS: usize = 2_000;
const MAX_CLIPBOARD_CHARS: usize = 2_000;
const MAX_OCR_CHARS: usize = 800;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    Accessibility,
    Clipboard,
    WindowMetadata,
    Ocr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContextCaptureSettings {
    pub application_metadata: bool,
    pub focused_field: bool,
    pub selected_text: bool,
    pub clipboard: bool,
    pub ocr_fallback: bool,
}

impl Default for ContextCaptureSettings {
    fn default() -> Self {
        Self {
            application_metadata: true,
            focused_field: true,
            selected_text: true,
            clipboard: false,
            ocr_fallback: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedContextInput {
    pub application_id: Option<String>,
    pub application_name: Option<String>,
    pub window_title: Option<String>,
    pub focused_field_role: Option<String>,
    pub selected_text: Option<String>,
    pub clipboard_text: Option<String>,
    pub ocr_text: Option<String>,
    pub sources: Vec<ContextSource>,
    pub captured_at_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedContext {
    pub application_id: Option<String>,
    pub application_name: Option<String>,
    pub window_title: Option<String>,
    pub focused_field_role: Option<String>,
    pub selected_text: Option<String>,
    pub clipboard_text: Option<String>,
    pub ocr_text: Option<String>,
    pub sources: Vec<ContextSource>,
    pub captured_at_ms: i64,
}

impl CapturedContext {
    pub fn new(input: CapturedContextInput) -> Self {
        Self {
            application_id: clean_metadata(input.application_id, 256),
            application_name: clean_metadata(input.application_name, 256),
            window_title: clean_metadata(input.window_title, 512),
            focused_field_role: clean_metadata(input.focused_field_role, 128),
            selected_text: clean_content(input.selected_text, MAX_SELECTION_CHARS),
            clipboard_text: clean_content(input.clipboard_text, MAX_CLIPBOARD_CHARS),
            ocr_text: clean_content(input.ocr_text, MAX_OCR_CHARS),
            sources: deduplicate_sources(input.sources),
            captured_at_ms: if input.captured_at_ms == 0 {
                chrono::Utc::now().timestamp_millis()
            } else {
                input.captured_at_ms
            },
        }
    }

    pub fn filtered_by(&self, settings: &ContextCaptureSettings) -> Self {
        let mut filtered = self.clone();
        if !settings.application_metadata {
            filtered.application_id = None;
            filtered.application_name = None;
            filtered.window_title = None;
        }
        if !settings.focused_field {
            filtered.focused_field_role = None;
        }
        if !settings.selected_text {
            filtered.selected_text = None;
        }
        if !settings.clipboard {
            filtered.clipboard_text = None;
        }
        if !settings.ocr_fallback {
            filtered.ocr_text = None;
        }

        filtered.sources.retain(|source| match source {
            ContextSource::Accessibility => settings.focused_field || settings.selected_text,
            ContextSource::Clipboard => settings.clipboard,
            ContextSource::WindowMetadata => settings.application_metadata,
            ContextSource::Ocr => settings.ocr_fallback,
        });
        filtered
    }

    pub fn to_stt_prompt_hint(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(application_name) = self.application_name.as_deref() {
            parts.push(format!("Active application: {application_name}"));
        }
        if let Some(window_title) = self.window_title.as_deref() {
            parts.push(format!("Active window: {window_title}"));
        }
        if let Some(role) = self.focused_field_role.as_deref() {
            parts.push(format!("Focused field role: {role}"));
        }
        if let Some(selection) = self.selected_text.as_deref() {
            parts.push(format!("Selected text: {selection}"));
        }
        if let Some(clipboard) = self.clipboard_text.as_deref() {
            parts.push(format!("Clipboard text: {clipboard}"));
        }
        if let Some(ocr) = self.ocr_text.as_deref() {
            parts.push(format!("Nearby visible text: {ocr}"));
        }
        (!parts.is_empty()).then(|| parts.join(". "))
    }

    pub fn to_polish_reference(&self) -> Option<String> {
        let mut lines = Vec::new();
        if let Some(application_id) = self.application_id.as_deref() {
            lines.push(format!("Application id: {application_id}"));
        }
        if let Some(application_name) = self.application_name.as_deref() {
            lines.push(format!("Application: {application_name}"));
        }
        if let Some(window_title) = self.window_title.as_deref() {
            lines.push(format!("Window title: {window_title}"));
        }
        if let Some(role) = self.focused_field_role.as_deref() {
            lines.push(format!("Focused field role: {role}"));
        }
        if let Some(selection) = self.selected_text.as_deref() {
            lines.push(format!("Selected text: {selection}"));
        }
        if let Some(clipboard) = self.clipboard_text.as_deref() {
            lines.push(format!("Clipboard text: {clipboard}"));
        }
        if let Some(ocr) = self.ocr_text.as_deref() {
            lines.push(format!("Visible text: {ocr}"));
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}

fn clean_metadata(value: Option<String>, max_chars: usize) -> Option<String> {
    value
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|value| value.chars().take(max_chars).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn clean_content(value: Option<String>, max_chars: usize) -> Option<String> {
    value
        .map(|value| value.replace("\r\n", "\n").replace('\r', "\n"))
        .map(|value| value.trim().chars().take(max_chars).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn deduplicate_sources(sources: Vec<ContextSource>) -> Vec<ContextSource> {
    let mut seen = HashSet::new();
    sources
        .into_iter()
        .filter(|source| seen.insert(*source as u8))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputAction {
    Insert,
    Preview,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowProfile {
    pub id: String,
    pub name: String,
    pub hotkey: String,
    pub trigger_mode: WorkflowTriggerMode,
    pub language: Option<String>,
    pub polish_template_id: Option<String>,
    pub translation_target: Option<String>,
    pub output_action: OutputAction,
    pub code_aware: bool,
    #[serde(default)]
    pub protected: bool,
}

impl WorkflowProfile {
    pub fn shortcut_profile(&self) -> crate::shortcut::ShortcutProfile {
        crate::shortcut::ShortcutProfile {
            hotkey: self.hotkey.trim().to_string(),
            trigger_mode: self.trigger_mode,
            action: crate::shortcut::ShortcutAction::Record {
                polish_template_id: self.polish_template_id.clone(),
            },
        }
    }
}

pub fn default_workflow_profiles() -> Vec<WorkflowProfile> {
    vec![WorkflowProfile {
        id: "dictate".to_string(),
        name: "Dictate".to_string(),
        hotkey: crate::shortcut::default_dictate_hotkey().to_string(),
        trigger_mode: WorkflowTriggerMode::Hold,
        language: None,
        polish_template_id: None,
        translation_target: None,
        output_action: OutputAction::Insert,
        code_aware: false,
        protected: true,
    }]
}

pub fn migrate_legacy_profiles(
    legacy: &crate::shortcut::ShortcutProfilesMap,
) -> Vec<WorkflowProfile> {
    let mut profiles = default_workflow_profiles();
    profiles[0].hotkey = legacy.dictate.hotkey.clone();
    profiles[0].trigger_mode = legacy.dictate.trigger_mode;
    profiles[0].polish_template_id =
        shortcut_template_id(&legacy.dictate).or_else(|| shortcut_template_id(&legacy.riff));

    if let Some(custom) = legacy.custom.as_ref() {
        profiles.push(WorkflowProfile {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            hotkey: custom.hotkey.clone(),
            trigger_mode: custom.trigger_mode,
            language: None,
            polish_template_id: shortcut_template_id(custom),
            translation_target: None,
            output_action: OutputAction::Insert,
            code_aware: false,
            protected: false,
        });
    }
    profiles
}

fn shortcut_template_id(profile: &crate::shortcut::ShortcutProfile) -> Option<String> {
    match &profile.action {
        crate::shortcut::ShortcutAction::Record { polish_template_id } => {
            polish_template_id.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationRule {
    pub id: String,
    pub application_id: String,
    pub title_contains: Option<String>,
    pub profile_id: String,
    pub enabled: bool,
}

pub fn resolve_profile<'a>(
    profiles: &'a [WorkflowProfile],
    rules: &[ApplicationRule],
    requested_profile_id: Option<&str>,
    application_id: Option<&str>,
    window_title: Option<&str>,
) -> Option<&'a WorkflowProfile> {
    let matched_rule = application_id.and_then(|application_id| {
        rules.iter().find(|rule| {
            rule.enabled
                && rule.application_id.eq_ignore_ascii_case(application_id)
                && rule.title_contains.as_ref().is_none_or(|needle| {
                    window_title.is_some_and(|title| {
                        title
                            .to_lowercase()
                            .contains(needle.trim().to_lowercase().as_str())
                    })
                })
        })
    });

    matched_rule
        .and_then(|rule| {
            profiles
                .iter()
                .find(|profile| profile.id == rule.profile_id)
        })
        .or_else(|| {
            requested_profile_id.and_then(|id| profiles.iter().find(|profile| profile.id == id))
        })
        .or_else(|| profiles.iter().find(|profile| profile.protected))
        .or_else(|| profiles.first())
}

pub fn resolve_recording_profile<'a>(
    profiles: &'a [WorkflowProfile],
    rules: &[ApplicationRule],
    requested_shortcut: Option<&crate::shortcut::ShortcutProfile>,
    context: &CapturedContext,
) -> Option<&'a WorkflowProfile> {
    let requested_id = requested_shortcut.and_then(|requested| {
        profiles
            .iter()
            .find(|profile| profile.shortcut_profile() == *requested)
            .map(|profile| profile.id.as_str())
    });
    resolve_profile(
        profiles,
        rules,
        requested_id,
        context.application_id.as_deref(),
        context.window_title.as_deref(),
    )
}

pub fn validate_profiles(profiles: &[WorkflowProfile]) -> Result<(), String> {
    if profiles.is_empty() {
        return Err("At least one profile is required".to_string());
    }

    let mut ids = HashSet::new();
    let mut hotkeys = HashSet::new();
    for profile in profiles {
        let id = profile.id.trim();
        if id.is_empty() {
            return Err("Profile id cannot be empty".to_string());
        }
        if !ids.insert(id.to_ascii_lowercase()) {
            return Err(format!("Duplicate profile id: {id}"));
        }

        let hotkey = profile.hotkey.trim().to_ascii_lowercase();
        if hotkey.is_empty() {
            if profile.protected {
                return Err(format!("Protected profile hotkey cannot be empty: {id}"));
            }
            continue;
        }
        if !hotkeys.insert(hotkey.clone()) {
            return Err(format!("Duplicate profile hotkey: {hotkey}"));
        }
    }

    if !profiles.iter().any(|profile| profile.protected) {
        return Err("A protected default profile is required".to_string());
    }
    Ok(())
}

pub trait WorkflowProfileRegistrar {
    fn register(
        &mut self,
        id: &str,
        profile: &crate::shortcut::ShortcutProfile,
    ) -> Result<(), String>;
    fn unregister(&mut self, id: &str) -> Result<(), String>;
}

pub fn apply_profile_registration_transaction(
    registrar: &mut dyn WorkflowProfileRegistrar,
    previous: &[WorkflowProfile],
    requested: &[WorkflowProfile],
) -> Result<(), String> {
    validate_profiles(requested)?;
    let requested_ids = requested
        .iter()
        .filter(|profile| !profile.hotkey.trim().is_empty())
        .map(|profile| profile.id.as_str())
        .collect::<HashSet<_>>();

    let apply_result = (|| {
        for profile in previous
            .iter()
            .filter(|profile| !profile.hotkey.trim().is_empty())
        {
            if !requested_ids.contains(profile.id.as_str()) {
                registrar.unregister(&profile.id)?;
            }
        }
        for profile in requested
            .iter()
            .filter(|profile| !profile.hotkey.trim().is_empty())
        {
            registrar.register(&profile.id, &profile.shortcut_profile())?;
        }
        Ok(())
    })();

    if let Err(error) = apply_result {
        for profile in requested
            .iter()
            .filter(|profile| !profile.hotkey.trim().is_empty())
        {
            let _ = registrar.unregister(&profile.id);
        }
        let rollback_errors = previous
            .iter()
            .filter(|profile| !profile.hotkey.trim().is_empty())
            .filter_map(|profile| {
                registrar
                    .register(&profile.id, &profile.shortcut_profile())
                    .err()
            })
            .collect::<Vec<_>>();
        return if rollback_errors.is_empty() {
            Err(error)
        } else {
            Err(format!(
                "{error}; shortcut rollback failed: {}",
                rollback_errors.join("; ")
            ))
        };
    }

    Ok(())
}

pub fn validate_application_rules(
    rules: &[ApplicationRule],
    profiles: &[WorkflowProfile],
) -> Result<(), String> {
    let profile_ids = profiles
        .iter()
        .map(|profile| profile.id.as_str())
        .collect::<HashSet<_>>();
    let mut rule_ids = HashSet::new();
    for rule in rules {
        let id = rule.id.trim();
        if id.is_empty() {
            return Err("Rule id cannot be empty".to_string());
        }
        if !rule_ids.insert(id.to_ascii_lowercase()) {
            return Err(format!("Duplicate rule id: {id}"));
        }
        if rule.application_id.trim().is_empty() {
            return Err(format!("Rule application id cannot be empty: {id}"));
        }
        if !profile_ids.contains(rule.profile_id.as_str()) {
            return Err(format!("Rule references an unknown profile: {id}"));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceSnippet {
    pub id: String,
    pub spoken_trigger: String,
    pub template: String,
    pub enabled: bool,
}

pub fn validate_snippets(snippets: &[VoiceSnippet]) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut triggers = HashSet::new();
    for snippet in snippets {
        let id = snippet.id.trim();
        let trigger = snippet.spoken_trigger.trim();
        if id.is_empty() || trigger.is_empty() || snippet.template.trim().is_empty() {
            return Err("Snippet id, trigger, and template are required".to_string());
        }
        if !ids.insert(id.to_ascii_lowercase()) {
            return Err(format!("Duplicate snippet id: {id}"));
        }
        if !triggers.insert(trigger.to_ascii_lowercase()) {
            return Err(format!("Duplicate snippet trigger: {trigger}"));
        }

        let mut remainder = snippet.template.as_str();
        while let Some(start) = remainder.find("{{") {
            let variable = &remainder[start..];
            let Some(end) = variable.find("}}") else {
                return Err(format!("Snippet contains an invalid variable: {id}"));
            };
            let variable = &variable[..end + 2];
            if !matches!(variable, "{{date}}" | "{{clipboard}}" | "{{selection}}") {
                return Err(format!(
                    "Snippet contains an unsupported variable: {variable}"
                ));
            }
            remainder = &remainder[start + end + 2..];
        }
    }
    Ok(())
}

pub fn find_matching_snippet<'a>(
    snippets: &'a [VoiceSnippet],
    spoken_text: &str,
) -> Option<&'a VoiceSnippet> {
    let spoken_text = spoken_text.trim();
    snippets.iter().find(|snippet| {
        snippet.enabled
            && snippet
                .spoken_trigger
                .trim()
                .eq_ignore_ascii_case(spoken_text)
    })
}

pub fn expand_matching_snippet(
    snippets: &[VoiceSnippet],
    spoken_text: &str,
    context: &CapturedContext,
    current_date: &str,
) -> Result<Option<String>, String> {
    find_matching_snippet(snippets, spoken_text)
        .map(|snippet| expand_snippet(snippet, context, current_date))
        .transpose()
}

pub fn expand_snippet(
    snippet: &VoiceSnippet,
    context: &CapturedContext,
    current_date: &str,
) -> Result<String, String> {
    if !snippet.enabled {
        return Err("Snippet is disabled".to_string());
    }

    let mut expanded = snippet.template.clone();
    expanded = expanded.replace("{{date}}", current_date);
    if expanded.contains("{{selection}}") {
        let selection = context
            .selected_text
            .as_deref()
            .ok_or_else(|| "Snippet requires selected text".to_string())?;
        expanded = expanded.replace("{{selection}}", selection);
    }
    if expanded.contains("{{clipboard}}") {
        let clipboard = context
            .clipboard_text
            .as_deref()
            .ok_or_else(|| "Snippet requires clipboard text".to_string())?;
        expanded = expanded.replace("{{clipboard}}", clipboard);
    }
    if expanded.contains("{{") {
        return Err("Snippet contains an unsupported variable".to_string());
    }
    Ok(expanded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceActionKind {
    Shorten,
    Translate,
    Reply,
    List,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceActionPreview {
    pub kind: VoiceActionKind,
    pub source_text: String,
    pub result_text: String,
    pub translation_target: Option<String>,
    pub output_action: OutputAction,
}

pub fn build_voice_action_preview(
    kind: VoiceActionKind,
    selected_text: Option<&str>,
    result_text: &str,
    translation_target: Option<String>,
) -> Result<VoiceActionPreview, String> {
    let source_text = selected_text
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "Selected text is required".to_string())?;
    let result_text = result_text.trim();
    if result_text.is_empty() {
        return Err("Voice action returned empty text".to_string());
    }
    if kind == VoiceActionKind::Translate && translation_target.as_deref().is_none_or(str::is_empty)
    {
        return Err("Translation target is required".to_string());
    }

    Ok(VoiceActionPreview {
        kind,
        source_text: source_text.to_string(),
        result_text: result_text.to_string(),
        translation_target,
        output_action: OutputAction::Preview,
    })
}

pub fn build_voice_action_instruction(
    kind: VoiceActionKind,
    translation_target: Option<&str>,
    custom_instruction: Option<&str>,
) -> Result<String, String> {
    match kind {
        VoiceActionKind::Shorten => {
            Ok("Shorten the selected text while preserving its meaning and language.".to_string())
        }
        VoiceActionKind::Translate => translation_target
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(|target| format!("Translate the selected text to {target}. Preserve its meaning."))
            .ok_or_else(|| "Translation target is required".to_string()),
        VoiceActionKind::Reply => {
            Ok("Draft a concise reply to the selected text in its language.".to_string())
        }
        VoiceActionKind::List => {
            Ok("Convert the selected text into a clear structured list.".to_string())
        }
        VoiceActionKind::Custom => custom_instruction
            .map(str::trim)
            .filter(|instruction| !instruction.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "Custom instruction is required".to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryRecord {
    pub raw_text: String,
    pub final_text: String,
    pub inserted_text: String,
    pub application_id: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryJournal {
    last: Option<DeliveryRecord>,
}

impl DeliveryJournal {
    pub fn record(&mut self, record: DeliveryRecord) {
        self.last = Some(record);
    }

    pub fn last(&self) -> Option<&DeliveryRecord> {
        self.last.as_ref()
    }

    pub fn last_raw(&self) -> Option<&str> {
        self.last.as_ref().map(|record| record.raw_text.as_str())
    }

    pub fn last_final(&self) -> Option<&str> {
        self.last.as_ref().map(|record| record.final_text.as_str())
    }
}

#[derive(Debug, Default)]
pub struct WorkflowRuntime {
    latest_context: Mutex<Option<CapturedContext>>,
    latest_preview: Mutex<Option<VoiceActionPreview>>,
    journal: Mutex<DeliveryJournal>,
    pending_delivery: Mutex<Option<(u64, DeliveryRecord)>>,
    active_profile: Mutex<Option<(u64, WorkflowProfile)>>,
    active_task_id: AtomicU64,
}

impl WorkflowRuntime {
    pub fn set_context(&self, context: CapturedContext) {
        *self.latest_context.lock() = Some(context);
    }

    pub fn context(&self) -> Option<CapturedContext> {
        self.latest_context.lock().clone()
    }

    pub fn set_preview(&self, preview: VoiceActionPreview) {
        *self.latest_preview.lock() = Some(preview);
    }

    pub fn preview(&self) -> Option<VoiceActionPreview> {
        self.latest_preview.lock().clone()
    }

    pub fn record_delivery(&self, record: DeliveryRecord) {
        self.journal.lock().record(record);
    }

    pub fn stage_delivery(&self, task_id: u64, record: DeliveryRecord) {
        *self.pending_delivery.lock() = Some((task_id, record));
    }

    pub fn commit_staged_delivery(&self, task_id: u64) -> bool {
        let staged = {
            let mut pending = self.pending_delivery.lock();
            if pending
                .as_ref()
                .is_some_and(|(pending_task_id, _)| *pending_task_id == task_id)
            {
                pending.take().map(|(_, record)| record)
            } else {
                None
            }
        };
        if let Some(record) = staged {
            self.record_delivery(record);
            true
        } else {
            false
        }
    }

    pub fn discard_staged_delivery(&self, task_id: u64) -> bool {
        let mut pending = self.pending_delivery.lock();
        if pending
            .as_ref()
            .is_some_and(|(pending_task_id, _)| *pending_task_id == task_id)
        {
            pending.take();
            true
        } else {
            false
        }
    }

    pub fn last_delivery(&self) -> Option<DeliveryRecord> {
        self.journal.lock().last().cloned()
    }

    pub fn mark_task_active(&self, task_id: u64) {
        self.active_task_id.store(task_id, Ordering::SeqCst);
    }

    pub fn start_profile_session(&self, task_id: u64, profile: WorkflowProfile) {
        *self.active_profile.lock() = Some((task_id, profile));
        self.mark_task_active(task_id);
    }

    pub fn profile_for_task(&self, task_id: u64) -> Option<WorkflowProfile> {
        self.active_profile
            .lock()
            .as_ref()
            .filter(|(active_task_id, _)| *active_task_id == task_id)
            .map(|(_, profile)| profile.clone())
    }

    pub fn active_task_id(&self) -> Option<u64> {
        match self.active_task_id.load(Ordering::SeqCst) {
            0 => None,
            task_id => Some(task_id),
        }
    }

    pub fn clear_active_task(&self, task_id: u64) -> bool {
        let cleared = self
            .active_task_id
            .compare_exchange(task_id, 0, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        if cleared {
            let mut active_profile = self.active_profile.lock();
            if active_profile
                .as_ref()
                .is_some_and(|(active_task_id, _)| *active_task_id == task_id)
            {
                *active_profile = None;
            }
        }
        cleared
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn profile(id: &str, hotkey: &str) -> WorkflowProfile {
        WorkflowProfile {
            id: id.to_string(),
            name: id.to_string(),
            hotkey: hotkey.to_string(),
            trigger_mode: WorkflowTriggerMode::Toggle,
            language: None,
            polish_template_id: None,
            translation_target: None,
            output_action: OutputAction::Insert,
            code_aware: false,
            protected: id == "dictate",
        }
    }

    #[test]
    fn captured_context_normalizes_and_bounds_sensitive_values() {
        let context = CapturedContext::new(CapturedContextInput {
            application_id: Some("  com.example.Editor  ".to_string()),
            application_name: Some(" Editor ".to_string()),
            window_title: Some("  project   —   file.rs ".to_string()),
            focused_field_role: Some(" text-area ".to_string()),
            selected_text: Some("s".repeat(5_000)),
            clipboard_text: Some("c".repeat(5_000)),
            ocr_text: Some("o".repeat(5_000)),
            sources: vec![ContextSource::Accessibility, ContextSource::Clipboard],
            captured_at_ms: 42,
        });

        assert_eq!(
            context.application_id.as_deref(),
            Some("com.example.Editor")
        );
        assert_eq!(context.window_title.as_deref(), Some("project — file.rs"));
        assert_eq!(
            context.selected_text.as_ref().unwrap().chars().count(),
            2_000
        );
        assert_eq!(
            context.clipboard_text.as_ref().unwrap().chars().count(),
            2_000
        );
        assert_eq!(context.ocr_text.as_ref().unwrap().chars().count(), 800);
        assert_eq!(context.captured_at_ms, 42);
    }

    #[test]
    fn context_capture_settings_remove_values_without_opt_in() {
        let context = CapturedContext::new(CapturedContextInput {
            application_id: Some("com.example.Editor".to_string()),
            application_name: Some("Editor".to_string()),
            window_title: Some("notes.txt".to_string()),
            focused_field_role: Some("text-area".to_string()),
            selected_text: Some("selected".to_string()),
            clipboard_text: Some("copied".to_string()),
            ocr_text: Some("visible".to_string()),
            sources: vec![
                ContextSource::WindowMetadata,
                ContextSource::Accessibility,
                ContextSource::Clipboard,
                ContextSource::Ocr,
            ],
            captured_at_ms: 42,
        });

        let filtered = context.filtered_by(&ContextCaptureSettings::default());

        assert_eq!(
            filtered.application_id.as_deref(),
            Some("com.example.Editor")
        );
        assert_eq!(filtered.selected_text.as_deref(), Some("selected"));
        assert!(filtered.clipboard_text.is_none());
        assert!(filtered.ocr_text.is_none());
        assert_eq!(
            filtered.sources,
            vec![ContextSource::WindowMetadata, ContextSource::Accessibility]
        );
    }

    #[test]
    fn selected_and_clipboard_content_preserve_lines_and_indentation() {
        let context = CapturedContext::new(CapturedContextInput {
            selected_text: Some("  fn main() {\r\n    run();\r\n  }  ".to_string()),
            clipboard_text: Some("  first\rsecond  ".to_string()),
            ..CapturedContextInput::default()
        });

        assert_eq!(
            context.selected_text.as_deref(),
            Some("fn main() {\n    run();\n  }")
        );
        assert_eq!(context.clipboard_text.as_deref(), Some("first\nsecond"));
    }

    #[test]
    fn application_rules_select_first_enabled_matching_profile() {
        let profiles = vec![profile("dictate", "Cmd+1"), profile("code", "Cmd+2")];
        let rules = vec![
            ApplicationRule {
                id: "disabled".to_string(),
                application_id: "com.example.Editor".to_string(),
                title_contains: None,
                profile_id: "dictate".to_string(),
                enabled: false,
            },
            ApplicationRule {
                id: "rust".to_string(),
                application_id: "com.example.Editor".to_string(),
                title_contains: Some(".rs".to_string()),
                profile_id: "code".to_string(),
                enabled: true,
            },
        ];

        let resolved = resolve_profile(
            &profiles,
            &rules,
            Some("dictate"),
            Some("com.example.Editor"),
            Some("project — lib.rs"),
        )
        .unwrap();

        assert_eq!(resolved.id, "code");
    }

    #[test]
    fn invalid_rule_reference_falls_back_to_requested_profile() {
        let profiles = vec![profile("dictate", "Cmd+1")];
        let rules = vec![ApplicationRule {
            id: "broken".to_string(),
            application_id: "com.example.Editor".to_string(),
            title_contains: None,
            profile_id: "missing".to_string(),
            enabled: true,
        }];

        let resolved = resolve_profile(
            &profiles,
            &rules,
            Some("dictate"),
            Some("com.example.Editor"),
            None,
        )
        .unwrap();

        assert_eq!(resolved.id, "dictate");
    }

    #[test]
    fn application_rules_do_not_match_when_metadata_capture_is_disabled() {
        let profiles = vec![profile("dictate", "Cmd+1"), profile("code", "Cmd+2")];
        let rules = vec![ApplicationRule {
            id: "editor".to_string(),
            application_id: "com.example.Editor".to_string(),
            title_contains: None,
            profile_id: "code".to_string(),
            enabled: true,
        }];
        let requested = profiles[0].shortcut_profile();

        let resolved = resolve_recording_profile(
            &profiles,
            &rules,
            Some(&requested),
            &CapturedContext::default(),
        )
        .unwrap();

        assert_eq!(resolved.id, "dictate");
    }

    #[test]
    fn snippet_expands_static_and_context_variables() {
        let snippet = VoiceSnippet {
            id: "meeting-note".to_string(),
            spoken_trigger: "meeting note".to_string(),
            template: "{{date}} — {{selection}} — {{clipboard}}".to_string(),
            enabled: true,
        };
        let context = CapturedContext::new(CapturedContextInput {
            selected_text: Some("selected".to_string()),
            clipboard_text: Some("copied".to_string()),
            ..CapturedContextInput::default()
        });

        let expanded = expand_snippet(&snippet, &context, "2026-08-24").unwrap();

        assert_eq!(expanded, "2026-08-24 — selected — copied");
    }

    #[test]
    fn snippet_reports_missing_required_context() {
        let snippet = VoiceSnippet {
            id: "quote".to_string(),
            spoken_trigger: "quote".to_string(),
            template: "{{selection}}".to_string(),
            enabled: true,
        };

        let error = expand_snippet(
            &snippet,
            &CapturedContext::new(CapturedContextInput::default()),
            "2026-08-24",
        )
        .unwrap_err();

        assert_eq!(error, "Snippet requires selected text");
    }

    #[test]
    fn spoken_snippet_flow_expands_before_delivery_and_ignores_other_speech() {
        let snippets = vec![VoiceSnippet {
            id: "meeting-note".to_string(),
            spoken_trigger: "meeting note".to_string(),
            template: "{{date}}: {{selection}}".to_string(),
            enabled: true,
        }];
        let context = CapturedContext::new(CapturedContextInput {
            selected_text: Some("Review P4".to_string()),
            ..CapturedContextInput::default()
        });

        assert_eq!(
            expand_matching_snippet(&snippets, "Meeting Note", &context, "2026-08-24")
                .unwrap()
                .as_deref(),
            Some("2026-08-24: Review P4")
        );
        assert!(
            expand_matching_snippet(&snippets, "ordinary dictation", &context, "2026-08-24")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn profile_validation_rejects_duplicate_ids_and_hotkeys() {
        let profiles = vec![profile("dictate", "Cmd+1"), profile("dictate", "Cmd+2")];
        assert_eq!(
            validate_profiles(&profiles).unwrap_err(),
            "Duplicate profile id: dictate"
        );

        let profiles = vec![profile("dictate", "Cmd+1"), profile("code", "cmd+1")];
        assert_eq!(
            validate_profiles(&profiles).unwrap_err(),
            "Duplicate profile hotkey: cmd+1"
        );
    }

    #[test]
    fn app_only_profile_can_omit_a_shortcut_and_resolve_from_an_application_rule() {
        let profiles = vec![profile("dictate", "Cmd+1"), profile("mail", "")];
        let rules = vec![ApplicationRule {
            id: "mail".to_string(),
            application_id: "com.apple.mail".to_string(),
            title_contains: None,
            profile_id: "mail".to_string(),
            enabled: true,
        }];

        validate_profiles(&profiles).unwrap();
        assert_eq!(
            resolve_profile(&profiles, &rules, None, Some("com.apple.mail"), None)
                .map(|profile| profile.id.as_str()),
            Some("mail")
        );
    }

    #[test]
    fn protected_profile_still_requires_a_shortcut() {
        let profiles = vec![profile("dictate", "")];

        assert_eq!(
            validate_profiles(&profiles).unwrap_err(),
            "Protected profile hotkey cannot be empty: dictate"
        );
    }

    #[test]
    fn legacy_riff_template_migrates_into_dictate_and_custom_is_preserved() {
        let legacy = crate::shortcut::ShortcutProfilesMap {
            dictate: crate::shortcut::ShortcutProfile {
                hotkey: "Cmd+D".to_string(),
                trigger_mode: WorkflowTriggerMode::Hold,
                action: crate::shortcut::ShortcutAction::Record {
                    polish_template_id: None,
                },
            },
            riff: crate::shortcut::ShortcutProfile {
                hotkey: "Cmd+R".to_string(),
                trigger_mode: WorkflowTriggerMode::Toggle,
                action: crate::shortcut::ShortcutAction::Record {
                    polish_template_id: Some("filler".to_string()),
                },
            },
            custom: Some(crate::shortcut::ShortcutProfile {
                hotkey: "Cmd+C".to_string(),
                trigger_mode: WorkflowTriggerMode::DoubleTap,
                action: crate::shortcut::ShortcutAction::Record {
                    polish_template_id: Some("formal".to_string()),
                },
            }),
        };

        let migrated = migrate_legacy_profiles(&legacy);

        assert_eq!(migrated.len(), 2);
        assert_eq!(migrated[0].hotkey, "Cmd+D");
        assert_eq!(migrated[0].polish_template_id.as_deref(), Some("filler"));
        assert_eq!(migrated[1].hotkey, "Cmd+C");
        assert_eq!(migrated[1].trigger_mode, WorkflowTriggerMode::DoubleTap);
        assert_eq!(migrated[1].polish_template_id.as_deref(), Some("formal"));
        assert!(migrated.iter().all(|profile| profile.id != "riff"));
    }

    #[test]
    fn rules_and_snippets_reject_dangling_or_ambiguous_records() {
        let profiles = vec![profile("dictate", "Cmd+1")];
        let broken_rule = ApplicationRule {
            id: "editor".to_string(),
            application_id: "com.example.Editor".to_string(),
            title_contains: None,
            profile_id: "missing".to_string(),
            enabled: true,
        };
        assert_eq!(
            validate_application_rules(&[broken_rule], &profiles).unwrap_err(),
            "Rule references an unknown profile: editor"
        );

        let snippets = vec![VoiceSnippet {
            id: "bad".to_string(),
            spoken_trigger: "bad".to_string(),
            template: "{{unknown}}".to_string(),
            enabled: true,
        }];
        assert_eq!(
            validate_snippets(&snippets).unwrap_err(),
            "Snippet contains an unsupported variable: {{unknown}}"
        );
    }

    #[test]
    fn selected_text_action_requires_source_and_returns_preview() {
        let preview = build_voice_action_preview(
            VoiceActionKind::Shorten,
            Some("A long selected sentence"),
            "A shorter sentence",
            None,
        )
        .unwrap();

        assert_eq!(preview.source_text, "A long selected sentence");
        assert_eq!(preview.result_text, "A shorter sentence");
        assert_eq!(preview.output_action, OutputAction::Preview);

        assert_eq!(
            build_voice_action_preview(VoiceActionKind::Reply, None, "Hello", None).unwrap_err(),
            "Selected text is required"
        );
    }

    #[test]
    fn delivery_journal_exposes_raw_and_final() {
        let mut journal = DeliveryJournal::default();
        journal.record(DeliveryRecord {
            raw_text: "raw words".to_string(),
            final_text: "Final words.".to_string(),
            inserted_text: "Final words.".to_string(),
            application_id: Some("com.example.Editor".to_string()),
            created_at_ms: 100,
        });

        assert_eq!(journal.last_raw(), Some("raw words"));
        assert_eq!(journal.last_final(), Some("Final words."));
    }

    #[test]
    fn workflow_runtime_owns_context_preview_delivery_and_task_state() {
        let runtime = WorkflowRuntime::default();
        runtime.set_context(CapturedContext::new(CapturedContextInput {
            application_id: Some("com.example.Editor".to_string()),
            ..CapturedContextInput::default()
        }));
        runtime.set_preview(
            build_voice_action_preview(
                VoiceActionKind::Shorten,
                Some("long source"),
                "short source",
                None,
            )
            .unwrap(),
        );
        runtime.record_delivery(DeliveryRecord {
            raw_text: "raw".to_string(),
            final_text: "final".to_string(),
            inserted_text: "final".to_string(),
            application_id: Some("com.example.Editor".to_string()),
            created_at_ms: 10,
        });
        runtime.mark_task_active(55);
        runtime.stage_delivery(
            55,
            DeliveryRecord {
                raw_text: "pending raw".to_string(),
                final_text: "pending final".to_string(),
                inserted_text: "pending final".to_string(),
                application_id: None,
                created_at_ms: 11,
            },
        );

        assert_eq!(
            runtime.context().unwrap().application_id.as_deref(),
            Some("com.example.Editor")
        );
        assert_eq!(runtime.preview().unwrap().result_text, "short source");
        assert_eq!(runtime.last_delivery().unwrap().raw_text, "raw");
        assert_eq!(runtime.active_task_id(), Some(55));
        assert!(runtime.clear_active_task(55));
        assert!(runtime.discard_staged_delivery(55));
        assert_eq!(runtime.active_task_id(), None);
        assert_eq!(runtime.preview().unwrap().result_text, "short source");
        assert_eq!(runtime.last_delivery().unwrap().raw_text, "raw");
    }

    #[derive(Default)]
    struct FakeRegistrar {
        registered: HashMap<String, crate::shortcut::ShortcutProfile>,
        fail_registration_for: Option<String>,
    }

    impl WorkflowProfileRegistrar for FakeRegistrar {
        fn register(
            &mut self,
            id: &str,
            profile: &crate::shortcut::ShortcutProfile,
        ) -> Result<(), String> {
            if self.fail_registration_for.as_deref() == Some(id) {
                self.fail_registration_for = None;
                return Err(format!("cannot register {id}"));
            }
            self.registered.insert(id.to_string(), profile.clone());
            Ok(())
        }

        fn unregister(&mut self, id: &str) -> Result<(), String> {
            self.registered.remove(id);
            Ok(())
        }
    }

    #[test]
    fn profile_registration_transaction_applies_unlimited_list_and_removals() {
        let previous = vec![profile("dictate", "Cmd+1"), profile("old", "Cmd+2")];
        let requested = vec![
            profile("dictate", "Cmd+1"),
            profile("code", "Cmd+3"),
            profile("mail", "Cmd+4"),
        ];
        let mut registrar = FakeRegistrar {
            registered: previous
                .iter()
                .map(|item| (item.id.clone(), item.shortcut_profile()))
                .collect(),
            fail_registration_for: None,
        };

        apply_profile_registration_transaction(&mut registrar, &previous, &requested).unwrap();

        assert!(!registrar.registered.contains_key("old"));
        assert!(registrar.registered.contains_key("dictate"));
        assert!(registrar.registered.contains_key("code"));
        assert!(registrar.registered.contains_key("mail"));
    }

    #[test]
    fn profile_registration_transaction_rolls_back_before_settings_change() {
        let previous = vec![profile("dictate", "Cmd+1"), profile("old", "Cmd+2")];
        let requested = vec![profile("dictate", "Cmd+1"), profile("code", "Cmd+3")];
        let mut registrar = FakeRegistrar {
            registered: previous
                .iter()
                .map(|item| (item.id.clone(), item.shortcut_profile()))
                .collect(),
            fail_registration_for: Some("code".to_string()),
        };

        assert!(
            apply_profile_registration_transaction(&mut registrar, &previous, &requested).is_err()
        );

        assert_eq!(registrar.registered.len(), 2);
        assert!(registrar.registered.contains_key("dictate"));
        assert!(registrar.registered.contains_key("old"));
    }

    #[test]
    fn profile_registration_transaction_unregisters_a_shortcut_removed_from_a_profile() {
        let previous = vec![profile("dictate", "Cmd+1"), profile("mail", "Cmd+2")];
        let requested = vec![profile("dictate", "Cmd+1"), profile("mail", "")];
        let mut registrar = FakeRegistrar {
            registered: previous
                .iter()
                .map(|item| (item.id.clone(), item.shortcut_profile()))
                .collect(),
            fail_registration_for: None,
        };

        apply_profile_registration_transaction(&mut registrar, &previous, &requested).unwrap();

        assert!(registrar.registered.contains_key("dictate"));
        assert!(!registrar.registered.contains_key("mail"));
    }

    #[test]
    fn profile_registration_rollback_restores_only_previously_active_shortcuts() {
        let previous = vec![profile("dictate", "Cmd+1"), profile("mail", "Cmd+2")];
        let requested = vec![
            profile("dictate", "Cmd+1"),
            profile("mail", ""),
            profile("code", "Cmd+3"),
        ];
        let mut registrar = FakeRegistrar {
            registered: previous
                .iter()
                .map(|item| (item.id.clone(), item.shortcut_profile()))
                .collect(),
            fail_registration_for: Some("code".to_string()),
        };

        assert!(
            apply_profile_registration_transaction(&mut registrar, &previous, &requested).is_err()
        );

        assert_eq!(registrar.registered.len(), 2);
        assert!(registrar.registered.contains_key("dictate"));
        assert!(registrar.registered.contains_key("mail"));
        assert!(!registrar.registered.contains_key("code"));
    }
}
