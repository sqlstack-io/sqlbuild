//! Adapt diagnostic display facts while preserving policy and code ordering.

use crate::bindings::_helpers::planner_migrations::column_warnings::{
    missing_origin_outcome, warning,
};
use crate::bindings::_helpers::planner_migrations::constants::HIDDEN_ORIGIN_REMEDY;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyString;
use sqlbuild_analysis::planner_migrations::main::model_warning::model_warning;
use sqlbuild_analysis::planner_migrations::models::ModelWarning;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(super) fn entry_warning<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = entry.py();
    let name: String = native_field(entry, "model_name")?.extract()?;
    let origin = display(&native_field(entry, "origin")?)?;
    let destination = display(&native_field(entry, "destination")?)?;
    let decision: String = native_field(entry, "decision")?.extract()?;
    let completed = native_field(entry, "completed_at")?;
    let completed: Option<String> = if completed.is_none() {
        None
    } else {
        Some(completed.call_method0("isoformat")?.extract()?)
    };
    let target: String = native_field(entry, "target_label")?.extract()?;
    let policy = native_field(entry, "missing_origin_policy")?;
    let remedy: String = HIDDEN_ORIGIN_REMEDY.to_owned();
    let outcome: String = missing_origin_outcome(&policy)?.to_owned();
    let findings: String = PyString::new(py, "; ")
        .call_method1("join", (native_field(entry, "compatibility_findings")?,))?
        .extract()?;
    let facts = ModelWarning {
        name: &name,
        origin: &origin,
        destination: &destination,
        decision: &decision,
        manual: native_field(entry, "discovery")?.eq("manual")?,
        completed: completed.as_deref(),
        target: &target,
        hidden: native_field(entry, "origin_hidden")?.is_truthy()?,
        missing: native_field(entry, "origin_missing")?.is_truthy()?,
        deny: policy.eq("deny")?,
        blocked: native_field(entry, "blocks_build")?.is_truthy()?,
        remedy: &remedy,
        outcome: &outcome,
        findings: &findings,
    };
    model_warning(&facts)
        .map(|(code, message, error)| warning(entry, code, &message, error))
        .transpose()
}

fn display(location: &Bound<'_, PyAny>) -> PyResult<String> {
    let qualified = native_field(location, "qualified_name")?;
    if qualified.is_truthy()? {
        qualified.extract()
    } else {
        native_field(location, "name")?.extract()
    }
}
