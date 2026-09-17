import Foundation
import MLXLLM
import MLXLMCommon

/// Modèles de polissage disponibles : celui du système, ou un modèle local
/// téléchargé, comme le fait l'app actuelle avec Qwen, Gemma et consorts.
enum PolishModel: String, CaseIterable, Identifiable {
    case appleIntelligence
    case qwen05B
    case llama1B
    case gemma2B
    case llama3B

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .appleIntelligence: "Apple Intelligence"
        case .qwen05B: "Qwen 1.5 0,5B"
        case .llama1B: "Llama 3.2 1B"
        case .gemma2B: "Gemma 2 2B"
        case .llama3B: "Llama 3.2 3B"
        }
    }

    var detail: String {
        switch self {
        case .appleIntelligence: "Modèle du système · aucun téléchargement"
        case .qwen05B: "≈ 300 Mo · le plus rapide"
        case .llama1B: "≈ 700 Mo"
        case .gemma2B: "≈ 1,5 Go"
        case .llama3B: "≈ 1,8 Go · le plus soigné"
        }
    }

    /// Dépôt Hugging Face, repris du registre de MLX Swift — donc vérifié.
    var repositoryID: String? {
        switch self {
        case .appleIntelligence: nil
        case .qwen05B: "mlx-community/Qwen1.5-0.5B-Chat-4bit"
        case .llama1B: "mlx-community/Llama-3.2-1B-Instruct-4bit"
        case .gemma2B: "mlx-community/gemma-2-2b-it-4bit"
        case .llama3B: "mlx-community/Llama-3.2-3B-Instruct-4bit"
        }
    }

    var isLocalModel: Bool { repositoryID != nil }
}

/// Polissage par un modèle local via MLX (Apple Silicon). Le modèle est
/// téléchargé au premier usage puis gardé en mémoire.
final class MLXPolisher: PolishEngine {
    private let repositoryID: String
    private let maxTokens: Int

    init(repositoryID: String, maxTokens: Int = 1200) {
        self.repositoryID = repositoryID
        self.maxTokens = maxTokens
    }

    func prewarm(template: PolishTemplate) {
        // Charger pendant que l'utilisateur parle : le téléchargement et la
        // mise en mémoire se paient avant la fin de la dictée.
        let repositoryID = repositoryID
        Task.detached(priority: .userInitiated) {
            _ = try? await MLXModelCache.shared.container(repositoryID)
        }
    }

    func polish(_ text: String, template: PolishTemplate) async throws -> String {
        let container = try await MLXModelCache.shared.container(repositoryID)
        let maxTokens = maxTokens
        return try await container.perform { context in
            let input = try await context.processor.prepare(
                input: UserInput(messages: [
                    ["role": "system", "content": template.systemPrompt],
                    ["role": "user", "content": text],
                ]))
            // Température basse : on reformule, on n'invente pas.
            var parameters = GenerateParameters()
            parameters.temperature = 0.3
            let result = try MLXLMCommon.generate(
                input: input, parameters: parameters, context: context
            ) { tokens in
                tokens.count >= maxTokens ? .stop : .more
            }
            return result.output.trimmingCharacters(in: .whitespacesAndNewlines)
        }
    }
}

/// Garde les modèles chargés : le chargement coûte plusieurs secondes, on ne
/// le paie qu'une fois par modèle et par session.
actor MLXModelCache {
    static let shared = MLXModelCache()

    private var containers: [String: ModelContainer] = [:]
    private var loading: [String: Task<ModelContainer, Error>] = [:]

    /// Avancement du téléchargement en cours (0…1), pour l'affichage.
    @MainActor static var progress: (model: String, fraction: Double)?

    func container(_ repositoryID: String) async throws -> ModelContainer {
        if let container = containers[repositoryID] { return container }
        if let task = loading[repositoryID] { return try await task.value }

        let task = Task<ModelContainer, Error> {
            log.info("loading MLX model \(repositoryID)…")
            let container = try await LLMModelFactory.shared.loadContainer(
                configuration: ModelConfiguration(id: repositoryID)
            ) { progress in
                Task { @MainActor in
                    MLXModelCache.progress = (repositoryID, progress.fractionCompleted)
                    AppState.shared.modelDownload = progress.fractionCompleted
                }
            }
            await MainActor.run {
                MLXModelCache.progress = nil
                AppState.shared.modelDownload = nil
            }
            log.info("MLX model \(repositoryID) ready")
            return container
        }
        loading[repositoryID] = task
        defer { loading[repositoryID] = nil }
        let container = try await task.value
        containers[repositoryID] = container
        return container
    }
}
