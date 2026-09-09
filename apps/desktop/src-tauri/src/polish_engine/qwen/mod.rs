mod engine;
mod models;

pub use engine::QwenPolishEngine;
pub use models::{get_all_models, is_qwen_model, QwenModelDef};

pub const DEFAULT_POLISH_PROMPT: &str = super::templates::CLEAN_DICTATION_PROMPT;

/// Qwen3 4B needs reasoning to follow layout and self-correction instructions.
/// Other Qwen models retain their existing non-thinking request policy.
pub(crate) fn uses_reasoning(model_id: &str) -> bool {
    model_id == "qwen3-4b"
}
