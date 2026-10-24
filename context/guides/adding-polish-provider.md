# Adding a Text Polish Provider

This guide walks through adding a new text polish provider to the polish engine framework.

## When to Read This

- Read [`../../AGENTS.md`](../../AGENTS.md) for execution constraints, verification expectations, and the default iteration loop
- Read [`../reference/providers/polish.md`](../reference/providers/polish.md) for provider-specific API details and existing conventions
- Read [`../architecture/data-flow.md`](../architecture/data-flow.md) when provider behavior affects pipeline contracts or state transitions
- Read this guide for the concrete integration steps inside the current polish engine architecture

## Overview

The polish engine uses a trait-based architecture. Providers implement `PolishEngine` and integrate into `UnifiedPolishManager`.

**Two provider types:**
- **Local providers**: Run on-device (e.g., Qwen, LFM) — add new module in `polish_engine/`
- **Cloud providers**: Call external APIs (e.g., Anthropic, OpenAI) — extend `CloudPolishEngine`

## Step 1: Implement the Trait

### For Local Providers

Create `polish_engine/new_provider/engine.rs`:

```rust
use crate::polish_engine::traits::{PolishEngine, PolishEngineType, PolishRequest, PolishResult};
use async_trait::async_trait;

pub struct NewProviderEngine;

#[async_trait]
impl PolishEngine for NewProviderEngine {
    fn engine_type(&self) -> PolishEngineType {
        PolishEngineType::NewProvider
    }

    async fn polish(&self, request: PolishRequest) -> Result<PolishResult, String> {
        let t0 = std::time::Instant::now();
        // ... inference logic ...
        let total_ms = t0.elapsed().as_millis() as u64;
        Ok(PolishResult::new(result_text, PolishEngineType::NewProvider, total_ms))
    }
}
```

### For Cloud Providers

Extend `CloudPolishEngine` in `polish_engine/cloud/engine.rs`:

```rust
async fn call_new_provider_api(&self, system_prompt: &str, user_message: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "model": self.config.model,
        "prompt": format!("{}\n\n{}", system_prompt, user_message),
    });
    // Send request and parse response...
}
```

Add to provider switch in `polish()`:

```rust
let result = match self.config.provider_type.as_str() {
    "anthropic" => self.call_anthropic_api(&system_prompt, &input_text).await?,
    "openai" => self.call_openai_api(&system_prompt, &input_text).await?,
    "new-provider" => self.call_new_provider_api(&system_prompt, &input_text).await?,
    provider => return Err(format!("Unsupported cloud polish provider: {provider}")),
};
```

## Step 2: Add Engine Type to Enum

In `polish_engine/traits.rs` (for local providers):

```rust
pub enum PolishEngineType {
    Qwen, Lfm, Cloud, NewProvider,
}

impl PolishEngineType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NewProvider => "new-provider",
            // ... existing variants
        }
    }
}
```

## Step 3: Register in Unified Manager

In `polish_engine/unified_manager.rs`:

```rust
engines.insert(
    PolishEngineType::NewProvider,
    Arc::new(new_provider::NewProviderEngine::new()),
);
```

Update `mod.rs` to export the new module:

```rust
pub mod new_provider;
pub use new_provider::NewProviderEngine;
```

## Step 4: Frontend Configuration

For cloud providers, add the provider and its fields to `POLISH_SCHEMAS` in `src-tauri/src/provider_schema.rs`. `CloudPolishSection.tsx` renders the backend schema and must not keep a second provider list. Local engines instead use the backend model catalogue and typed `modelCommands` in `src/lib/tauri.ts`; do not add them as cloud services.

```rust
ProviderSchema {
    id: "new-provider",
    name: "New Provider",
    fields: NEW_PROVIDER_FIELDS,
}
```

## Step 5: i18n Keys

Add to all 10 locale files in `src/i18n/locales/`:

```json
"model.polish.cloud.providerNewProvider": "New Provider"
```

**Locales**: `de`, `en`, `es`, `fr`, `it`, `ja`, `ko`, `pt`, `ru`, `zh`.

## Step 6: Contract Tests

