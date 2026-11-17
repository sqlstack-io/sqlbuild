//! Public traits and type aliases of project snapshot.

use crate::project_snapshot::models::WorkerBuild;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Node keys by snapshot node key, such as the models each test checks.
pub type NodeKeys = BTreeMap<String, Vec<String>>;

/// A claimed run's worker build and the project inputs it verified against the frozen snapshot.
pub type VerifiedRun<'a, 'py> = (Arc<WorkerBuild>, &'a Bound<'py, PyAny>);
