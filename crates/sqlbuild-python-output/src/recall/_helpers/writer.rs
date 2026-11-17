//! `sqlbuild.recall.RecallWriter`: what a Recall replacement sink publishes through.

use std::sync::Mutex;

use serde_json::{Map, Value};

/// The one stream a writer publishes to.
#[derive(Debug)]
pub(crate) enum Target {
    /// The events the sink published for the one it was handed.
    Lifecycle(Mutex<Vec<Map<String, Value>>>),
    /// The chunk the sink was handed, by sequence, and what it published for it.
    Output {
        sequence: u64,
        published: Mutex<Option<Vec<u8>>>,
    },
}
