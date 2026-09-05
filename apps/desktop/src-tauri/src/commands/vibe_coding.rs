use tauri::{AppHandle, Manager, State};

use crate::services::platform_quality::{
    clear_active_code_context, get_active_code_context_snapshot,
};
use crate::services::vibe_coding::{build_status, VibeCodingStatus};
use crate::state::app_state::AppState;

#[tauri::command]
pub fn get_vibe_coding_status(state: State<'_, AppState>) -> Result<VibeCodingStatus, String> {
    status(&state)
}

#[tauri::command]
pub fn set_vibe_coding_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<VibeCodingStatus, String> {
    crate::commands::settings::update_settings(
        app.clone(),
        state,
        "vibe_coding_enabled".to_string(),
        serde_json::Value::Bool(enabled),
    )?;
    if !enabled {
        clear_active_code_context()?;
    }
    status(&app.state::<AppState>())
}

pub fn status(state: &AppState) -> Result<VibeCodingStatus, String> {
    let enabled = state.settings.lock().vibe_coding_enabled;
    let context = get_active_code_context_snapshot()?;
    Ok(build_status(
        enabled,
        context.as_ref(),
        chrono::Utc::now().timestamp_millis(),
    ))
}
