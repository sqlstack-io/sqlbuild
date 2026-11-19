//! Type aliases of named-job settings.

use std::collections::BTreeSet;

use pyo3::prelude::Bound;
use pyo3::types::PyString;

/// The settings a command line gives, and its argv with its named jobs' flags when they add any.
pub type JobSettingsArgv<'py> = (BTreeSet<String>, Option<Vec<Bound<'py, PyString>>>);
