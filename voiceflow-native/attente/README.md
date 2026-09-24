# MLXPolisher — on hold

Polishing via local models (Qwen, Gemma, Llama) through MLX Swift, ready to
plug in, but **not buildable at the same time as WhisperKit**:

| Package                     | requires `swift-transformers` |
|-----------------------------|----------------------------|
| WhisperKit ≤ 0.14           | 0.1.8 ..< 0.2.0            |
| WhisperKit 0.15 → 0.18      | 1.1.2 ..< 1.2.0            |
| mlx-swift-examples 2.29.1   | 1.0.0 ..< 1.1.0            |
| mlx-swift-examples `main`   | ≥ 1.3.0                    |

No overlap: you have to choose between Whisper for transcription and
MLX for polishing, until one of the two moves.

To re-enable (without Whisper): put this file back in `Sources/VoiceFlow/`,
add the `mlx-swift-examples` dependency (`main` branch) to `Package.swift`
with the `MLXLLM` and `MLXLMCommon` products, and remove WhisperKit.

The API used was verified against the sources of version 2.21.2:
`LLMModelFactory.shared.loadContainer(configuration:progressHandler:)`,
`ModelContainer.perform`, `context.processor.prepare(input: UserInput(messages:))`,
`generate(input:parameters:context:didGenerate:)` → `GenerateResult.output`.
