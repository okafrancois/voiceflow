use crate::history::HistoryFilter;
use crate::permissions::{check_permission, PermissionKind, PermissionStatus};
use crate::state::app_state::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeReadiness {
    Ready,
    MicrophoneRequired,
    PermissionsRequired,
    ModelRequired,
    CloudConfigurationRequired,
}

#[derive(Serialize)]
pub struct RecoveryResult {
    id: String,
    raw_text: String,
    final_text: String,
    can_copy_raw: bool,
    can_copy_final: bool,
    delivery_failed: bool,
    error: Option<String>,
}

#[derive(Serialize)]
pub struct DictationHome {
    readiness: HomeReadiness,
    setup_path: Option<&'static str>,
    hotkey: String,
    trigger_mode: crate::shortcut::ShortcutTriggerMode,
    is_cloud: bool,
    last_result: Option<RecoveryResult>,
}

fn readiness(
    microphone: bool,
    keyboard_permissions: bool,
    provider_ready: bool,
    cloud: bool,
) -> HomeReadiness {
    if !microphone {
        HomeReadiness::MicrophoneRequired
    } else if !keyboard_permissions {
        HomeReadiness::PermissionsRequired
    } else if !provider_ready && cloud {
        HomeReadiness::CloudConfigurationRequired
    } else if !provider_ready {
        HomeReadiness::ModelRequired
    } else {
        HomeReadiness::Ready
    }
}

#[tauri::command]
pub fn get_dictation_home(state: State<'_, AppState>) -> Result<DictationHome, String> {
    let settings = state.settings.lock().clone();
    let cloud = settings.cloud_stt_enabled;
    let provider_ready = if cloud {
        let config = settings.get_active_cloud_stt_config();
        !config.api_key.trim().is_empty()
            && (settings.active_cloud_stt_provider != "volcengine-streaming"
                || !config.app_id.trim().is_empty())
    } else {
        let (engine, model) = state
            .engine_manager
            .resolve_available_model(&settings.model, &settings.stt_engine_language);
        state.engine_manager.is_model_downloaded(engine, &model)
    };
    let ready = readiness(
        check_permission(PermissionKind::Microphone) == PermissionStatus::Granted
            && crate::sensors::setup_diagnostics::probe_microphone(0).ready,
        check_permission(PermissionKind::Accessibility) == PermissionStatus::Granted
            && check_permission(PermissionKind::InputMonitoring) == PermissionStatus::Granted,
        provider_ready,
        cloud,
    );
    let setup_path = match ready {
        HomeReadiness::Ready => None,
        HomeReadiness::MicrophoneRequired | HomeReadiness::PermissionsRequired => {
            Some("/permission")
        }
        HomeReadiness::ModelRequired => Some("/private-ai"),
        HomeReadiness::CloudConfigurationRequired => Some("/cloud"),
    };
    let last_result = state
        .history_store
        .lock()
        .get_history(&HistoryFilter {
            search: None,
            engine: None,
            status: None,
            date_from: None,
            date_to: None,
            limit: Some(1),
            offset: Some(0),
        })?
        .into_iter()
        .next()
        .map(|entry| RecoveryResult {
            can_copy_raw: !entry.raw_text.trim().is_empty(),
            can_copy_final: !entry.final_text.trim().is_empty(),
            delivery_failed: matches!(entry.delivery_status.as_str(), "failed" | "copy_failed"),
            id: entry.id,
            raw_text: entry.raw_text,
            final_text: entry.final_text,
            error: entry.error,
        });
    let trigger_mode = settings
        .workflow_profiles
        .iter()
        .find(|profile| profile.id == "dictate")
        .map(|profile| profile.trigger_mode)
        .unwrap_or(crate::shortcut::ShortcutTriggerMode::Hold);
    Ok(DictationHome {
        readiness: ready,
        setup_path,
        hotkey: settings.get_dictate_hotkey(),
        trigger_mode,
        is_cloud: cloud,
        last_result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microphone_is_required_even_when_cloud_is_configured() {
        assert_eq!(
            readiness(false, true, true, true),
            HomeReadiness::MicrophoneRequired
        );
    }

    #[test]
    fn readiness_selects_the_setup_action_from_backend_evidence() {
        assert_eq!(
            readiness(true, false, true, false),
            HomeReadiness::PermissionsRequired
        );
        assert_eq!(
            readiness(true, true, false, false),
            HomeReadiness::ModelRequired
        );
        assert_eq!(
            readiness(true, true, false, true),
            HomeReadiness::CloudConfigurationRequired
        );
        assert_eq!(readiness(true, true, true, false), HomeReadiness::Ready);
    }
}
