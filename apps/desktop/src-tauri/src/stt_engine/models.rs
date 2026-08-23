use crate::stt_engine::traits::EngineType;

/// Model file entry for download tracking
#[derive(Debug, Clone)]
pub struct ModelFile {
    pub filename: &'static str,
    pub source_filename: &'static str,
    estimated_bytes: u64,
}

impl ModelFile {
    const SMALL_SUPPORT_FILE_BYTES: u64 = 2 * 1024 * 1024;

    pub const fn new(filename: &'static str, estimated_bytes: u64) -> Self {
        Self {
            filename,
            source_filename: filename,
            estimated_bytes,
        }
    }

    pub const fn from_source(
        filename: &'static str,
        source_filename: &'static str,
        estimated_bytes: u64,
    ) -> Self {
        Self {
            filename,
            source_filename,
            estimated_bytes,
        }
    }

    pub fn estimated_size_bytes(&self) -> u64 {
        self.estimated_bytes
    }

    pub fn minimum_complete_bytes(&self) -> u64 {
        let estimated = self.estimated_size_bytes();
        if estimated <= Self::SMALL_SUPPORT_FILE_BYTES {
            1
        } else {
            estimated * 4 / 5
        }
    }
}

/// Unified model definition for all local STT models
#[derive(Debug, Clone)]
pub struct ModelDefinition {
    pub name: &'static str,
    pub display_name: &'static str,
    pub size_mb: u32,
    pub speed_score: u8,
    pub accuracy_score: u8,
    pub engine_type: EngineType,
    pub repository: Option<&'static str>,
    pub files: &'static [&'static ModelFile],
    pub prefer_lang: &'static [&'static str],
    pub description: &'static str,
}

impl ModelDefinition {
    pub fn whisper_prefix(&self) -> Option<&str> {
        (self.engine_type == EngineType::Whisper)
            .then(|| self.name.strip_prefix("whisper-"))
            .flatten()
    }
}

/// Language codes for which SenseVoice is the preferred engine
pub const SENSEVOICE_PREFERRED_CODES: &[&str] = &["zh", "yue", "ja", "ko", "en"];

// ============================================================================
// Model Definitions
// ============================================================================

/// SenseVoice Small - optimized for CJK + English
pub const SENSE_VOICE_SMALL: ModelDefinition = ModelDefinition {
    name: "sense-voice-small",
    display_name: "SenseVoice Small (229M)",
    size_mb: 229,
    speed_score: 8,
    accuracy_score: 9,
    engine_type: EngineType::SenseVoice,
    repository: Some("csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17"),
    files: &[
        &ModelFile::new("model.int8.onnx", 239_233_841),
        &ModelFile::new("tokens.txt", 315_894),
    ],
    prefer_lang: &["zh", "yue", "ja", "ko", "en"],
    description: "SenseVoice Small for Chinese, Japanese, Korean, Cantonese, and English",
};

/// Whisper Base - general purpose for all languages
pub const WHISPER_BASE: ModelDefinition = ModelDefinition {
    name: "whisper-base",
    display_name: "Whisper Base (279M)",
    size_mb: 279,
    speed_score: 9,
    accuracy_score: 7,
    engine_type: EngineType::Whisper,
    repository: Some("csukuangfj/sherpa-onnx-whisper-base"),
    files: &[
        &ModelFile::new("base-encoder.onnx", 95_087_154),
        &ModelFile::new("base-decoder.onnx", 196_548_998),
        &ModelFile::new("base-tokens.txt", 816_730),
    ],
    prefer_lang: &[], // Empty = all languages
    description: "Whisper Base for all languages, fast and lightweight",
};

/// Whisper Small - better accuracy for all languages
pub const WHISPER_SMALL: ModelDefinition = ModelDefinition {
    name: "whisper-small",
    display_name: "Whisper Small (925M)",
    size_mb: 925,
    speed_score: 7,
    accuracy_score: 8,
    engine_type: EngineType::Whisper,
    repository: Some("csukuangfj/sherpa-onnx-whisper-small"),
    files: &[
        &ModelFile::new("small-encoder.onnx", 409_528_992),
        &ModelFile::new("small-decoder.onnx", 559_127_829),
        &ModelFile::new("small-tokens.txt", 816_730),
    ],
    prefer_lang: &[], // Empty = all languages
    description: "Whisper Small for all languages, better accuracy than Base",
};

