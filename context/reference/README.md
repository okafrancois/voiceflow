# Provider Reference

API reference documentation for cloud and local providers integrated into Voice Flow.

## When to Read This

- Read [`../README.md`](../README.md) for document routing and canonical sources
- Read [`../guides/`](../guides/README.md) for step-by-step integration or debugging workflows
- Read [`../spec/engine-api-contract.md`](../spec/engine-api-contract.md) for contract-testing policy and engine-level interface rules
- Read this directory when you need provider facts: endpoints, auth methods, models, limits, or request shapes

## Purpose

This directory contains stable API reference documentation for external providers. These docs support implementation and debugging; they are not plans or strategy documents.

## Provider Categories

| Category | Document | Description |
|----------|----------|-------------|
| **STT (Speech-to-Text)** | [providers/stt.md](./providers/stt.md) | Shipped cloud STT providers: Volcengine, Aliyun Realtime, and ElevenLabs |
| **Local STT Models** | [providers/local-stt-models.md](./providers/local-stt-models.md) | Local STT model research: SenseVoice, Whisper, Distil-Whisper, Parakeet TDT, Moonshine, Qwen3-ASR, Paraformer, FireRedAsr comparison |
| **Polish (Text Enhancement)** | [providers/polish.md](./providers/polish.md) | Shipped cloud Polish providers: Anthropic and OpenAI, plus the separate local runtime |
| **Polish Latency Case Study** | [providers/capswriter-polish-latency.md](./providers/capswriter-polish-latency.md) | CapsWriter-Offline Polish latency analysis and Voice Flow architecture recommendations |

## How These Docs Relate

| Want to... | Go to... |
|------------|----------|
| Understand engine API contracts | [spec/engine-api-contract.md](../spec/engine-api-contract.md) |
| Add a new STT provider | [guides/adding-stt-provider.md](../guides/adding-stt-provider.md) |
| Add a new Polish provider | [guides/adding-polish-provider.md](../guides/adding-polish-provider.md) |
| Look up provider-specific API details | [providers/](./providers/) |
| Compare local STT models for sherpa-onnx | [providers/local-stt-models.md](./providers/local-stt-models.md) |
| Understand why CapsWriter-Offline polish feels fast | [providers/capswriter-polish-latency.md](./providers/capswriter-polish-latency.md) |
| Understand provider selection logic | [architecture/data-flow.md](../architecture/data-flow.md) |

## Maintenance

- Update provider docs when API contracts change or new providers are added
- Verify provider URLs and authentication methods against provider documentation and deterministic local contract tests
- Keep feature comparison tables current with implemented functionality
