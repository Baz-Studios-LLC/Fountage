import XCTest
import AppKit
@testable import Fountage

@MainActor final class ShelfDropLoadingTests: XCTestCase {
    func testAllProviderRequestsStartSynchronouslyAndFinishInOriginalOrderOnce() async throws {
        var callbacks: [(String?, Error?) -> Void] = []
        let done = expectation(description: "Loaded all text")
        var results: [[String]] = []
        let drop = ShelfTextDrop.start([NSItemProvider(), NSItemProvider()], load: { _, callback in
            callbacks.append(callback); return Progress(totalUnitCount: 1)
        }, completion: { result in results.append((try? result.get()) ?? []); done.fulfill() })
        XCTAssertEqual(callbacks.count, 2, "Loading must start before performDrop can return")
        callbacks[1]("Second 🌙\r\n", nil); callbacks[1]("Duplicate", nil)
        XCTAssertTrue(results.isEmpty)
        callbacks[0]("First Zoë\n", nil)
        await fulfillment(of: [done], timeout: 1)
        XCTAssertEqual(results, [["First Zoë\n", "Second 🌙\r\n"]])
        drop.cancel()
    }
    func testDefaultLoaderPreservesActualNSStringProviderContent() async throws {
        let text = "Zoë 🌙\r\n[ ] Review\r\n"
        let done = expectation(description: "Actual provider loaded")
        let drop = ShelfTextDrop.start([NSItemProvider(object: text as NSString)]) { result in
            guard case .success(let values) = result else { XCTFail("Provider failed"); done.fulfill(); return }
            XCTAssertEqual(values.count, 1)
            XCTAssertEqual(values.first.map { Data($0.utf8) }, Data(text.utf8))
            done.fulfill()
        }
        await fulfillment(of: [done], timeout: 1)
        drop.cancel()
    }
    func testFailureDoesNotDeliverPartialValuesOrChangeDocument() async throws {
        var callbacks: [(String?, Error?) -> Void] = []
        var document = ScreenplayDocument(); document.text = "Original\r\n"
        let before = document.metadata
        let done = expectation(description: "Failed atomically")
        let drop = ShelfTextDrop.start([NSItemProvider(), NSItemProvider()], load: { _, callback in
            callbacks.append(callback); return Progress(totalUnitCount: 1)
        }, completion: { result in
            if case .success(let values) = result { XCTFail("Partial drop must fail"); try? document.addDroppedShelfText(values) }
            done.fulfill()
        })
        callbacks[0]("Valid", nil); callbacks[1](nil, ShelfError.invalid)
        await fulfillment(of: [done], timeout: 1)
        XCTAssertEqual(document.text, "Original\r\n"); XCTAssertTrue(document.shelfItems.isEmpty)
        XCTAssertTrue(NSDictionary(dictionary: before).isEqual(to: document.metadata)); drop.cancel()
    }
    func testCancellationStopsProgressAndSuppressesEvenQueuedCompletion() async throws {
        let done = expectation(description: "No callback after cancel"); done.isInverted = true
        var callback: ((String?, Error?) -> Void)?
        let progress = Progress(totalUnitCount: 1)
        let drop = ShelfTextDrop.start([NSItemProvider()], load: { _, result in callback = result; return progress }, completion: { _ in done.fulfill() })
        callback?("Ready but not delivered", nil)
        drop.cancel()
        XCTAssertTrue(progress.isCancelled)
        callback?("Late duplicate", nil)
        await fulfillment(of: [done], timeout: 0.1)
    }
    func testClaimedReorderSurvivesSourceEndButCannotBeReplayedOrAppliedAfterInvalidation() throws {
        var document = ScreenplayDocument()
        let a = ShelfItem(kind: .note, title: "A"), b = ShelfItem(kind: .note, title: "B")
        try document.saveShelfItem(a); try document.saveShelfItem(b)
        let store = ShelfDragStore()
        var session = ShelfDragSession(item: b, order: document.shelfItems.map(\.id)); store.begin(session)
        let receipt = try XCTUnwrap(store.claim()); XCTAssertNil(store.claim())
        store.end(id: session.id)
        XCTAssertTrue(try store.commit(receipt, token: session.token, to: &document, before: a.id))
        XCTAssertEqual(document.shelfItems.map(\.id), [b.id, a.id])
        XCTAssertThrowsError(try store.commit(receipt, token: session.token, to: &document, before: nil))
        session = ShelfDragSession(item: a, order: document.shelfItems.map(\.id)); store.begin(session)
        let cancelled = try XCTUnwrap(store.claim()); store.invalidate()
        XCTAssertThrowsError(try store.commit(cancelled, token: session.token, to: &document, before: b.id))
        store.begin(session); let invalidToken = try XCTUnwrap(store.claim())
        XCTAssertThrowsError(try store.commit(invalidToken, token: Data("wrong".utf8), to: &document, before: b.id))
        XCTAssertThrowsError(try store.commit(invalidToken, token: session.token, to: &document, before: b.id))
        XCTAssertEqual(document.shelfItems.map(\.id), [b.id, a.id])
    }
}
