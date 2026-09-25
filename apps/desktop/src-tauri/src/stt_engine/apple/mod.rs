//! Apple SpeechAnalyzer engine (macOS 26+), reached through a Swift bridge.

pub mod bridge;
pub mod consumer;
pub mod engine;

pub use bridge::AppleSpeechStatus;
pub use consumer::AppleStreamingConsumer;
pub use engine::AppleSpeechEngine;