/// Whisper Tiny INT8 - smallest multilingual Whisper variant
pub const WHISPER_TINY: ModelDefinition = ModelDefinition {
    name: "whisper-tiny",
    display_name: "Whisper Tiny INT8 (99M)",
    size_mb: 99,
    speed_score: 10,
    accuracy_score: 6,
    engine_type: EngineType::Whisper,
    repository: Some("csukuangfj/sherpa-onnx-whisper-tiny"),
    files: &[
        &ModelFile::from_source("tiny-encoder.onnx", "tiny-encoder.int8.onnx", 12_937_772),
        &ModelFile::from_source("tiny-decoder.onnx", "tiny-decoder.int8.onnx", 89_855_401),
        &ModelFile::new("tiny-tokens.txt", 816_730),
    ],
    prefer_lang: &[],
    description: "Whisper Tiny INT8 for fast multilingual transcription",
};

/// Whisper Medium INT8 - higher-accuracy multilingual Whisper variant
pub const WHISPER_MEDIUM: ModelDefinition = ModelDefinition {
    name: "whisper-medium",
    display_name: "Whisper Medium INT8 (902M)",
    size_mb: 902,
    speed_score: 5,
    accuracy_score: 9,
    engine_type: EngineType::Whisper,
    repository: Some("csukuangfj/sherpa-onnx-whisper-medium"),
    files: &[
        &ModelFile::from_source(
            "medium-encoder.onnx",
            "medium-encoder.int8.onnx",
            374_196_283,
        ),
        &ModelFile::from_source(
            "medium-decoder.onnx",
            "medium-decoder.int8.onnx",
            571_059_257,
        ),
        &ModelFile::new("medium-tokens.txt", 816_730),
    ],
    prefer_lang: &[],
    description: "Whisper Medium INT8 for higher-accuracy multilingual transcription",
};

/// Whisper Large v3 INT8 - highest-accuracy Whisper option
pub const WHISPER_LARGE_V3: ModelDefinition = ModelDefinition {
    name: "whisper-large-v3",
    display_name: "Whisper Large v3 INT8 (1.69G)",
    size_mb: 1_694,
    speed_score: 2,
    accuracy_score: 10,
    engine_type: EngineType::Whisper,
    repository: Some("csukuangfj/sherpa-onnx-whisper-large-v3"),
    files: &[
        &ModelFile::from_source(
            "large-v3-encoder.onnx",
            "large-v3-encoder.int8.onnx",
            766_671_985,
        ),
        &ModelFile::from_source(
            "large-v3-decoder.onnx",
            "large-v3-decoder.int8.onnx",
            1_008_265_203,
        ),
        &ModelFile::new("large-v3-tokens.txt", 816_730),
    ],
    prefer_lang: &[],
    description: "Whisper Large v3 INT8 for maximum multilingual accuracy",
};

/// Whisper Turbo INT8 - Large v3 quality with a smaller decoder
pub const WHISPER_TURBO: ModelDefinition = ModelDefinition {
    name: "whisper-turbo",
    display_name: "Whisper Turbo INT8 (989M)",
    size_mb: 989,
    speed_score: 5,
    accuracy_score: 10,
    engine_type: EngineType::Whisper,
    repository: Some("csukuangfj/sherpa-onnx-whisper-turbo"),
    files: &[
        &ModelFile::from_source("turbo-encoder.onnx", "turbo-encoder.int8.onnx", 674_716_297),
        &ModelFile::from_source("turbo-decoder.onnx", "turbo-decoder.int8.onnx", 361_080_764),
        &ModelFile::new("turbo-tokens.txt", 816_730),
    ],
    prefer_lang: &[],
    description: "Whisper Turbo INT8 for high-accuracy multilingual transcription",
};

/// Qwen3-ASR 0.6B INT8 - high accuracy multilingual ASR via sherpa-onnx
pub const QWEN3_ASR_0_6B_INT8: ModelDefinition = ModelDefinition {
    name: "qwen3-asr-0.6b-int8",
    display_name: "Qwen3-ASR 0.6B INT8 (838M)",
    size_mb: 838,
    speed_score: 6,
    accuracy_score: 9,
    engine_type: EngineType::Qwen3Asr,
    repository: None,
    files: &[
        &ModelFile::new("conv_frontend.onnx", 44_148_281),
        &ModelFile::new("encoder.int8.onnx", 182_491_662),
        &ModelFile::new("decoder.int8.onnx", 755_914_231),
        &ModelFile::new("tokenizer/vocab.json", 2_776_833),
        &ModelFile::new("tokenizer/tokenizer_config.json", 12_487),
        &ModelFile::new("tokenizer/merges.txt", 1_671_853),
    ],
    prefer_lang: &[],
    description: "Qwen3-ASR 0.6B INT8 for high-accuracy multilingual transcription",
};

