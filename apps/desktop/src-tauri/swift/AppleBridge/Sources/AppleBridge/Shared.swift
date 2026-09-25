import Foundation

// Helpers shared by the bridge's C entry points. Strings returned to Rust are
// allocated with `strdup` and released through `vf_apple_bridge_free`.

final class ResultBox<T>: @unchecked Sendable {
    var value: T?
}

/// Runs async work to completion from a synchronous C entry point.
func blocking<T>(_ operation: @escaping @Sendable () async -> T) -> T {
    let semaphore = DispatchSemaphore(value: 0)
    let box = ResultBox<T>()
    Task.detached {
        box.value = await operation()
        semaphore.signal()
    }
    semaphore.wait()
    return box.value!
}

func duplicate(_ string: String) -> UnsafeMutablePointer<CChar> {
    strdup(string)
}

func json(_ payload: [String: String]) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: payload),
          let text = String(data: data, encoding: .utf8)
    else {
        return #"{"error":"failed to encode the bridge result","kind":"failed"}"#
    }
    return text
}

@_cdecl("vf_apple_bridge_free")
public func vf_apple_bridge_free(_ pointer: UnsafeMutablePointer<CChar>?) {
    free(pointer)
}
