//! The worker side of run output: complete lines per node and stream go into batches one sender thread posts.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use crate::api::errors::ClientError;
use crate::run_output::constants::{
    BATCH_TEXT_BYTES, BATCH_WINDOW, MAX_PARTIAL_LINE_BYTES, MAX_QUEUED_BATCHES, UPLOAD_ATTEMPTS,
};
use crate::run_output::models::{
    OutputBatch, OutputSegment, OutputStream, RunOutputUploader, UploadShared, UploadState,
};
use crate::worker::models::WorkerClient;

/// The run output of this process's run; a worker process runs one run.
pub(crate) static ATTACHED: Mutex<Option<Arc<RunOutputUploader>>> = Mutex::new(None);

thread_local! {
    /// The node whose code runs on this thread, keyed `<kind>:<name>`.
    pub(crate) static OUTPUT_NODE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) fn attached() -> MutexGuard<'static, Option<Arc<RunOutputUploader>>> {
    ATTACHED.lock().unwrap_or_else(PoisonError::into_inner)
}

impl UploadShared {
    pub(crate) fn locked(&self) -> MutexGuard<'_, UploadState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl UploadState {
    /// Take `data` for `node`'s `stream`, keeping a line until its newline unless it grows past its limit.
    pub(crate) fn append(&mut self, (node, stream): (String, OutputStream), data: &[u8]) {
        if self.summary.failure.is_some() || self.closing.is_some() {
            self.summary.dropped_bytes += data.len() as u64;
            return;
        }
        let key = (node, stream);
        let mut buffer: Vec<u8> = self.partial.remove(&key).unwrap_or_default();
        buffer.extend_from_slice(data);
        let complete: usize = match buffer.iter().rposition(|byte| *byte == b'\n') {
            Some(position) => position + 1,
            None if buffer.len() > MAX_PARTIAL_LINE_BYTES => {
                buffer.push(b'\n');
                buffer.len()
            }
            None => 0,
        };
        let rest: Vec<u8> = buffer.split_off(complete);
        if !rest.is_empty() {
            self.partial.insert(key.clone(), rest);
        }
        if !buffer.is_empty() {
            self.push_lines(key, &String::from_utf8_lossy(&buffer));
        }
    }

    /// End every held partial line, as when the run finishes.
    pub(crate) fn end_lines(&mut self) {
        for (key, mut buffer) in std::mem::take(&mut self.partial) {
            buffer.push(b'\n');
            self.push_lines(key, &String::from_utf8_lossy(&buffer));
        }
    }

    fn push_lines(&mut self, (node, stream): (String, OutputStream), text: &str) {
        let node_key: Option<String> = Some(node).filter(|node| !node.is_empty());
        let dropped: u64 = std::mem::take(&mut self.unreported_drops);
        match self.segments.last_mut() {
            Some(last) if last.node_key == node_key && last.stream == stream && dropped == 0 => {
                last.text.push_str(text);
            }
            _ => self.segments.push(OutputSegment {
                node_key,
                stream,
                text: text.to_owned(),
                dropped_bytes: dropped,
            }),
        }
        self.batch_bytes += text.len();
        self.batch_started.get_or_insert_with(Instant::now);
        if self.batch_bytes >= BATCH_TEXT_BYTES {
            self.seal();
        }
    }

    /// Queue the open batch; a printing thread waits while too many are queued.
    pub(crate) fn seal(&mut self) {
        if self.segments.is_empty() {
            return;
        }
        let segments: Vec<OutputSegment> = std::mem::take(&mut self.segments);
        self.batch_bytes = 0;
        self.batch_started = None;
        self.queued.push_back(OutputBatch {
            batch: self.next_batch,
            segments,
        });
        self.next_batch += 1;
    }

    /// Whether the printing thread must wait for the sender before printing more.
    pub(crate) fn backlogged(&self) -> bool {
        self.queued.len() > MAX_QUEUED_BATCHES && self.summary.failure.is_none()
    }

    /// Drop the newest queued batches past the limit and count their bytes, when the sender has stalled.
    pub(crate) fn shed(&mut self) {
        while self.queued.len() > MAX_QUEUED_BATCHES {
            let Some(batch) = self.queued.pop_back() else {
                return;
            };
            let bytes: u64 = batch
                .segments
                .iter()
                .map(|segment| segment.text.len() as u64)
                .sum();
            self.next_batch -= 1;
            self.summary.dropped_bytes += bytes;
            self.unreported_drops += bytes;
        }
    }

    /// Whether the open batch has waited its window.
    pub(crate) fn window_passed(&self, now: Instant) -> bool {
        self.batch_started
            .is_some_and(|started| now.duration_since(started) >= BATCH_WINDOW)
    }
}

/// Post batches until the uploader closes and nothing is left, or its close budget is spent.
pub(crate) fn send_batches(
    shared: &Arc<UploadShared>,
    client: &WorkerClient,
    runtime: &tokio::runtime::Runtime,
) {
    loop {
        let batch: Option<OutputBatch> = {
            let mut state = shared.locked();
            loop {
                let now = Instant::now();
                if state.window_passed(now) || state.closing.is_some() {
                    state.seal();
                }
                if let Some(batch) = state.queued.pop_front() {
                    break Some(batch);
                }
                if state.closing.is_some() {
                    break None;
                }
                state = shared
                    .wake
                    .wait_timeout(state, BATCH_WINDOW)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
        };
        let Some(batch) = batch else {
            return;
        };
        let bytes: u64 = batch
            .segments
            .iter()
            .map(|segment| segment.text.len() as u64)
            .sum();
        let past_budget: bool = shared
            .locked()
            .closing
            .is_some_and(|deadline| Instant::now() >= deadline);
        let sent: Result<(), String> = if past_budget {
            Err("the run finished before its last output could be sent".into())
        } else {
            send(client, runtime, &batch)
        };
        let mut state = shared.locked();
        shared.wake.notify_all();
        match sent {
            Ok(()) => state.summary.stored_batches += 1,
            Err(failure) => {
                state.summary.dropped_bytes += bytes;
                if state.summary.failure.is_none() {
                    state.summary.failure = Some(failure);
                }
                let dropped: u64 = state
                    .queued
                    .drain(..)
                    .flat_map(|queued| queued.segments)
                    .map(|segment| segment.text.len() as u64)
                    .sum();
                state.summary.dropped_bytes += dropped;
            }
        }
    }
}

/// Send one batch, again while the server is unreachable or busy; a refusal says why.
fn send(
    client: &WorkerClient,
    runtime: &tokio::runtime::Runtime,
    batch: &OutputBatch,
) -> Result<(), String> {
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match runtime.block_on(client.publish_output_batch(batch)) {
            Ok(_) => return Ok(()),
            Err(ClientError::Rejected(status))
                if status == reqwest::StatusCode::NOT_FOUND
                    || status == reqwest::StatusCode::METHOD_NOT_ALLOWED =>
            {
                return Err(
                    "the control server is older than this worker and does not accept run output; upgrade the server before its workers".into(),
                );
            }
            Err(error) if attempt < UPLOAD_ATTEMPTS && retryable(&error) => {
                std::thread::sleep(std::time::Duration::from_millis(200 * u64::from(attempt)));
            }
            Err(error) => {
                return Err(format!(
                    "the control server did not accept run output: {error}"
                ));
            }
        }
    }
}

fn retryable(error: &ClientError) -> bool {
    match error {
        ClientError::Transport(_) => true,
        ClientError::Rejected(status) => {
            status.is_server_error() || *status == reqwest::StatusCode::TOO_MANY_REQUESTS
        }
        _ => false,
    }
}
