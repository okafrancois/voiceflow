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

## Local Polish runtime

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
