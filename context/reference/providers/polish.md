# Shipped Polish providers

Voice Flow exposes two cloud Polish provider IDs: `anthropic` and `openai`. A separate `custom` provider ID does not exist. OpenAI-compatible and Anthropic-compatible services can still be used by setting a compatible Base URL under the matching provider.

## Provider list

| Provider ID | Default endpoint | Request format | Streaming preview |
|-------------|------------------|----------------|-------------------|
| `anthropic` | `https://api.anthropic.com/v1/messages` | Anthropic Messages | No |
| `openai` | `https://api.openai.com/v1/chat/completions` | OpenAI Chat Completions | Yes |

Both providers require an API key and model. An unsupported provider ID fails before network I/O.

## Endpoint resolution

The backend resolves the Base URL in this order:

- Empty value: use the official default endpoint for the selected provider.
- Full `/v1/messages` or `/v1/chat/completions` URL: use it unchanged.
- URL ending in `/v1`: append the provider path after `/v1`.
- Other base URL: append the complete provider path.

This allows compatible gateways without advertising them as separate provider integrations. Compatibility depends on the gateway accepting the exact request and response shape described below.

## Anthropic

### HTTP contract

```http
POST /v1/messages
x-api-key: <api-key>
anthropic-version: 2023-06-01
content-type: application/json
```

The request contains the configured model, `max_tokens: 4096`, the Voice Flow core Polish prompt in `system`, and one user message. When `enable_thinking` is false, the backend sends `thinking: {"type":"disabled"}`. When it is true, the backend omits the field and lets the selected model use its default thinking behavior.

Thinking support is model-specific. Some current Anthropic models reject `disabled`, while older models may not enable thinking when the field is omitted. The connection check exercises the selected model and returns its error instead of assuming compatibility.

The response parser selects the first `content` block containing text. Anthropic requests currently return only a final result to Voice Flow.

Official references:

