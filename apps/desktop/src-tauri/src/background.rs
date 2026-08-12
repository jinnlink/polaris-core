use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundJob {
    GradeQueue,
    MirrorReport,
    NightlyConsolidation,
    MentalDynamicsFit,
    ParameterTuning,
    FsrsFit,
    Backup,
}

impl BackgroundJob {
    pub const fn id(self) -> &'static str {
        match self {
            Self::GradeQueue => "grade_queue",
            Self::MirrorReport => "mirror_report",
            Self::NightlyConsolidation => "nightly_consolidation",
            Self::MentalDynamicsFit => "mental_dynamics_fit",
            Self::ParameterTuning => "parameter_tuning",
            Self::FsrsFit => "fsrs_fit",
            Self::Backup => "backup",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundEvent {
    Started(BackgroundJob),
    Finished {
        job: BackgroundJob,
        invalidates: Vec<String>,
        message: String,
    },
    Failed {
        job: BackgroundJob,
        message: String,
    },
    Cancelled(BackgroundJob),
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundJobResult {
    pub invalidates: Vec<String>,
    pub message: String,
}

enum WorkerMessage {
    Enqueue(BackgroundJob),
    DrainAndStop,
    CancelAndStop,
}

pub struct SerialWorker {
    sender: Sender<WorkerMessage>,
    events: Mutex<Receiver<BackgroundEvent>>,
    join: Mutex<Option<JoinHandle<()>>>,
    cancel_requested: Arc<AtomicBool>,
}

impl SerialWorker {
    pub fn start<F>(handler: F) -> Self
    where
        F: Fn(BackgroundJob) -> Result<BackgroundJobResult, String> + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel_requested);
        let join = thread::spawn(move || {
            worker_loop(receiver, event_sender, thread_cancel, handler);
        });
        Self {
            sender,
            events: Mutex::new(events),
            join: Mutex::new(Some(join)),
            cancel_requested,
        }
    }

    pub fn enqueue(&self, job: BackgroundJob) -> Result<(), String> {
        if self.cancel_requested.load(Ordering::Acquire) {
            return Err("后台 worker 正在退出，不能再接收任务".to_owned());
        }
        self.sender
            .send(WorkerMessage::Enqueue(job))
            .map_err(|_| "后台 worker 已停止".to_owned())
    }

    pub fn take_events(&self) -> Vec<BackgroundEvent> {
        let Ok(receiver) = self.events.lock() else {
            return Vec::new();
        };
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn drain_and_stop(&self) -> Result<(), String> {
        if self.is_stopped()? {
            return Ok(());
        }
        self.sender
            .send(WorkerMessage::DrainAndStop)
            .map_err(|_| "后台 worker 已停止".to_owned())?;
        self.join()
    }

    pub fn cancel_and_stop(&self) -> Result<(), String> {
        if self.is_stopped()? {
            return Ok(());
        }
        self.cancel_requested.store(true, Ordering::Release);
        self.sender
            .send(WorkerMessage::CancelAndStop)
            .map_err(|_| "后台 worker 已停止".to_owned())?;
        self.join()
    }

    fn join(&self) -> Result<(), String> {
        let mut join = self
            .join
            .lock()
            .map_err(|_| "后台 worker 退出锁损坏".to_owned())?;
        if let Some(handle) = join.take() {
            handle
                .join()
                .map_err(|_| "后台 worker 线程异常退出".to_owned())?;
        }
        Ok(())
    }

    fn is_stopped(&self) -> Result<bool, String> {
        self.join
            .lock()
            .map(|join| join.is_none())
            .map_err(|_| "后台 worker 退出锁损坏".to_owned())
    }
}

impl Drop for SerialWorker {
    fn drop(&mut self) {
        self.cancel_requested.store(true, Ordering::Release);
        let _ = self.sender.send(WorkerMessage::CancelAndStop);
        if let Ok(join) = self.join.get_mut() {
            if let Some(handle) = join.take() {
                let _ = handle.join();
            }
        }
    }
}

fn worker_loop<F>(
    receiver: Receiver<WorkerMessage>,
    events: Sender<BackgroundEvent>,
    cancel_requested: Arc<AtomicBool>,
    handler: F,
) where
    F: Fn(BackgroundJob) -> Result<BackgroundJobResult, String>,
{
    let mut queue = VecDeque::new();
    let mut drain = false;
    loop {
        if queue.is_empty() {
            if drain || cancel_requested.load(Ordering::Acquire) {
                break;
            }
            match receiver.recv() {
                Ok(WorkerMessage::Enqueue(job)) => queue.push_back(job),
                Ok(WorkerMessage::DrainAndStop) => drain = true,
                Ok(WorkerMessage::CancelAndStop) | Err(_) => {
                    cancel_requested.store(true, Ordering::Release)
                }
            }
        }
        while let Ok(message) = receiver.try_recv() {
            match message {
                WorkerMessage::Enqueue(job) if !drain => queue.push_back(job),
                WorkerMessage::Enqueue(job) => {
                    let _ = events.send(BackgroundEvent::Cancelled(job));
                }
                WorkerMessage::DrainAndStop => drain = true,
                WorkerMessage::CancelAndStop => {
                    cancel_requested.store(true, Ordering::Release);
                }
            }
        }
        if cancel_requested.load(Ordering::Acquire) {
            for job in queue.drain(..) {
                let _ = events.send(BackgroundEvent::Cancelled(job));
            }
            break;
        }
        if let Some(job) = queue.pop_front() {
            let _ = events.send(BackgroundEvent::Started(job));
            let event = match handler(job) {
                Ok(result) => BackgroundEvent::Finished {
                    job,
                    invalidates: result.invalidates,
                    message: result.message,
                },
                Err(message) => BackgroundEvent::Failed { job, message },
            };
            let _ = events.send(event);
        }
    }
    let _ = events.send(BackgroundEvent::Stopped);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn worker_runs_jobs_strictly_in_enqueue_order() {
        let (observed_sender, observed) = mpsc::channel();
        let worker = SerialWorker::start(move |job| {
            observed_sender.send(job).unwrap();
            Ok(BackgroundJobResult {
                invalidates: vec![job.id().to_owned()],
                message: "ok".to_owned(),
            })
        });
        worker.enqueue(BackgroundJob::GradeQueue).unwrap();
        worker.enqueue(BackgroundJob::MirrorReport).unwrap();
        worker.enqueue(BackgroundJob::NightlyConsolidation).unwrap();
        worker.drain_and_stop().unwrap();
        worker.drain_and_stop().unwrap();
        assert_eq!(
            observed.iter().collect::<Vec<_>>(),
            vec![
                BackgroundJob::GradeQueue,
                BackgroundJob::MirrorReport,
                BackgroundJob::NightlyConsolidation,
            ]
        );
        let events = worker.take_events();
        assert!(matches!(events.last(), Some(BackgroundEvent::Stopped)));
    }

    #[test]
    fn cancellation_waits_for_current_safe_boundary_and_drops_queued_jobs() {
        let (started_sender, started) = mpsc::channel();
        let (release_sender, release) = mpsc::channel();
        let worker = SerialWorker::start(move |job| {
            started_sender.send(job).unwrap();
            if job == BackgroundJob::GradeQueue {
                release.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            Ok(BackgroundJobResult {
                invalidates: Vec::new(),
                message: "ok".to_owned(),
            })
        });
        worker.enqueue(BackgroundJob::GradeQueue).unwrap();
        worker.enqueue(BackgroundJob::ParameterTuning).unwrap();
        assert_eq!(
            started.recv_timeout(Duration::from_secs(2)).unwrap(),
            BackgroundJob::GradeQueue
        );
        worker.cancel_requested.store(true, Ordering::Release);
        release_sender.send(()).unwrap();
        worker.cancel_and_stop().unwrap();
        assert!(started.try_recv().is_err());
        assert!(worker.take_events().iter().any(|event| matches!(
            event,
            BackgroundEvent::Cancelled(BackgroundJob::ParameterTuning)
        )));
    }
}
