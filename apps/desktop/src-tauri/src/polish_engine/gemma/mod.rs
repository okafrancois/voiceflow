mod engine;
mod models;

pub use engine::GemmaPolishEngine;
pub use models::{get_all_models, is_gemma_model, GemmaModelDef};

pub const DEFAULT_POLISH_PROMPT: &str = super::templates::CLEAN_DICTATION_PROMPT;