/// Default model for general use
pub const DEFAULT: &ModelDefinition = &SENSE_VOICE_SMALL;

/// All available local models
pub const ALL: &[&ModelDefinition] = &[
    &WHISPER_TINY,
    &SENSE_VOICE_SMALL,
    &WHISPER_BASE,
    &QWEN3_ASR_0_6B_INT8,
    &WHISPER_MEDIUM,
    &WHISPER_SMALL,
    &WHISPER_TURBO,
    &WHISPER_LARGE_V3,
];

// ============================================================================
// Helper Functions
// ============================================================================

/// Find a model by its name
pub fn find_by_name(name: &str) -> Option<&'static ModelDefinition> {
    ALL.iter().find(|m| m.name == name).copied()
}

/// Check if a language is a SenseVoice-preferred language (based on base language code)
pub fn is_sensevoice_preferred(lang: &str) -> bool {
    let base_lang = lang.split('-').next().unwrap_or(lang);
    SENSEVOICE_PREFERRED_CODES.contains(&base_lang)
}

/// Recommend models by language
///
/// For SenseVoice-preferred languages: returns SenseVoice Small only
/// For other languages: returns Whisper Base only
pub fn recommend_by_language(lang: &str) -> Vec<&'static ModelDefinition> {
    if lang == "auto" {
        // Return all models for auto-detect, sorted by accuracy
        let mut models: Vec<_> = ALL.to_vec();
        models.sort_by_key(|model| std::cmp::Reverse(model.accuracy_score));
        return models;
    }

    let base_lang = lang.split('-').next().unwrap_or(lang);

    // Check if it's a SenseVoice-preferred language
    if SENSEVOICE_PREFERRED_CODES.contains(&base_lang) {
        // For preferred languages, recommend SenseVoice only
        vec![&SENSE_VOICE_SMALL]
    } else {
        // For other languages, recommend Whisper models
        vec![&WHISPER_BASE]
    }
}

