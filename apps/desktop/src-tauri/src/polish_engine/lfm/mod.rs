mod engine;
mod models;

pub use engine::LfmPolishEngine;
pub use models::{get_all_models, is_lfm_model, LfmModelDef};

pub const DEFAULT_POLISH_PROMPT: &str = super::templates::CLEAN_DICTATION_PROMPT;