- [Anthropic Messages API](https://platform.claude.com/docs/en/api/messages/create)
- [Anthropic authentication](https://platform.claude.com/docs/en/manage-claude/authentication)
- [Anthropic thinking model differences](https://platform.claude.com/docs/en/build-with-claude/extended-thinking)

## OpenAI

### HTTP contract

```http
POST /v1/chat/completions
Authorization: Bearer <api-key>
content-type: application/json
```

The request contains the configured model, `max_tokens: 4096`, a system message with the Voice Flow core Polish prompt, and one user message. The normal path sends `stream: false`. A request with a preview callback sends `stream: true` and parses server-sent events until `[DONE]`.

For the Alibaba Coding Plan compatibility domains, the backend also sends `enable_thinking` and uses `opencode/1.0.0` as the User-Agent. Other OpenAI-compatible endpoints receive the standard OpenAI request fields only.

Official reference:

- [OpenAI Chat Completions API](https://developers.openai.com/api/reference/cli/resources/chat/subresources/completions)

## Connection checks and timeouts

The Cloud Service Check action uses the saved active provider configuration and sends a minimal request through the same endpoint path as normal Polish requests. It does not send a transcription or expose the API key through frontend IPC arguments.

Connection checks time out after 10 seconds. Normal Polish requests start with a 5-second timeout, add 5 seconds for each additional 1,000 bytes of prompt and input, and stop growing at 30 seconds.

The backend classifies common failures as missing field, invalid URL, authentication failure, model failure, network failure, timeout, unsupported provider, or provider error.

## Managed MLX local Polish

On Apple Silicon, model IDs `mlx-qwen3-0.6b` and `mlx-qwen3-1.7b` use the `mlx` engine through `UnifiedPolishManager`, without an HTTP endpoint. Display names end in `· MLX`. The app installs a dedicated private Python 3.12.15 / MLX-LM 0.32.2 / MLX 0.32.4 environment with a hashed binary lock; macOS 14 or newer is required. It does not modify the managed MLX Audio STT environment. The [Apple Silicon policy](../../feat/apple-silicon-model-policy/0.1.0/prd/erd.md) permits only these managed models and Apple Intelligence locally. Excluded native selections migrate idempotently to `mlx-qwen3-1.7b`, preserving valid Apple/MLX selections, cached legacy files and unrelated cloud configuration. Other hardware retains its native policy.

The catalogue includes only complete immutable checkpoint manifests. Download verifies every file's size/hash before publishing model readiness; each file's HTTP future races cancellation and a 20-minute total deadline even with stalled headers/body. First use hashes every file again, caching only verified manifest/file identities and revalidating metadata changes. Confirmed corruption removes model readiness and the mismatching file for repair, without invalidating a healthy runtime. Cancelled/expired verification preserves healthy files. Selected availability is backend-owned and requires model readiness, the correct runtime, and verified Apple Silicon macOS 14+ eligibility. Native OS-version detection is cached; an older Apple Silicon OS does not reenable excluded GGUF engines.

A persistent offline JSON-lines worker owns one model at a time; timeouts, cancellation, unload, idle expiry, and shutdown stop only its owned process. Lock acquisition/integrity preparation has a 60-second budget, worker preparation/startup a separate 30-second budget, and the worker request retains the caller's inference timeout (60 seconds by default), with an additive outer cap. Generation rejects incomplete/token-limited output. Common prompts, language/context rules, and output rejection remain shared with dictation.

The shared text-transform service awaits MLX's staged operation directly; it must not spend the short-transcript 20-second inference allowance on first-use integrity/startup. Native local and cloud service timeouts are unchanged. Real verification must include this service deadline policy, not only a direct manager call.

Both checkpoints loaded and corrected a French synthetic sample through production manager dispatch. The 1.7B model corrected the complete targeted sentence; 0.6B corrected only part of it. Neither has a general accuracy/latency benchmark or validated per-template recommendation. Exact inputs, outputs, revisions, and verification limits are in the [MLX polish specification](../../feat/mlx-polish/0.1.0/prd/erd.md). See the [provider integration guide](../../guides/adding-polish-provider.md#managed-mlx-local-providers) for extending manifests and running isolated inference checks.

## Native HTTP local Polish runtime

Local GGUF Polish is separate from the two cloud providers. It uses an OpenAI-compatible service, normally at `http://127.0.0.1:8000/v1`. The settings presets cover `llama-server`, LM Studio, Ollama, and a custom compatible endpoint.

Voice Flow may start a configured `llama-server` process when a local Polish request needs it. It stops only child processes that it started. An externally launched LM Studio or Ollama process stays under the user's control. Cloud Polish and idle unloading stop the managed local process.

Recognized bundled executable locations are:

- macOS: `bin/apple-silicon/llama-server`, `bin/intel/llama-server`, `bin/universal/llama-server`, or `bin/macos/llama-server`
- Windows: `bin/windows/llama-server.exe`
- Linux: `bin/linux/llama-server` or `bin/llama-server`

Packaging uses `scripts/prepare-tauri-runtime-resources.mjs`. Release builds require the sidecar and its sibling dynamic libraries. Development builds may run without a bundled runtime.

The runtime preparation variables use the `VOICEFLOW_` prefix:

- `VOICEFLOW_LLAMA_SERVER_MACOS_ARM64_PATH`
- `VOICEFLOW_LLAMA_SERVER_MACOS_X64_PATH`
- `VOICEFLOW_LLAMA_SERVER_MACOS_PATH`
- `VOICEFLOW_LLAMA_SERVER_WINDOWS_X64_PATH`
- `VOICEFLOW_LLAMA_SERVER_WINDOWS_PATH`
- `VOICEFLOW_LLAMA_SERVER_LINUX_X64_PATH`
- `VOICEFLOW_LLAMA_SERVER_LINUX_PATH`
- `VOICEFLOW_LLAMA_SERVER_<PLATFORM_OR_ARCH>_SHA256`
- `VOICEFLOW_REQUIRE_LOCAL_POLISH_RUNTIME`

Runtime override variables are `VOICEFLOW_LOCAL_POLISH_BASE_URL`, `VOICEFLOW_LOCAL_POLISH_API_KEY`, `VOICEFLOW_LOCAL_POLISH_SERVER_COMMAND`, `VOICEFLOW_LOCAL_POLISH_SERVER_ARGS_JSON`, and `VOICEFLOW_LOCAL_POLISH_READY_TIMEOUT_SECS`.

See [the local Polish latency and runtime case study](./capswriter-polish-latency.md) for process lifecycle, streaming, packaging, and verification details.

## Deterministic contract tests

The cloud tests use local HTTP mock servers and need no credentials or internet connection:

```bash
cd apps/desktop/src-tauri
cargo test --test cloud_polish_mock_test
```

The suite checks Anthropic and OpenAI headers, endpoint paths, request bodies, response parsing, OpenAI streaming, both connection-check contracts, request timeouts, and unsupported provider rejection.

Optional live auth-rejection checks are ignored by default:

```bash
cd apps/desktop/src-tauri
cargo test --test cloud_provider_api_test -- --ignored --nocapture
```

These live checks depend on vendor network access. They do not replace the deterministic suite and are not evidence of valid credentials or successful production access.


## Built-in profile quality policy

Version 1.2.3 restores the six v1.2.1 built-in prompts and conservative shared local/cloud instructions after v1.2.2 caused unwanted lists. Automatic enumeration and paragraph formatting is limited again; Structured Notes retains its historical list-oriented style. The family defaults still share Clean Dictation, with no copyable input/output examples. The v1.2.2 transcript preparation, output guards and model request policies below remain in place.

Qwen3 4B uses `enable_thinking: true` in both the top-level request and `chat_template_kwargs`, with `thinking_budget_tokens: 1536`. This is intentional: French inference checks found that disabling reasoning caused it to ignore layout and self-correction instructions. The shared service gives this model a 60-second deadline. Other Qwen models keep their existing non-thinking policy. The budget field is supported by the shipped llama.cpp b9568 runtime; an external compatible server may ignore that extension, but the request deadline still applies. Reasoning content is not rendered as the result.

LFM requests explicitly enclose the transcript in an editing task. This reduces its tendency to answer a dictated question. Model responses still pass through the shared output policy; framing alone is not a content-safety guarantee.

Before inference, Cleanup and Concise resolve unambiguous single-value weekday/numeric corrections, preserving the raw transcript. The shared policy rejects lost question marks, additional questions, recognized assistant-answer patterns, and loss of a clear weekday/numeric correction marked by `non pardon` or `no sorry`. The length guard discounts adjacent identical long sentences and spoken ordinal markers only when enough actual list items exist. These are bounded heuristics, not general semantic equivalence. The original transcript remains the fallback.

See [profile specification](../../feat/polish-profile-structure/0.1.0/prd/erd.md) and [inference evidence](../../feat/polish-profile-structure/0.1.0/prd/quality-investigation.md).

The llama.cpp budget request field is defined in [b9568 server-common.cpp](https://github.com/ggml-org/llama.cpp/blob/b9568/tools/server/server-common.cpp).
