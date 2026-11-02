import AppKit

/// Start every provider request in performDrop's call stack; only completion may be deferred.
final class ShelfTextDrop {
    typealias Loader = (NSItemProvider, @escaping (String?, Error?) -> Void) -> Progress
    private let lock = NSLock()
    private var values: [String?]
    private var received: [Bool]
    private var remaining: Int
    private var failure: Error?
    private var cancelled = false
    private var requests: [Progress] = []
    private var completion: ((Result<[String], Error>) -> Void)?

    private init(count: Int, completion: @escaping (Result<[String], Error>) -> Void) {
        values = Array(repeating: nil, count: count); received = Array(repeating: false, count: count)
        remaining = count; self.completion = completion
    }
    @MainActor static func start(_ providers: [NSItemProvider], load: Loader = { provider, callback in
        provider.loadObject(ofClass: NSString.self) { value, error in callback(value as? String, error) }
    }, completion: @escaping (Result<[String], Error>) -> Void) -> ShelfTextDrop {
        let drop = ShelfTextDrop(count: providers.count, completion: completion)
        if providers.isEmpty { drop.failure = ShelfError.invalid; drop.deliver() }
        for (index, provider) in providers.enumerated() {
            let request = load(provider) { value, error in drop.receive(value, error: error, at: index) }
            drop.lock.lock()
            if drop.cancelled { drop.lock.unlock(); request.cancel() }
            else { drop.requests.append(request); drop.lock.unlock() }
        }
        return drop
    }
    func cancel() {
        lock.lock(); cancelled = true; completion = nil
        let active = requests; requests.removeAll(); lock.unlock()
        active.forEach { $0.cancel() }
    }
    private func receive(_ value: String?, error: Error?, at index: Int) {
        lock.lock()
        guard !cancelled, !received[index] else { lock.unlock(); return }
        received[index] = true; remaining -= 1
        if let error { failure = failure ?? error }
        else if let value { values[index] = value }
        else { failure = failure ?? ShelfError.invalid }
        let ready = remaining == 0; lock.unlock()
        if ready { deliver() }
    }
    private func deliver() {
        DispatchQueue.main.async { [self] in
            lock.lock()
            guard !cancelled, let callback = completion else { lock.unlock(); return }
            completion = nil; requests.removeAll()
            let result: Result<[String], Error> = failure.map { .failure($0) } ?? .success(values.compactMap { $0 })
            lock.unlock()
            callback(result)
        }
    }
}
