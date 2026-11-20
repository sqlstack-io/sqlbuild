//! Type aliases of named-job settings.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::Bound;
use pyo3::types::PyString;
use serde_json::Value;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Each job a frozen ad-hoc build pins: the exact nodes that take its settings, and those settings.
pub type PinnedJobs = BTreeMap<String, (BTreeSet<ObjectKey>, BTreeMap<String, Value>)>;

/// The settings a command line gives, and its argv with its named jobs' flags when they add any.
pub type JobSettingsArgv<'py> = (BTreeSet<String>, Option<Vec<Bound<'py, PyString>>>);
