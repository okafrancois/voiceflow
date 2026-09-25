//! Apple Intelligence polish (Foundation Models, macOS 26+), reached through
//! the Swift bridge. Built into the OS: no model file and no local runtime.

pub mod bridge;
pub mod engine;
pub mod prompt;

pub use bridge::AppleLlmStatus;
pub use engine::ApplePolishEngine;

/// Polish model id stored in `settings.polish_model`.
pub const APPLE_POLISH_MODEL_ID: &str = "apple-intelligence";
pub const APPLE_POLISH_DISPLAY_NAME: &str = "Apple Intelligence";

pub fn is_apple_model(model_id: &str) -> bool {
    model_id == APPLE_POLISH_MODEL_ID
}
