use crate::remote::RemoteError;
use crate::sentence::{CandidateToken, TranslationRequest, TranslationResponse};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Submission {
    Queued,
    Busy,
    Stopped,
}

struct Job {
    id: u64,
    token: CandidateToken,
    configuration: u64,
    cancel: Arc<AtomicBool>,
}
pub struct Result {
    pub id: u64,
    pub token: CandidateToken,
    pub response: TranslationResponse,
}
pub struct Translator {
    tx: Option<SyncSender<Job>>,
    rx: Receiver<Result>,
    active: RefCell<Option<Arc<AtomicBool>>>,
    stop: Arc<AtomicBool>,
}
impl Translator {
    pub fn new(executable: PathBuf) -> Self {
        Self::with_handler(move |job| {
            let request = TranslationRequest {
                revision: format!("{:016x}", job.configuration),
                source: job.token.source.clone(),
            };
            (|| -> std::result::Result<TranslationResponse, RemoteError> {
                let bytes = serde_json::to_vec(&request).map_err(|_| RemoteError::Configuration)?;
                let child = crate::helper::spawn(&executable, &["--translate".into()], true)?;
                let output = child.capture(&bytes, &job.cancel, Duration::from_secs(30))?;
                serde_json::from_slice(&output).map_err(|_| RemoteError::InvalidResponse)
            })()
            .unwrap_or_else(|error| TranslationResponse {
                english: None,
                error: Some(if error == RemoteError::Unavailable {
                    RemoteError::HelperUnavailable
                } else {
                    error
                }),
            })
        })
    }
    fn with_handler(mut handler: impl FnMut(&Job) -> TranslationResponse + Send + 'static) -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, rx) = mpsc::sync_channel(2);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let guard = super::Guard::new();
        std::thread::spawn(move || {
            let _guard = guard;
            while let Ok(job) = jobs.recv() {
                if stopping.load(Ordering::Acquire) {
                    break;
                }
                if job.cancel.load(Ordering::Acquire) {
                    continue;
                }
                let response = handler(&job);
                if stopping.load(Ordering::Acquire) {
                    break;
                }
                if !job.cancel.load(Ordering::Acquire) {
                    let _ = results.try_send(Result {
                        id: job.id,
                        token: job.token,
                        response,
                    });
                }
            }
        });
        Self {
            tx: Some(tx),
            rx,
            active: RefCell::new(None),
            stop,
        }
    }
    pub fn cancel(&self) {
        if let Some(cancel) = self.active.borrow_mut().take() {
            cancel.store(true, Ordering::Release);
        }
    }
    pub fn request(&self, id: u64, token: CandidateToken, configuration: u64) -> Submission {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        *self.active.borrow_mut() = Some(cancel.clone());
        self.tx.as_ref().map_or(Submission::Stopped, |tx| {
            match tx.try_send(Job {
                id,
                token,
                configuration,
                cancel,
            }) {
                Ok(()) => Submission::Queued,
                Err(mpsc::TrySendError::Full(_)) => Submission::Busy,
                Err(mpsc::TrySendError::Disconnected(_)) => Submission::Stopped,
            }
        })
    }
    pub fn poll(&self) -> Option<Result> {
        self.rx.try_recv().ok()
    }
}
impl Drop for Translator {
    fn drop(&mut self) {
        self.cancel();
        self.stop.store(true, Ordering::Release);
        self.tx.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    fn token(selected: usize) -> CandidateToken {
        CandidateToken {
            input_revision: selected as u64,
            engine_generation: 1,
            selected,
            page: 0,
            source: "固定测试候选".into(),
        }
    }
    fn poll_until(translator: &Translator) -> Result {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(result) = translator.poll() {
                return result;
            }
            assert!(
                Instant::now() < deadline,
                "translation worker did not finish"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn full_queue_is_retryable_and_only_the_latest_candidate_can_complete() {
        let (started, running) = mpsc::channel();
        let (release, held) = mpsc::channel();
        let translator = Translator::with_handler(move |job| {
            if job.id == 1 {
                started.send(()).unwrap();
                held.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            TranslationResponse {
                english: Some("Fixed test candidate.".into()),
                error: None,
            }
        });
        assert_eq!(translator.request(1, token(1), 0), Submission::Queued);
        running.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(translator.request(2, token(2), 0), Submission::Queued);
        assert_eq!(translator.request(3, token(3), 0), Submission::Busy);
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if translator.request(3, token(3), 0) == Submission::Queued {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        let result = poll_until(&translator);
        assert_eq!(result.id, 3);
        assert_eq!(result.token, token(3));
        assert!(result.response.error.is_none());
        assert!(translator.poll().is_none());
    }
    #[test]
    fn missing_helper_is_distinct_from_unconfigured_local_translation() {
        let translator = Translator::new(
            std::env::temp_dir().join(format!("lingomate-missing-{}.exe", crate::storage::nonce())),
        );
        assert_eq!(translator.request(1, token(1), 0), Submission::Queued);
        let result = poll_until(&translator);
        assert_eq!(result.response.error, Some(RemoteError::HelperUnavailable));
        assert!(result.response.english.is_none());
    }
}