In `tests/cloud_polish_mock_test.rs`:

```rust
#[tokio::test]
async fn test_new_provider_request_format() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)).mount(&mock_server).await;
    
    let config = CloudProviderConfig {
        provider_type: "new-provider".to_string(),
        api_key: "test_key".to_string(),
        base_url: format!("{}/v1/polish", mock_server.uri()),
        model: "model".to_string(),
        enable_thinking: false,
    };
    let engine = CloudPolishEngine::new(config);
    let result = engine.polish(request).await.expect("failed");
    assert!(!result.text.is_empty());
}
```

## Step 7: Integration Tests

In `tests/polish_engine_test.rs`:

```rust
#[tokio::test]
async fn test_new_provider_basic() {
    let engine = NewProviderEngine::new();
    let result = engine.polish(PolishRequest::new("test", "prompt", "en")).await.unwrap();
    assert_eq!(result.engine, PolishEngineType::NewProvider);
}
```

## Verification Checklist

- [ ] Trait implementation compiles (`cargo check`)
- [ ] Engine type added to enum (local) or provider switch (cloud)
- [ ] Unified manager registration works
- [ ] Frontend renders new provider option
- [ ] All 10 locales have i18n keys
- [ ] Mock server test passes (cloud)
- [ ] Integration test passes
- [ ] `cargo clippy --all-features -- -D warnings` passes
- [ ] `cargo test` passes

## Managed MLX Local Providers

The Apple Silicon `mlx` polish engine is separate from both the GGUF HTTP runtime and the managed MLX Audio STT runtime. Its private Python 3.12.15 / MLX-LM 0.32.2 / MLX 0.32.4 environment lives in `runtimes/mlx-lm-0.32.2-python-3.12.15` beside the model directory. It reuses the bundled `uv` resource, not the STT environment, lock, readiness signature, or installation duties. Catalogue, compatibility, readiness, download and execution all require macOS 14 or newer through one fail-closed policy and a cached native OS-version observation. Native and cloud selections remain available, including on older Apple Silicon Macs; adding an engine does not migrate user settings.

The bootstrap resource is verified during preparation and belongs to the signed application bundle. Do not compare the running signed executable to its pre-sign raw SHA256: code-signing changes those bytes. Check the bundled resource/signing boundary in packaging validation instead.

When extending this engine:

Apple Silicon exposes only managed MLX and built-in Apple Intelligence for local polish. Apply `PolishEngineType` platform policy at registration, catalogue/compatibility, settings selection, download/load and inference boundaries, including direct native engine calls. Keep legacy ID lookup and explicit deletion available without executing excluded models. Do not use macOS runtime eligibility to decide hardware exclusions: macOS 12/13 on ARM does not reenable GGUF. Intel/Windows/Linux and vendor cloud retain their policies. See the [shared platform contract](../feat/apple-silicon-model-policy/0.1.0/prd/erd.md).

1. Verify the exact release metadata and pinned wheel API, then record complete immutable Hugging Face manifests in `polish_engine/mlx/models.rs`. Every file needs its exact size and SHA256. A repository name alone is not a supported model; arbitrary GGUF/ONNX files are not MLX checkpoints.
2. Update the worker's explicit model allowlist and tests. Use offline local loading, `trust_remote_code=False`, and deterministic generation. Keep all protocol messages on stdout and diagnostics on stderr.
3. Regenerate the hashed binary-only lock with the bundled `uv` and `MACOSX_DEPLOYMENT_TARGET=14.0`; verify the pinned wheel APIs and import/version validation. Never resolve polish dependencies into the verified STT environment.
4. Keep runtime/model readiness backend-owned and fail closed after cancellation or incomplete installation. `get_polish_model_status.available` is authoritative for the selected model, not merely evidence that another model's runtime is installed.
5. Use the common prompt builder, manager dispatch, transcript preparation, and output acceptance. Preserve templates, profile instructions, language, contextual correction, and original-text fallback. Reject truncated generation rather than treating it as successful polish.
6. Test malformed protocol, timeouts, dropped futures, active unload, idle cleanup, model switching, restart, and owned-process cleanup. Do not add TCP readiness checks or stop unrelated processes.
7. Run real French correction under an explicit `/tmp` root through the same manager dispatch and service-level deadline policy as dictation, using the service's actual short-text inference allowance. Manager-only success does not prove an enclosing service deadline permits cold preparation. Record each model's output and quality limits separately. A single sample is not a speed/accuracy benchmark; unmeasured models must not receive invented gauges or template recommendations.

