//! A logger that prints data over the IPC connection to the frontend.

use std::{
    collections::VecDeque,
    io,
    sync::{
        Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use interprocess::local_socket::Stream;
use log::{LevelFilter, Log, Metadata, Record};

use crate::protocol::{BackendMessage, LogRecord};

/// how many records a newly connected frontend gets to see
const HISTORY_LEN: usize = 512;

struct History {
    records: VecDeque<LogRecord>,
    /// how many records were ever logged, i.e. the sequence number of the next one
    end: u64,
}

static HISTORY: Mutex<History> = Mutex::new(History {
    records: VecDeque::new(),
    end: 0,
});
static RECORDED: Condvar = Condvar::new();

fn history() -> MutexGuard<'static, History> {
    HISTORY.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Logger {
    level: LevelFilter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let level = if metadata.target().starts_with("erebus") {
            self.level
        } else {
            LevelFilter::Warn.min(self.level)
        };

        metadata.level() <= level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let record = LogRecord {
            level: record.level(),
            target: record.target().to_owned(),
            message: record.args().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_millis() as u64),
        };

        let mut history = history();
        if history.records.len() == HISTORY_LEN {
            history.records.pop_front();
        }
        history.records.push_back(record);
        history.end += 1;
        drop(history);

        RECORDED.notify_all();
    }

    fn flush(&self) {}
}

/// Installs the logger. Only the first call does anything.
pub fn init() {
    let level = std::env::var("EREBUS_LOG")
        .ok()
        .and_then(|level| level.parse().ok())
        .unwrap_or(LevelFilter::Info);

    let logger = Box::leak(Box::new(Logger { level }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level.max(LevelFilter::Warn));
    }
}

/// Sends the history and then every new record to `stream` until [`close`] is called or the
/// stream fails.
pub(super) fn forward(stream: &Stream, closed: &AtomicBool) -> io::Result<()> {
    let mut next = 0;

    loop {
        let pending: Vec<LogRecord> = {
            let mut history = history();
            while history.end <= next && !closed.load(Ordering::Relaxed) {
                history = RECORDED
                    .wait(history)
                    .unwrap_or_else(PoisonError::into_inner);
            }

            if closed.load(Ordering::Relaxed) {
                return Ok(());
            }

            let start = history.end - history.records.len() as u64;
            let skip = next.saturating_sub(start) as usize;
            next = history.end;

            history.records.iter().skip(skip).cloned().collect()
        };

        for record in pending {
            crate::transport::send(stream, &BackendMessage::Log(record))?;
        }
    }
}

/// Stops a [`forward`] using `closed`.
pub(super) fn close(closed: &AtomicBool) {
    // under the lock so `forward` can't miss the wakeup between checking and waiting
    let _history = history();
    closed.store(true, Ordering::Relaxed);
    RECORDED.notify_all();
}
