//! Where a command reads past runs from, and what it reads.

use std::collections::BTreeSet;
use std::path::PathBuf;

use uuid::Uuid;

/// Where a command's run selectors read past runs from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PastRunSource {
    /// A server, with the `sqb login` session for it.
    Server {
        origin: String,
        ca: Option<PathBuf>,
        location_id: Uuid,
        target: String,
    },
    /// The local SQLite Recall store.
    Recall { store: PathBuf },
    /// Neither can be read, and why.
    Unavailable(String),
}

/// One past run's results as node keys; failed audits are `audit:<name>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PastRunKeys {
    pub selected: BTreeSet<String>,
    pub failed: BTreeSet<String>,
    pub skipped: BTreeSet<String>,
}
