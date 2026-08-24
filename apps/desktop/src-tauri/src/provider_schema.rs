use serde::Serialize;

use crate::polish_engine::{ANTHROPIC_MESSAGES_ENDPOINT, OPENAI_CHAT_COMPLETIONS_ENDPOINT};
use crate::stt_engine::cloud::aliyun_stream::{ALIYUN_REALTIME_ENDPOINT, ALIYUN_REALTIME_MODEL};
use crate::stt_engine::cloud::elevenlabs::{
    ELEVENLABS_REALTIME_ENDPOINT, ELEVENLABS_REALTIME_MODEL,
};
use crate::stt_engine::cloud::volcengine_streaming::{
    DEFAULT_VOLCENGINE_RESOURCE_ID, URL_BIGMODEL_NOSTREAM,
};

#[derive(Debug, Clone, Serialize)]
pub struct ProviderFieldSchema {
    pub name: &'static str,
    pub key: &'static str,
    pub required: bool,
    pub default_value: &'static str,
    pub example: &'static str,
    pub secret: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderSchema {
    pub id: &'static str,
    pub name: &'static str,
    pub fields: &'static [ProviderFieldSchema],
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudProviderSchemas {
    pub stt: &'static [ProviderSchema],
    pub polish: &'static [ProviderSchema],
}

pub static STT_SCHEMAS: &[ProviderSchema] = &[
    ProviderSchema {
        id: "volcengine-streaming",
        name: "Volcengine Streaming",
        fields: &[
            ProviderFieldSchema {
                name: "App ID",
                key: "app_id",
                required: true,
                default_value: "",
                example: "1234567890",
                secret: false,
            },
            ProviderFieldSchema {
                name: "Access Token",
                key: "api_key",
                required: true,
                default_value: "",
                example: "xxxx.xxxx.xxxx",
                secret: true,
            },
            ProviderFieldSchema {
                name: "Base URL",
                key: "base_url",
                required: false,
                default_value: URL_BIGMODEL_NOSTREAM,
                example: URL_BIGMODEL_NOSTREAM,
                secret: false,
            },
            ProviderFieldSchema {
                name: "Resource ID",
                key: "model",
                required: false,
                default_value: DEFAULT_VOLCENGINE_RESOURCE_ID,
                example: DEFAULT_VOLCENGINE_RESOURCE_ID,
                secret: false,
            },
        ],
    },
    ProviderSchema {
        id: "aliyun-stream",
        name: "Aliyun Realtime",
        fields: &[
            ProviderFieldSchema {
                name: "API Key",
                key: "api_key",
                required: true,
                default_value: "",
                example: "sk-xxxx.xxxx.xxxx",
                secret: true,
            },
            ProviderFieldSchema {
                name: "Base URL",
                key: "base_url",
                required: false,
                default_value: ALIYUN_REALTIME_ENDPOINT,
                example: ALIYUN_REALTIME_ENDPOINT,
                secret: false,
            },
            ProviderFieldSchema {
                name: "Model",
                key: "model",
                required: false,
                default_value: ALIYUN_REALTIME_MODEL,
                example: ALIYUN_REALTIME_MODEL,
                secret: false,
            },
        ],
    },
    ProviderSchema {
        id: "elevenlabs",
        name: "ElevenLabs",
        fields: &[
            ProviderFieldSchema {
                name: "API Key",
                key: "api_key",
                required: true,
                default_value: "",
                example: "sk_xxxx.xxxx.xxxx",
                secret: true,
            },
            ProviderFieldSchema {
                name: "Base URL",
                key: "base_url",
                required: false,
                default_value: ELEVENLABS_REALTIME_ENDPOINT,
                example: ELEVENLABS_REALTIME_ENDPOINT,
                secret: false,
            },
            ProviderFieldSchema {
                name: "Model",
                key: "model",
                required: false,
                default_value: ELEVENLABS_REALTIME_MODEL,
                example: ELEVENLABS_REALTIME_MODEL,
                secret: false,
            },
        ],
    },
];

pub static POLISH_SCHEMAS: &[ProviderSchema] = &[
    ProviderSchema {
        id: "anthropic",
        name: "Anthropic",
        fields: &[
            ProviderFieldSchema {
                name: "API Key",
                key: "api_key",
                required: true,
                default_value: "",
                example: "sk-ant-xxxx.xxxx.xxxx",
                secret: true,
            },
            ProviderFieldSchema {
                name: "Base URL",
                key: "base_url",
                required: false,
                default_value: ANTHROPIC_MESSAGES_ENDPOINT,
                example: ANTHROPIC_MESSAGES_ENDPOINT,
                secret: false,
            },
            ProviderFieldSchema {
                name: "Model",
                key: "model",
                required: true,
                default_value: "",
                example: "claude-sonnet-4-20250514",
                secret: false,
            },
        ],
    },
    ProviderSchema {
        id: "openai",
        name: "OpenAI",
        fields: &[
            ProviderFieldSchema {
                name: "API Key",
                key: "api_key",
                required: true,
                default_value: "",
                example: "sk-xxxx.xxxx.xxxx",
                secret: true,
            },
            ProviderFieldSchema {
                name: "Base URL",
                key: "base_url",
                required: false,
                default_value: OPENAI_CHAT_COMPLETIONS_ENDPOINT,
                example: OPENAI_CHAT_COMPLETIONS_ENDPOINT,
                secret: false,
            },
            ProviderFieldSchema {
                name: "Model",
                key: "model",
                required: true,
                default_value: "",
                example: "gpt-4.1",
                secret: false,
            },
        ],
    },
];

pub fn get_schemas() -> CloudProviderSchemas {
    CloudProviderSchemas {
        stt: STT_SCHEMAS,
        polish: POLISH_SCHEMAS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_provider_ids_are_exact() {
        let schemas = get_schemas();
        let stt_ids = schemas
            .stt
            .iter()
            .map(|schema| schema.id)
            .collect::<Vec<_>>();
        let polish_ids = schemas
            .polish
            .iter()
            .map(|schema| schema.id)
            .collect::<Vec<_>>();

        assert_eq!(
            stt_ids,
            ["volcengine-streaming", "aliyun-stream", "elevenlabs"]
        );
        assert_eq!(polish_ids, ["anthropic", "openai"]);
    }

    #[test]
    fn volcengine_schema_defaults_to_bigmodel_nostream() {
        let schema = STT_SCHEMAS
            .iter()
            .find(|schema| schema.id == "volcengine-streaming")
            .unwrap();
        let base_url = schema
            .fields
            .iter()
            .find(|field| field.key == "base_url")
            .unwrap();

        assert_eq!(base_url.default_value, URL_BIGMODEL_NOSTREAM);
    }

    #[test]
    fn stt_schema_defaults_match_runtime_contracts() {
        let expected = [
            (
                "volcengine-streaming",
                URL_BIGMODEL_NOSTREAM,
                DEFAULT_VOLCENGINE_RESOURCE_ID,
            ),
            (
                "aliyun-stream",
                ALIYUN_REALTIME_ENDPOINT,
                ALIYUN_REALTIME_MODEL,
            ),
            (
                "elevenlabs",
                ELEVENLABS_REALTIME_ENDPOINT,
                ELEVENLABS_REALTIME_MODEL,
            ),
        ];

        for (provider_id, endpoint, model) in expected {
            let schema = STT_SCHEMAS
                .iter()
                .find(|schema| schema.id == provider_id)
                .unwrap();
            let field = |key| schema.fields.iter().find(|field| field.key == key).unwrap();

            assert_eq!(field("base_url").default_value, endpoint);
            assert_eq!(field("model").default_value, model);
        }
    }

    #[test]
    fn polish_schema_defaults_match_runtime_contracts() {
        let anthropic = POLISH_SCHEMAS
            .iter()
            .find(|schema| schema.id == "anthropic")
            .unwrap();
        let openai = POLISH_SCHEMAS
            .iter()
            .find(|schema| schema.id == "openai")
            .unwrap();
        let endpoint = |schema: &ProviderSchema| {
            schema
                .fields
                .iter()
                .find(|field| field.key == "base_url")
                .unwrap()
                .default_value
        };

        assert_eq!(endpoint(anthropic), ANTHROPIC_MESSAGES_ENDPOINT);
        assert_eq!(endpoint(openai), OPENAI_CHAT_COMPLETIONS_ENDPOINT);
    }
}