/// Get the default model for a given language
///
/// For SenseVoice-preferred languages: returns SenseVoice Small
/// For other languages: returns Whisper Base
pub fn default_for_language(lang: &str) -> &'static ModelDefinition {
    if lang == "auto" {
        return DEFAULT;
    }

    let base_lang = lang.split('-').next().unwrap_or(lang);

    if SENSEVOICE_PREFERRED_CODES.contains(&base_lang) {
        &SENSE_VOICE_SMALL
    } else {
        &WHISPER_BASE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_definitions() {
        assert_eq!(SENSE_VOICE_SMALL.name, "sense-voice-small");
        assert_eq!(SENSE_VOICE_SMALL.speed_score, 8);
        assert_eq!(SENSE_VOICE_SMALL.accuracy_score, 9);
        assert_eq!(SENSE_VOICE_SMALL.engine_type, EngineType::SenseVoice);
        assert_eq!(SENSE_VOICE_SMALL.files.len(), 2);

        assert_eq!(WHISPER_BASE.name, "whisper-base");
        assert_eq!(WHISPER_BASE.speed_score, 9);
        assert_eq!(WHISPER_BASE.accuracy_score, 7);
        assert_eq!(WHISPER_BASE.engine_type, EngineType::Whisper);
        assert_eq!(WHISPER_BASE.files.len(), 3);

        assert_eq!(WHISPER_SMALL.name, "whisper-small");
        assert_eq!(WHISPER_SMALL.speed_score, 7);
        assert_eq!(WHISPER_SMALL.accuracy_score, 8);
        assert_eq!(WHISPER_SMALL.engine_type, EngineType::Whisper);
        assert_eq!(WHISPER_SMALL.files.len(), 3);

        assert_eq!(QWEN3_ASR_0_6B_INT8.name, "qwen3-asr-0.6b-int8");
        assert_eq!(QWEN3_ASR_0_6B_INT8.engine_type, EngineType::Qwen3Asr);
        assert_eq!(QWEN3_ASR_0_6B_INT8.files.len(), 6);

        assert_eq!(ALL.len(), 8);
        assert!(find_by_name("whisper-tiny").is_some());
        assert!(find_by_name("whisper-medium").is_some());
        assert!(find_by_name("whisper-large-v3").is_some());
        assert!(find_by_name("whisper-turbo").is_some());
    }

    #[test]
    fn qwen_auxiliary_files_accept_the_sizes_from_the_official_archive() {
        let merges = QWEN3_ASR_0_6B_INT8
            .files
            .iter()
            .find(|file| file.filename == "tokenizer/merges.txt")
            .unwrap();

        assert!(merges.minimum_complete_bytes() <= 1_671_853);
    }

    #[test]
    fn test_find_by_name() {
        assert!(find_by_name("sense-voice-small").is_some());
        assert!(find_by_name("whisper-base").is_some());
        assert!(find_by_name("whisper-small").is_some());
        assert!(find_by_name("qwen3-asr-0.6b-int8").is_some());
        assert!(find_by_name("unknown").is_none());

        let model = find_by_name("sense-voice-small").unwrap();
        assert_eq!(model.name, "sense-voice-small");
    }

    #[test]
    fn test_is_sensevoice_preferred() {
        // Full codes
        assert!(is_sensevoice_preferred("zh-CN"));
        assert!(is_sensevoice_preferred("zh-TW"));
        assert!(is_sensevoice_preferred("yue-CN"));
        assert!(is_sensevoice_preferred("ja-JP"));
        assert!(is_sensevoice_preferred("ko-KR"));
        assert!(is_sensevoice_preferred("en-US"));

        // Base codes
        assert!(is_sensevoice_preferred("zh"));
        assert!(is_sensevoice_preferred("yue"));
        assert!(is_sensevoice_preferred("ja"));
        assert!(is_sensevoice_preferred("ko"));
        assert!(is_sensevoice_preferred("en"));

        // Non-preferred
        assert!(!is_sensevoice_preferred("es-ES"));
        assert!(!is_sensevoice_preferred("fr-FR"));
    }

    #[test]
    fn test_recommend_by_language_cjk() {
        // Chinese variants
        let zh_models = recommend_by_language("zh");
        assert_eq!(zh_models.len(), 1);
        assert_eq!(zh_models[0].name, "sense-voice-small");

        let zh_cn_models = recommend_by_language("zh-CN");
        assert_eq!(zh_cn_models.len(), 1);
        assert_eq!(zh_cn_models[0].name, "sense-voice-small");

        // Japanese
        let ja_models = recommend_by_language("ja");
        assert_eq!(ja_models.len(), 1);
        assert_eq!(ja_models[0].name, "sense-voice-small");

        // Korean
        let ko_models = recommend_by_language("ko");
        assert_eq!(ko_models.len(), 1);
        assert_eq!(ko_models[0].name, "sense-voice-small");

        // Cantonese
        let yue_models = recommend_by_language("yue");
        assert_eq!(yue_models.len(), 1);
        assert_eq!(yue_models[0].name, "sense-voice-small");
    }

    #[test]
    fn test_recommend_by_language_non_preferred() {
        // English is now SenseVoice-preferred
        let en_models = recommend_by_language("en");
        assert_eq!(en_models.len(), 1);
        assert_eq!(en_models[0].name, "sense-voice-small");

        // Spanish
        let es_models = recommend_by_language("es");
        assert_eq!(es_models.len(), 1);
        assert_eq!(es_models[0].name, "whisper-base");

        // French
        let fr_models = recommend_by_language("fr");
        assert_eq!(fr_models.len(), 1);
        assert_eq!(fr_models[0].name, "whisper-base");
    }

    #[test]
    fn test_recommend_by_language_auto() {
        let auto_models = recommend_by_language("auto");
        assert_eq!(auto_models.len(), 8);
        // Should be sorted by accuracy descending
        assert!(auto_models[0].accuracy_score >= auto_models[1].accuracy_score);
        assert!(auto_models[1].accuracy_score >= auto_models[2].accuracy_score);
        assert!(auto_models[2].accuracy_score >= auto_models[3].accuracy_score);
    }

    #[test]
    fn test_default_for_language() {
        // SenseVoice-preferred languages should default to SenseVoice
        assert_eq!(default_for_language("zh").name, "sense-voice-small");
        assert_eq!(default_for_language("zh-CN").name, "sense-voice-small");
        assert_eq!(default_for_language("ja").name, "sense-voice-small");
        assert_eq!(default_for_language("ko").name, "sense-voice-small");
        assert_eq!(default_for_language("yue").name, "sense-voice-small");
        assert_eq!(default_for_language("en").name, "sense-voice-small");
        assert_eq!(default_for_language("en-US").name, "sense-voice-small");

        // Other languages should default to Whisper Base
        assert_eq!(default_for_language("es").name, "whisper-base");
        assert_eq!(default_for_language("fr").name, "whisper-base");

        // Auto should use global default
        assert_eq!(default_for_language("auto").name, DEFAULT.name);
    }
}
