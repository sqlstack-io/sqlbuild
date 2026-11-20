//! Public traits and type aliases of project snapshot.

use crate::project_snapshot::models::WorkerBuild;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Node keys by snapshot node key, such as the models each test checks.
pub type NodeKeys = BTreeMap<String, Vec<String>>;

/// A claimed run's worker build and the project inputs it verified against the frozen snapshot.
pub type VerifiedRun<'a, 'py> = (Arc<WorkerBuild>, &'a Bound<'py, PyAny>);

/// Each provider's consumers, as `{kind, name}` documents, and the snapshot nodes that run them, by provider name.
pub(crate) type ProviderConsumers = (
    BTreeMap<String, Vec<Value>>,
    BTreeMap<String, BTreeSet<String>>,
);
