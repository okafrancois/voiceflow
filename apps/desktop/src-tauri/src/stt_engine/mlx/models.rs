use crate::stt_engine::models::{ModelDefinition, ModelFile};
use crate::stt_engine::EngineType;

pub const WHISPER_TURBO: ModelDefinition = ModelDefinition {
    name: "mlx-whisper-turbo",
    display_name: "Whisper Turbo · MLX",
    size_mb: 1544,
    speed_score: 0,
    accuracy_score: 0,
    engine_type: EngineType::Mlx,
    repository: Some("mlx-community/whisper-large-v3-turbo-asr-fp16"),
    files: &[
        &ModelFile::new("config.json", 1301),
        &ModelFile::new("model.safetensors", 1_613_977_443),
        &ModelFile::new("model.safetensors.index.json", 37_633),
        &ModelFile::new("added_tokens.json", 34_648),
        &ModelFile::new("generation_config.json", 3772),
        &ModelFile::new("merges.txt", 493_869),
        &ModelFile::new("normalizer.json", 52_666),
        &ModelFile::new("preprocessor_config.json", 340),
        &ModelFile::new("special_tokens_map.json", 2186),
        &ModelFile::new("tokenizer.json", 2_710_337),
        &ModelFile::new("tokenizer_config.json", 282_843),
        &ModelFile::new("vocab.json", 1_036_558),
    ],
    prefer_lang: &[],
    description: "Multilingual Whisper transcription on Apple Silicon with the managed MLX engine",
    built_in: false,
};

pub const PARAKEET_V3: ModelDefinition = ModelDefinition {
    name: "mlx-parakeet-v3",
    display_name: "Parakeet v3 · MLX",
    size_mb: 2393,
    speed_score: 0,
    accuracy_score: 0,
    engine_type: EngineType::Mlx,
    repository: Some("mlx-community/parakeet-tdt-0.6b-v3"),
    files: &[
        &ModelFile::new("config.json", 244_093),
        &ModelFile::new("model.safetensors", 2_508_288_736),
        &ModelFile::new("tokenizer.model", 360_916),
        &ModelFile::new("tokenizer.vocab", 101_024),
        &ModelFile::new("vocab.txt", 46_772),
    ],
    prefer_lang: &[
        "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv", "lt",
        "mt", "pl", "pt", "ro", "ru", "sk", "sl", "es", "sv", "uk",
    ],
    description: "NVIDIA Parakeet v3 transcription for 25 European languages on Apple Silicon",
    built_in: false,
};

pub const QWEN3_ASR: ModelDefinition = ModelDefinition {
    name: "mlx-qwen3-asr-0.6b",
    display_name: "Qwen3-ASR 0.6B · MLX",
    size_mb: 964,
    speed_score: 0,
    accuracy_score: 0,
    engine_type: EngineType::Mlx,
    repository: Some("mlx-community/Qwen3-ASR-0.6B-8bit"),
    files: &[
        &ModelFile::new("config.json", 7187),
        &ModelFile::new("model.safetensors", 1_006_229_426),
        &ModelFile::new("model.safetensors.index.json", 71_815),
        &ModelFile::new("chat_template.json", 1161),
        &ModelFile::new("generation_config.json", 142),
        &ModelFile::new("merges.txt", 1_671_853),
        &ModelFile::new("preprocessor_config.json", 330),
        &ModelFile::new("tokenizer_config.json", 12_487),
        &ModelFile::new("vocab.json", 2_776_833),
    ],
    prefer_lang: &[
        "ar", "cs", "da", "de", "el", "en", "es", "fa", "fi", "fr", "hi", "hu", "id", "it", "ja",
        "ko", "mk", "ms", "nl", "pl", "pt", "ro", "ru", "sv", "th", "tr", "uk", "vi", "yue", "zh",
    ],
    description: "Multilingual Qwen3-ASR transcription with the managed MLX engine",
    built_in: false,
};

pub fn revision(name: &str) -> Option<&'static str> {
    match name {
        "mlx-whisper-turbo" => Some("624c19c9af5603fa73b83bce14d4aeea96156d18"),
        "mlx-parakeet-v3" => Some("ed2b7e8c15f9aaa0b5772e2efb986255eaef7e15"),
        "mlx-qwen3-asr-0.6b" => Some("89e96d92ba34aca20b3e29fb10cc284097d1219f"),
        _ => None,
    }
}
