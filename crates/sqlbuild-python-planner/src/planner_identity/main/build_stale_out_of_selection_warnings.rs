//! Warn for selected models stale through changed upstreams outside the run set.

use crate::planner_identity::_helpers::staleness::StalenessInputs;
use crate::planner_identity::_helpers::staleness::changed_model_names;
use crate::planner_identity::_helpers::staleness::changed_seed_names;
use crate::planner_identity::_helpers::staleness::plan_warnings;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_analysis::planner_identity::constants::{
    MODEL_RESOURCE, SEED_RESOURCE, SOURCE_RESOURCE,
};
use sqlbuild_analysis::planner_identity::main::classify_selection_staleness::classify_selection_staleness;
use sqlbuild_analysis::planner_identity::models::{GraphKey, StalenessGraph, StalenessNames};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use std::collections::HashSet;

/// Warn for selected models stale through changed upstreams outside the run set.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_stale_out_of_selection_warnings<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_stale_out_of_selection_warnings",
        options,
        (
            &[
                "original_scope",
                "execution_scope",
                "changes",
                "snapshot",
                "version_identities",
                "source_freshness",
            ],
            &["include_sources", "model_changes_complete"],
        ),
    )?;
    let include_sources: bool =
        !arguments.passed("include_sources")? || arguments.flag("include_sources")?;
    let inputs = StalenessInputs {
        original_scope: arguments.value("original_scope")?,
        changes: arguments.value("changes")?,
        snapshot: arguments.value("snapshot")?,
        identities: arguments.value("version_identities")?,
        model_changes_complete: arguments.flag("model_changes_complete")?,
    };
    let run = scope_data(&arguments.value("execution_scope")?)?;
    let original = scope_data(&inputs.original_scope)?;
    let names = |kind: &str| -> HashSet<String> {
        run.selected
            .iter()
            .filter(|key| key.0 == kind)
            .map(|key| key.1.clone())
            .collect()
    };
    let source_freshness: Bound<'py, PyAny> = arguments.value("source_freshness")?;
    let mut changed_sources: HashSet<String> = HashSet::new();
    if include_sources && !source_freshness.is_none() {
        for identity in native_field(&source_freshness, "changed_identities")?.try_iter()? {
            changed_sources.insert(native_field(&identity?, "source_name")?.extract()?);
        }
    }
    let graph = StalenessGraph {
        upstream_deps: original
            .upstream
            .iter()
            .map(|(key, deps)| {
                (
                    GraphKey::from(key),
                    deps.iter().map(GraphKey::from).collect(),
                )
            })
            .collect(),
        selected_model_names: original
            .selected
            .iter()
            .filter(|key| key.0 == MODEL_RESOURCE)
            .map(|key| key.1.clone())
            .collect(),
        run: StalenessNames {
            models: names(MODEL_RESOURCE),
            seeds: names(SEED_RESOURCE),
            sources: names(SOURCE_RESOURCE),
        },
        changed: StalenessNames {
            models: changed_model_names(&inputs)?,
            seeds: changed_seed_names(&inputs)?,
            sources: changed_sources,
        },
    };
    plan_warnings(py, classify_selection_staleness(&graph))
}
