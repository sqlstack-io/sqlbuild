//! The worker side of run output: complete lines per node and stream go into batches one sender thread posts.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use crate::api::errors::ClientError;
use crate::run_output::constants::{
    BATCH_TEXT_BYTES, BATCH_WINDOW, MAX_BATCH_SEGMENTS, MAX_PARTIAL_LINE_BYTES, MAX_QUEUED_BATCHES,
    UPLOAD_ATTEMPTS, UTF8_CONTINUATION, UTF8_PREFIX_MASK,
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
    /// Take `data` for `node`'s `stream`, holding a line until its newline unless it outgrows its limit and is cut between characters.
    pub(crate) fn append(&mut self, (node, stream): (String, OutputStream), data: &[u8]) {
        if self.closing.is_some() {
            self.summary.dropped_bytes += data.len() as u64;
            return;
        }
        let key = (node, stream);
        let mut buffer: Vec<u8> = self.partial.remove(&key).unwrap_or_default();
        buffer.extend_from_slice(data);
        let complete: usize = match buffer.iter().rposition(|byte| *byte == b'\n') {
            Some(position) => position + 1,
            None if buffer.len() > MAX_PARTIAL_LINE_BYTES => complete_characters(&buffer),
            None => 0,
        };
        let rest: Vec<u8> = buffer.split_off(complete);
        if complete > 0 && buffer.last() != Some(&b'\n') {
            buffer.push(b'\n');
        }
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

    /// Add complete lines, sealing the batch before a line would overflow its size or a segment its count.
    fn push_lines(&mut self, (node, stream): (String, OutputStream), text: &str) {
        let node_key: Option<String> = Some(node).filter(|node| !node.is_empty());
        for line in text.split_inclusive('\n') {
            for piece in line_pieces(line) {
                self.push_piece((&node_key, stream), &piece);
            }
        }
    }

    fn push_piece(&mut self, (node_key, stream): (&Option<String>, OutputStream), piece: &str) {
        if self.batch_bytes + piece.len() > BATCH_TEXT_BYTES {
            self.seal();
        }
        let joins: bool = self.unreported_drops == 0
            && self
                .segments
                .last()
                .is_some_and(|last| last.node_key == *node_key && last.stream == stream);
        if joins {
            if let Some(last) = self.segments.last_mut() {
                last.text.push_str(piece);
            }
        } else {
            if self.segments.len() >= MAX_BATCH_SEGMENTS {
                self.seal();
            }
            self.segments.push(OutputSegment {
                node_key: node_key.clone(),
                stream,
                text: piece.to_owned(),
                dropped_bytes: std::mem::take(&mut self.unreported_drops),
            });
        }
        self.batch_bytes += piece.len();
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
        self.queued.len() > MAX_QUEUED_BATCHES
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

/// How many leading bytes end on a character boundary; an incomplete UTF-8 sequence at the end waits for the next write.
pub(crate) fn complete_characters(bytes: &[u8]) -> usize {
    let tail: usize = bytes.len().min(3);
    for back in 1..=tail {
        let byte: u8 = bytes[bytes.len() - back];
        if byte & UTF8_PREFIX_MASK == UTF8_CONTINUATION {
            continue;
        }
        let width: usize = (byte.leading_ones() as usize).max(1);
        return if width > back {
            bytes.len() - back
        } else {
            bytes.len()
        };
    }
    bytes.len()
}

/// A line cut into pieces of at most a partial line's size at character boundaries, each ending in a newline.
pub(crate) fn line_pieces(line: &str) -> Vec<std::borrow::Cow<'_, str>> {
    if line.len() <= MAX_PARTIAL_LINE_BYTES + 1 {
        return vec![std::borrow::Cow::Borrowed(line)];
    }
    let body: &str = line.strip_suffix('\n').unwrap_or(line);
    let mut pieces: Vec<std::borrow::Cow<'_, str>> = Vec::new();
    let mut start: usize = 0;
    while start < body.len() {
        let mut end: usize = (start + MAX_PARTIAL_LINE_BYTES).min(body.len());
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        pieces.push(std::borrow::Cow::Owned(format!("{}\n", &body[start..end])));
        start = end;
    }
    pieces
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
                state.unreported_drops += bytes;
                if state.summary.failure.is_none() {
                    state.summary.failure = Some(failure);
                }
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