Managed checkpoint downloads race cancellation and a 20-minute per-file total deadline, including stalled headers/body; cancellation remains exactly `cancelled` and drops/cleans partial artifacts. Do not broaden the shared downloader's native-engine policy for this. Download hashes do not replace first-use integrity: cache only successfully verified manifest/file identities, invalidate on metadata changes, and remove readiness plus the mismatching file on confirmed corruption so download can repair it. Healthy runtime readiness remains intact. Preserve healthy checkpoint files on cancelled/expired first-use verification; dropped hash futures signal the blocking hash loop to stop between 64 KiB reads.

Keep separate execution budgets: worker-lock acquisition plus first-use integrity preparation is bounded to 60 seconds, worker preparation/startup to 30 seconds, and the actual worker request to the caller's `PolishRequest::timeout` (default 60 seconds). An additive outer cap and lifecycle cancellation cover every stage. Preparation must not spend a short caller inference budget before generation starts. Retain deterministic regressions for delayed preparation, stalled inference, and owned-process cleanup; do not bypass integrity in debug/tests to make a large checkpoint pass.

`services::text_transform::await_local_polish` preserves these staged deadlines for MLX instead of applying the native whole-operation inference timeout. Keep service-boundary delayed-preparation and stalled-inference tests alongside engine-level timeout tests. Native local and cloud whole-operation deadlines must remain unchanged.

Targeted verification:

```bash
python3 -m unittest discover -s apps/desktop/src-tauri/tests -p 'mlx_polish_worker_test.py'
cd apps/desktop/src-tauri
cargo test --test managed_mlx_polish_test
cargo test --lib polish_engine::mlx
VOICEFLOW_MLX_POLISH_TEST_ROOT=/tmp/voiceflow-mlx-polish-validation \
  cargo test --lib real_managed_mlx_polish_corrects_french_through_dictation_dispatch -- --ignored --nocapture
```

Create the isolated test root before the ignored test. The test may install/download there, then runs offline worker inference with bounded requests and explicit unload; it never changes application settings. Installed-app UI, bundled bootstrap signing/resource discovery, and non-Apple host execution need separate platform verification. See the [MLX polish specification](../feat/mlx-polish/0.1.0/prd/erd.md) for concrete checkpoints and observed outputs, and the [testing guide](./testing.md) before combining frontend/Cargo checks: the normal desktop build runs `cargo clean`.

## Key Files Reference

| File | Purpose |
|------|---------|
| `polish_engine/traits.rs` | `PolishEngine` trait, `PolishEngineType`, request/result types |
| `polish_engine/cloud/engine.rs` | `CloudPolishEngine` for all cloud providers |
| `polish_engine/unified_manager.rs` | Engine lifecycle, caching |
| `polish_engine/mlx/` | Dedicated hashed runtime, curated checkpoints, offline worker and lifecycle |
| `src/lib/tauri.ts` | Frontend IPC types |
| `src/components/Home/cloud/CloudPolishSection.tsx` | Settings UI |
| `tests/cloud_polish_mock_test.rs` | Cloud API contract tests |

## Differences from STT Providers

| Aspect | STT | Polish |
|--------|-----|--------|
| Cloud architecture | Separate client per provider | Single `CloudPolishEngine` handles all |
| Trait methods | `transcribe()` or streaming lifecycle | Single `polish()` method |
| Engine types | Unified `SttEngine` (send_chunk + finish) | Single `PolishEngine` trait |
| Request flow | Audio → Text | Text → Text |

## Related Documentation

- [Engine API Contract Testing](../spec/engine-api-contract.md)
- [Adding STT Provider](./adding-stt-provider.md)
