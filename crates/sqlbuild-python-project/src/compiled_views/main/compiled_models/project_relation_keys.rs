//! Declared keys of a compiled project's models, seeds and sources.

use crate::compiled_views::main::compiled_models::model_key_row::model_key_row;
use crate::compiled_views::main::compiled_models::one_row_seed_names::one_row_seed_names;
use crate::compiled_views::main::compiled_models::relation_keys::relation_keys;
use crate::compiled_views::main::compiled_models::source_key_input::source_key_input;
use crate::compiled_views::models::RelationKeyRow;
use crate::compiled_views::types::RelationKeys;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Declared keys of a compiled project's models, seeds and sources.
pub fn project_relation_keys(project: &Bound<'_, PyAny>) -> PyResult<RelationKeys> {
    let py: Python<'_> = project.py();
    let mut rows: Vec<RelationKeyRow> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        rows.push(model_key_row(&model?)?);
    }
    let compiled_seeds: Vec<Bound<'_, PyAny>> = native_field(project, "seeds")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let one_row_seeds: Vec<String> = one_row_seed_names(&compiled_seeds)?;
    let mut seeds: Vec<Bound<'_, PyAny>> = Vec::new();
    for seed in &compiled_seeds {
        seeds.push(native_field(seed, "schema_entry")?);
    }
    let mut sources: Vec<Bound<'_, PyAny>> = Vec::new();
    for source in native_field(project, "sources")?.try_iter()? {
        sources.push(source_key_input(&source?)?);
    }
    relation_keys(
        py,
        rows,
        (
            PyTuple::new(py, seeds)?.as_any(),
            PyTuple::new(py, sources)?.as_any(),
        ),
        &one_row_seeds,
    )
}
