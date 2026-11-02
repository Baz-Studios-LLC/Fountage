//! One in-flight analysis and at most one waiting snapshot. No UI objects leave the UI thread.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, SyncSender},
};

pub struct Request {
    pub revision: u64,
    pub source: String,
    pub category: i32,
}
pub struct Worker {
    wake: SyncSender<()>,
    pending: Arc<Mutex<Option<Request>>>,
    latest: Arc<AtomicU64>,
}
impl Worker {
    pub fn new(
        mut process: impl FnMut(Request, Arc<AtomicU64>) + Send + 'static,
    ) -> std::io::Result<Self> {
        let (wake, receiver) = mpsc::sync_channel(1);
        let pending = Arc::new(Mutex::new(None));
        let latest = Arc::new(AtomicU64::new(0));
        let slot = pending.clone();
        let revision = latest.clone();
        std::thread::Builder::new()
            .name("fountage-analysis".into())
            .spawn(move || {
                while receiver.recv().is_ok() {
                    let request = slot.lock().expect("analysis slot").take();
                    if let Some(request) = request {
                        process(request, revision.clone());
                    }
                }
            })?;
        Ok(Self {
            wake,
            pending,
            latest,
        })
    }
    pub fn submit(&self, source: String, category: i32) -> u64 {
        let revision = self.latest.fetch_add(1, Ordering::SeqCst) + 1;
        *self.pending.lock().expect("analysis slot") = Some(Request {
            revision,
            source,
            category,
        });
        let _ = self.wake.try_send(()); // Full means a wake is already queued; latest snapshot replaces old.
        revision
    }
}
pub fn is_current(latest: &AtomicU64, revision: u64) -> bool {
    latest.load(Ordering::SeqCst) == revision
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn coalesces_waiting_edits_and_rejects_in_flight_result() {
        let (started_tx, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (done_tx, done) = mpsc::channel();
        let worker = Worker::new(move |r, latest| {
            if r.revision == 1 {
                started_tx.send(()).unwrap();
                gate.recv().unwrap();
            }
            done_tx
                .send((
                    r.revision,
                    r.source,
                    r.category,
                    is_current(&latest, r.revision),
                ))
                .unwrap();
        })
        .unwrap();
        worker.submit("old".into(), 0);
        started.recv_timeout(Duration::from_secs(3)).unwrap();
        worker.submit("discarded".into(), 1);
        let latest = worker.submit("Zoë 🌙\r\n世界".into(), 7);
        release.send(()).unwrap();
        assert_eq!(
            done.recv_timeout(Duration::from_secs(3)).unwrap(),
            (1, "old".into(), 0, false)
        );
        assert_eq!(
            done.recv_timeout(Duration::from_secs(3)).unwrap(),
            (latest, "Zoë 🌙\r\n世界".into(), 7, true)
        );
        drop(worker);
        assert!(done.recv_timeout(Duration::from_secs(3)).is_err());
    }
    #[test]
    fn stale_check_also_rejects_a_previously_current_queued_delivery() {
        let latest = AtomicU64::new(4);
        assert!(is_current(&latest, 4));
        latest.store(5, Ordering::SeqCst);
        assert!(!is_current(&latest, 4));
    }
}
