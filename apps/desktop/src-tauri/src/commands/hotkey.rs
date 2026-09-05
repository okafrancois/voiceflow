//! Hotkey capture and updates use the canonical workflow profile list.

use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::settings::save_settings_internal;
use crate::events::EventName;
use crate::services::product_workflows::{
    apply_profile_registration_transaction, WorkflowProfile, WorkflowProfileRegistrar,
};
use crate::shortcut::{ShortcutManager, ShortcutProfile};
use crate::state::app_state::AppState;

struct HotkeyWorkflowRegistrar<'a>(&'a ShortcutManager);

impl WorkflowProfileRegistrar for HotkeyWorkflowRegistrar<'_> {
    fn register(&mut self, id: &str, profile: &ShortcutProfile) -> Result<(), String> {
        self.0.register_profile(id, profile)
    }

    fn unregister(&mut self, id: &str) -> Result<(), String> {
        self.0.unregister_profile(id)
    }
}

/// Starts hotkey capture for a specific profile key.
///
/// The listener captures the next hotkey combination pressed by the user.
/// When captured, emits `hotkey-captured` event.
#[tauri::command]
pub fn start_hotkey_capture(app: AppHandle, profile_key: String) -> Result<(), String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "app_state_unavailable".to_string())?;
    validate_workflow_profile_exists(&state, &profile_key)?;

    app.try_state::<ShortcutManager>()
        .ok_or_else(|| "shortcut manager not available".to_string())?
        .start_recording_capture()
}

/// Stops hotkey capture and binds to the specified profile key.
///
/// Returns the captured hotkey string, or None if no valid hotkey captured.
#[tauri::command]
pub fn stop_hotkey_capture(app: AppHandle, profile_key: String) -> Result<String, String> {
    let shortcut_manager = app.try_state::<ShortcutManager>();
    let Some(shortcut_manager) = shortcut_manager else {
        return Err("shortcut_manager_not_available".to_string());
    };

    let captured_hotkey = shortcut_manager
        .stop_recording_capture()
        .map_err(|e| format!("stop_hotkey_capture_failed: {}", e))?;

    let hotkey = captured_hotkey.ok_or("no_hotkey_captured")?;

    let app_state = app.try_state::<crate::state::app_state::AppState>();
    let Some(app_state) = app_state else {
        return Err("app_state_unavailable".to_string());
    };

    let mut profiles = app_state.settings.lock().workflow_profiles.clone();
    let profile = profiles
        .iter_mut()
        .find(|profile| profile.id == profile_key)
        .ok_or_else(|| format!("invalid_profile_key: {profile_key}"))?;
    profile.hotkey = hotkey.clone();
    persist_workflow_profiles(&app, &app_state, profiles)?;

    Ok(hotkey)
}

fn persist_workflow_profiles(
    app: &AppHandle,
    state: &AppState,
    requested: Vec<WorkflowProfile>,
) -> Result<(), String> {
    crate::services::product_workflows::validate_profiles(&requested)?;
    let previous = {
        let settings = state.settings.lock();
        crate::services::product_workflows::validate_application_rules(
            &settings.application_rules,
            &requested,
        )?;
        settings.workflow_profiles.clone()
    };

    let manager = app
        .try_state::<ShortcutManager>()
        .ok_or_else(|| "shortcut_manager_not_available".to_string())?;
    apply_profile_registration_transaction(
        &mut HotkeyWorkflowRegistrar(&manager),
        &previous,
        &requested,
    )?;

    {
        let mut settings = state.settings.lock();
        settings.workflow_profiles = requested.clone();
    }

    if let Err(error) = save_settings_internal(app) {
        {
            let mut settings = state.settings.lock();
            settings.workflow_profiles = previous.clone();
        }
        if let Err(rollback_error) = apply_profile_registration_transaction(
            &mut HotkeyWorkflowRegistrar(&manager),
            &requested,
            &previous,
        ) {
            return Err(format!(
                "save_settings_failed: {error}; shortcut rollback failed: {rollback_error}"
            ));
        }
        return Err(format!("save_settings_failed: {error}"));
    }

    let settings = state.settings.lock().clone();
    app.emit(EventName::SETTINGS_CHANGED, settings)
        .map_err(|error| format!("emit_settings_changed_failed: {error}"))?;
    Ok(())
}

fn validate_workflow_profile_exists(state: &AppState, profile_key: &str) -> Result<(), String> {
    let settings = state.settings.lock();
    if settings
        .workflow_profiles
        .iter()
        .any(|profile| profile.id == profile_key)
    {
        Ok(())
    } else {
        Err(format!("unknown_profile_key: {profile_key}"))
    }
}

/// Cancels hotkey capture without saving.
#[tauri::command]
pub fn cancel_hotkey_capture(app: AppHandle) {
    if let Some(shortcut_manager) = app.try_state::<ShortcutManager>() {
        shortcut_manager.cancel_recording_capture();
    } else {
        tracing::error!("shortcut_manager_not_available");
    }
}

/// Peeks at the currently captured hotkey without stopping.
#[tauri::command]
pub fn peek_hotkey_capture(app: AppHandle) -> Option<String> {
    app.try_state::<ShortcutManager>()
        .and_then(|shortcut_manager| shortcut_manager.peek_recording_capture())
}

/// Update a specific profile by key.
///
/// Validates:
/// - dictate: template_id can be None or any template
/// - custom: template_id can be None or any template
/// - hotkey uniqueness across all profiles
#[tauri::command]
pub fn update_shortcut_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    profile: ShortcutProfile,
) -> Result<(), String> {
    validate_profile_constraints(&key, &profile)?;
    let mut profiles = state.settings.lock().workflow_profiles.clone();
    let workflow_profile = profiles
        .iter_mut()
        .find(|workflow_profile| workflow_profile.id == key)
        .ok_or_else(|| format!("unknown_profile_key: {key}"))?;
    workflow_profile.hotkey = profile.hotkey.clone();
    workflow_profile.trigger_mode = profile.trigger_mode;
    workflow_profile.polish_template_id = match profile.action {
        crate::shortcut::ShortcutAction::Record { polish_template_id } => polish_template_id,
    };
    persist_workflow_profiles(&app, &state, profiles)?;

    tracing::info!(key = %key, hotkey = %profile.hotkey, "shortcut_profile_updated");
    Ok(())
}

fn validate_profile_constraints(key: &str, profile: &ShortcutProfile) -> Result<(), String> {
    match &profile.action {
        crate::shortcut::ShortcutAction::Record { .. } => match key {
            "dictate" | "custom" => {}
            _ => return Err(format!("unknown_profile_key: {}", key)),
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcut::{ShortcutAction, ShortcutTriggerMode};

    fn profile_with_template(polish_template_id: Option<&str>) -> ShortcutProfile {
        ShortcutProfile {
            hotkey: "Cmd+Slash".to_string(),
            trigger_mode: ShortcutTriggerMode::Hold,
            action: ShortcutAction::Record {
                polish_template_id: polish_template_id.map(str::to_string),
            },
        }
    }

    #[test]
    fn dictate_accepts_no_polish_or_any_template() {
        assert!(validate_profile_constraints("dictate", &profile_with_template(None)).is_ok());
        assert!(
            validate_profile_constraints("dictate", &profile_with_template(Some("document")))
                .is_ok()
        );
    }

    #[test]
    fn riff_is_no_longer_an_updatable_builtin_profile() {
        assert_eq!(
            validate_profile_constraints("riff", &profile_with_template(Some("filler")))
                .unwrap_err(),
            "unknown_profile_key: riff"
        );
    }
}
