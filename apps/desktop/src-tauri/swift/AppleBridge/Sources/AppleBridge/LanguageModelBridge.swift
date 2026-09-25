import Foundation
import FoundationModels

// C ABI over Foundation Models (Apple Intelligence, macOS 26+), used for
// on-device polish. `generate` blocks the calling thread until the model
// answers, so Rust must call it from a blocking thread.

private enum LanguageModelStatus: Int32 {
    case available = 0
    case unavailable = 1
    case osUnsupported = 3
}

/// Polishing is a deterministic task: a low temperature leaves less room for
/// fanciful output.
private let polishTemperature = 0.2

@_cdecl("vf_apple_llm_status")
public func vf_apple_llm_status() -> Int32 {
    guard #available(macOS 26, *) else {
        return LanguageModelStatus.osUnsupported.rawValue
    }
    switch SystemLanguageModel.default.availability {
    case .available:
        return LanguageModelStatus.available.rawValue
    default:
        return LanguageModelStatus.unavailable.rawValue
    }
}

@_cdecl("vf_apple_llm_generate")
public func vf_apple_llm_generate(
    _ instructions: UnsafePointer<CChar>,
    _ prompt: UnsafePointer<CChar>
) -> UnsafeMutablePointer<CChar> {
    guard #available(macOS 26, *) else {
        return duplicate(json([
            "error": "Apple Intelligence requires macOS 26 or later", "kind": "unavailable",
        ]))
    }
    let instructions = String(cString: instructions)
    let prompt = String(cString: prompt)
    let payload: [String: String] = blocking {
        guard case .available = SystemLanguageModel.default.availability else {
            return [
                "error": "Apple Intelligence is not available: enable it in System Settings",
                "kind": "unavailable",
            ]
        }
        let session = LanguageModelSession(instructions: instructions)
        do {
            let response = try await session.respond(
                to: prompt, options: GenerationOptions(temperature: polishTemperature))
            return ["text": response.content]
        } catch {
            return ["error": error.localizedDescription, "kind": errorKind(error)]
        }
    }
    return duplicate(json(payload))
}

/// Classifies content-filter refusals without naming the error types, which
/// changed between the macOS 26 and 27 SDKs.
private func errorKind(_ error: Error) -> String {
    let description = String(describing: error)
    if description.contains("guardrailViolation") || description.contains("refusal") {
        return "refused"
    }
    return "failed"
}
