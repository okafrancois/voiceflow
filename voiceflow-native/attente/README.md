# MLXPolisher — en attente

Polissage par modèles locaux (Qwen, Gemma, Llama) via MLX Swift, prêt à
brancher, mais **non compilable en même temps que WhisperKit** :

| Paquet                      | exige `swift-transformers` |
|-----------------------------|----------------------------|
| WhisperKit ≤ 0.14           | 0.1.8 ..< 0.2.0            |
| WhisperKit 0.15 → 0.18      | 1.1.2 ..< 1.2.0            |
| mlx-swift-examples 2.29.1   | 1.0.0 ..< 1.1.0            |
| mlx-swift-examples `main`   | ≥ 1.3.0                    |

Aucune intersection : il faut choisir entre Whisper pour la transcription et
MLX pour le polissage, tant qu'un des deux n'aura pas bougé.

Pour réactiver (sans Whisper) : remettre ce fichier dans `Sources/VoiceFlow/`,
ajouter au `Package.swift` la dépendance `mlx-swift-examples` (branche `main`)
avec les produits `MLXLLM` et `MLXLMCommon`, et retirer WhisperKit.

L'API utilisée a été vérifiée sur les sources de la version 2.21.2 :
`LLMModelFactory.shared.loadContainer(configuration:progressHandler:)`,
`ModelContainer.perform`, `context.processor.prepare(input: UserInput(messages:))`,
`generate(input:parameters:context:didGenerate:)` → `GenerateResult.output`.
