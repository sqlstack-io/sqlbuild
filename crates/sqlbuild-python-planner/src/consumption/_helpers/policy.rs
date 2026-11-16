//! What `--changed` counts and how far a change reaches: the command line, else `[changes]`.

use std::collections::BTreeSet;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_state::consumption::models::{ChangeKind, NodeKey, TriggeredJob};

use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const DEFINITION: &str = "definition";
pub(crate) const SOURCE_DATA: &str = "source_data";
pub(crate) const REACH_JOB: &str = "job";
pub(crate) const REACH_ALL: &str = "all";

pub(crate) fn change_kind(name: &str) -> PyResult<ChangeKind> {
    match name {
        DEFINITION => Ok(ChangeKind::Definition),
        SOURCE_DATA => Ok(ChangeKind::SourceData),
        other => Err(PyValueError::new_err(format!(
            "unknown change kind '{other}': expected {DEFINITION} or {SOURCE_DATA}"
        ))),
    }
}

/// The request's `--select` selectors.
pub(crate) fn selectors(request: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    Ok(native_field(request, "select")?
        .extract::<Option<Vec<String>>>()?
        .unwrap_or_default())
}

/// A triggered job with its selected nodes as consumption keys.
pub(crate) fn triggered_job((name, keys): (String, BTreeSet<ObjectKey>)) -> TriggeredJob {
    TriggeredJob {
        name,
        nodes: keys
            .into_iter()
            .map(|(kind, name)| NodeKey { kind, name })
            .collect(),
    }
}
