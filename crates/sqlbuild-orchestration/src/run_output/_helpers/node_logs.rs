//! Queue a worker's `ctx.log` lines and send them from one background thread, in batches.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use crate::run_output::constants::{
    NODE_LOG_FLUSH_INTERVAL, NODE_LOG_FLUSH_LINES, NODE_LOG_FLUSH_WAIT, NODE_LOGS_PER_NODE,
};
use crate::run_output::main::node_log_change::node_log_change;
use crate::run_output::models::{NodeLogReport, NodeLogSender, NodeLogShared, NodeLogState};
use crate::runs::models::{NodeLogLevel, RunChange};

impl NodeLogShared {
    fn locked(&self) -> MutexGuard<'_, NodeLogState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl NodeLogState {
    /// Whether the queue should go now: enough lines, a second since the oldest, or someone waiting for it.
    fn due(&self) -> bool {
        !self.queue.is_empty()
            && (self.closing
                || self.flushing > 0
                || self.queue.len() >= NODE_LOG_FLUSH_LINES
                || self
                    .oldest
                    .is_some_and(|oldest| oldest.elapsed() >= NODE_LOG_FLUSH_INTERVAL))
    }

    /// Whether any of `node_key`'s lines are still waiting or being sent.
    fn holds(&self, node_key: &str) -> bool {
        self.sending.contains(node_key) || self.queue.iter().any(|(node, _, _)| node == node_key)
    }
}

impl NodeLogSender {
    /// A sender that reports each line with `report`; its thread starts with the first line.
    #[must_use]
    pub fn new(report: Box<NodeLogReport>) -> Self {
        Self {
            shared: Arc::new(NodeLogShared {
                state: Mutex::default(),
                wake: std::sync::Condvar::new(),
                report,
            }),
            thread: Mutex::default(),
        }
    }

    /// Queue the line `node_key` just logged, if it is among the lines kept; never waits on the server.
    pub fn push(&self, node_key: &str, (level, message): (NodeLogLevel, &str)) {
        {
            let mut state = self.shared.locked();
            if state.closing {
                return;
            }
            let logged: &mut usize = state.logged.entry(node_key.to_owned()).or_default();
            let index: usize = *logged;
            *logged += 1;
            if index >= NODE_LOGS_PER_NODE {
                return;
            }
            let (key, change) = node_log_change(node_key, index, (level, message));
            state.queue.push_back((node_key.to_owned(), key, change));
            state.oldest.get_or_insert_with(Instant::now);
        }
        self.shared.wake.notify_all();
        self.start();
    }

    /// Send `node_key`'s waiting lines now and wait, for a while at most, until the server has them.
    pub fn flush(&self, node_key: &str) {
        let mut state = self.shared.locked();
        if !state.holds(node_key) {
            return;
        }
        state.flushing += 1;
        self.shared.wake.notify_all();
        let deadline: Instant = Instant::now() + NODE_LOG_FLUSH_WAIT;
        while state.holds(node_key) {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            state = self
                .shared
                .wake
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        state.flushing -= 1;
    }

    /// Whether the server acknowledged the line reported under `key`.
    #[must_use]
    pub fn was_sent(&self, key: &str) -> bool {
        self.shared.locked().sent.contains(key)
    }

    /// Record that the line reported under `key` reached the server another way.
    pub fn mark_sent(&self, key: &str) {
        self.shared.locked().sent.insert(key.to_owned());
    }

    /// How many lines `node_key` has logged, kept or not.
    #[must_use]
    pub fn logged(&self, node_key: &str) -> usize {
        self.shared
            .locked()
            .logged
            .get(node_key)
            .copied()
            .unwrap_or(0)
    }

    /// Send what is waiting, then stop the thread; later lines are dropped.
    pub fn close(&self) {
        self.shared.locked().closing = true;
        self.shared.wake.notify_all();
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            drop(thread.join());
        }
    }

    fn start(&self) {
        let mut thread = self.thread.lock().unwrap_or_else(PoisonError::into_inner);
        if thread.is_none() {
            let shared: Arc<NodeLogShared> = Arc::clone(&self.shared);
            *thread = Some(std::thread::spawn(move || send_node_logs(&shared)));
        }
    }
}

/// Send each due batch in order until the sender closes and nothing is left.
fn send_node_logs(shared: &NodeLogShared) {
    loop {
        let batch: Vec<(String, String, RunChange)> = {
            let mut state = shared.locked();
            loop {
                if state.due() {
                    state.oldest = None;
                    let batch: Vec<(String, String, RunChange)> = state.queue.drain(..).collect();
                    for (node, _, _) in &batch {
                        state.sending.insert(node.clone());
                    }
                    break batch;
                }
                if state.closing {
                    return;
                }
                let wait = state.oldest.map_or(NODE_LOG_FLUSH_INTERVAL, |oldest| {
                    NODE_LOG_FLUSH_INTERVAL.saturating_sub(oldest.elapsed())
                });
                state = shared
                    .wake
                    .wait_timeout(state, wait)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
        };
        let acknowledged: Vec<String> = batch
            .into_iter()
            .filter(|(_, key, change)| (shared.report)(key, change))
            .map(|(_, key, _)| key)
            .collect();
        let mut state = shared.locked();
        state.sent.extend(acknowledged);
        state.sending.clear();
        drop(state);
        shared.wake.notify_all();
    }
}

impl Drop for NodeLogSender {
    fn drop(&mut self) {
        self.close();
    }
}
