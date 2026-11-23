//! While output is captured rather than shown on a terminal, one line names the nodes that run long.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use pyo3::prelude::{Bound, Py, Python};
use sqlbuild_core::build_progress::main::heartbeat_interval::heartbeat_interval;
use sqlbuild_core::build_progress::main::heartbeat_message::heartbeat_message;
use sqlbuild_core::build_progress::models::RunningNodeText;
use sqlbuild_core::cli_text::models::CliStyle;

use crate::build_progress::models::BuildProgressCallbacks;

const HEARTBEAT_TICK: Duration = Duration::from_secs(1);

/// A running node's start and the rows it has written, unknown once a batch reports no count.
#[derive(Debug)]
pub(crate) struct RunningNode {
    pub(crate) started: Instant,
    pub(crate) rows: Option<i64>,
    pub(crate) uncounted: bool,
}

/// The running nodes, how many heartbeats this long stretch has printed, and the timer thread.
#[derive(Debug, Default)]
pub(crate) struct HeartbeatState {
    pub(crate) running: BTreeMap<String, RunningNode>,
    pub(crate) beats: usize,
    pub(crate) last: Option<Instant>,
    pub(crate) stop: Option<Sender<()>>,
    pub(crate) thread: Option<JoinHandle<()>>,
}

impl HeartbeatState {
    /// The heartbeat line when one is due: once a node has run 30 s, then backing off; a quiet stretch starts over.
    pub(crate) fn due_line(&mut self, now: Instant, style: CliStyle) -> Option<String> {
        let running: Vec<RunningNodeText> = self
            .running
            .iter()
            .map(|(name, node)| RunningNodeText {
                name: name.clone(),
                elapsed_seconds: now.duration_since(node.started).as_secs(),
                rows_so_far: node.rows,
            })
            .collect();
        let longest: u64 = running
            .iter()
            .map(|node| node.elapsed_seconds)
            .max()
            .unwrap_or_default();
        if Duration::from_secs(longest) < heartbeat_interval(0) {
            self.beats = 0;
            self.last = None;
            return None;
        }
        let due: bool = self
            .last
            .is_none_or(|last| now.duration_since(last) >= heartbeat_interval(self.beats));
        if !due {
            return None;
        }
        self.beats += 1;
        self.last = Some(now);
        Some(heartbeat_message(&running, style))
    }
}

/// Record a started node and start the timer thread on the first one.
pub(crate) fn node_started(callbacks: &Bound<'_, BuildProgressCallbacks>, name: &str) {
    let owner: &BuildProgressCallbacks = callbacks.get();
    let mut state = owner.state();
    state.heartbeat.running.insert(
        name.to_owned(),
        RunningNode {
            started: Instant::now(),
            rows: None,
            uncounted: false,
        },
    );
    if state.heartbeat.thread.is_some() {
        return;
    }
    let (stop, stopped) = mpsc::channel::<()>();
    let owned: Py<BuildProgressCallbacks> = callbacks.clone().unbind();
    state.heartbeat.stop = Some(stop);
    state.heartbeat.thread = Some(std::thread::spawn(move || {
        while stopped.recv_timeout(HEARTBEAT_TICK) == Err(RecvTimeoutError::Timeout) {
            let owner: &BuildProgressCallbacks = owned.get();
            let line: Option<String> = owner
                .state()
                .heartbeat
                .due_line(Instant::now(), owner.style);
            if let Some(line) = line {
                let _ = Python::attach(|py| owner.write_persistent(py, &line));
            }
        }
    }));
}

/// Add a finished batch's rows to a running node; a batch with no count leaves its rows unknown.
pub(crate) fn batch_rows(owner: &BuildProgressCallbacks, name: &str, rows: Option<i64>) {
    let mut state = owner.state();
    let Some(node) = state.heartbeat.running.get_mut(name) else {
        return;
    };
    node.uncounted |= rows.is_none();
    node.rows = match (node.uncounted, rows) {
        (false, Some(rows)) => Some(node.rows.unwrap_or_default() + rows),
        _ => None,
    };
}

/// Set a running loader's rows written so far.
pub(crate) fn rows_written(owner: &BuildProgressCallbacks, name: &str, rows: i64) {
    if let Some(node) = owner.state().heartbeat.running.get_mut(name) {
        node.rows = Some(rows);
    }
}

/// Stop the timer thread and wait for it, without holding the interpreter it may be writing through.
pub(crate) fn stop(py: Python<'_>, owner: &BuildProgressCallbacks) {
    let (stop, thread) = {
        let mut state = owner.state();
        (state.heartbeat.stop.take(), state.heartbeat.thread.take())
    };
    drop(stop);
    if let Some(thread) = thread {
        let _ = py.detach(|| thread.join());
    }
}
