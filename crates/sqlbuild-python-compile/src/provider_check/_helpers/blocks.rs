//! The providers missing secrets that a set of selected node keys uses.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyRef, PyResult, Python};
use pyo3::types::PyAnyMethods;
use sqlbuild_python_front::discovery::main::sources::sink_uses::sink_uses;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;
use sqlbuild_python_project::graph::models::ProjectSelection;

use crate::provider_check::_helpers::readers::{downstream_readers, readers_by_key};
use crate::provider_check::_helpers::users::provider_users;
use crate::provider_check::models::SecretBlock;

/// The providers missing secrets that the run of exactly `selected` node keys uses, with the nodes they stop.
pub(crate) fn blocks_for(
    (discovered, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (selection, selected): (&ProjectSelection, &BTreeSet<String>),
    missing: Vec<(String, String, Vec<String>)>,
) -> PyResult<Vec<SecretBlock>> {
    let py: Python<'_> = project.py();
    let view: PyRef<'_, NativeProjectView> = project.extract()?;
    let graph = view.lineage_graph(py)?;
    let readers = readers_by_key(graph.bind(py).get().graph(), selection);
    let sinks: Vec<(String, String)> = sink_uses(discovered)?;
    let mut blocks: Vec<SecretBlock> = Vec::new();
    for (provider, path, variables) in missing {
        let users: BTreeSet<String> = provider_users((discovered, project), selection, &provider)?
            .into_iter()
            .filter(|key| selected.contains(key))
            .collect();
        let mut using_sinks: BTreeSet<String> = BTreeSet::new();
        for (used, sink) in &sinks {
            if *used == provider {
                using_sinks.insert(sink.clone());
            }
        }
        if users.is_empty() && using_sinks.is_empty() {
            continue;
        }
        blocks.push(SecretBlock {
            readers: downstream_readers(&users, selected, &readers),
            users: users.into_iter().collect(),
            sinks: using_sinks.into_iter().collect(),
            provider,
            path,
            variables,
        });
    }
    Ok(blocks)
}
