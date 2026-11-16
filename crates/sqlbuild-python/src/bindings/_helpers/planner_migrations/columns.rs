//! Convert projection facts and construct the existing Python hint models.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_migrations::main::{
    identical_renames as native_renames, rename_hints as native_hints,
};
use sqlbuild_analysis::planner_migrations::models::Projection;

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

fn projections(shape: &Bound<'_, PyAny>) -> PyResult<Vec<Projection>> {
    let mut result: Vec<Projection> = Vec::new();
    for item in native_field(shape, "projections")?.try_iter()? {
        let item = item?;
        result.push(Projection {
            name: native_field(&item, "name")?.extract()?,
            key: native_field(&item, "key")?.extract()?,
            expression: native_field(&item, "expression")?.extract()?,
            references: native_field(&item, "references")?.extract()?,
        });
    }
    Ok(result)
}

/// Unique automatic column renames whose full query change is explained.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn identical_renames<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments = KeywordArguments::read(
        py,
        "identical_renames",
        options,
        (
            &[
                "previous",
                "current",
                "excluded",
                "declared",
                "input_columns",
            ],
            &[],
        ),
    )?;
    let previous = arguments.value("previous")?;
    let current = arguments.value("current")?;
    let excluded: BTreeSet<String> = arguments.value("excluded")?.extract()?;
    let previous_projections = projections(&previous)?;
    let current_projections = projections(&current)?;
    let mut proof = |pairs: &[(String, String)]| -> PyResult<bool> {
        let renames = PyDict::new(py);
        for item in arguments
            .value("declared")?
            .call_method0("items")?
            .try_iter()?
        {
            let item = item?;
            renames.set_item(item.get_item(0)?, item.get_item(1)?)?;
        }
        for (old, new) in pairs {
            renames.set_item(old, new)?;
        }
        super::projections::explain(
            &previous,
            &current,
            &renames.into_any(),
            &arguments.value("input_columns")?,
        )
    };
    PyTuple::new(
        py,
        native_renames::identical_renames(
            (&previous_projections, &current_projections),
            &excluded,
            &mut proof,
        )?,
    )
}

/// Explicit migration hints for newly added columns resembling removed live columns.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn rename_hints<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments = KeywordArguments::read(
        py,
        "rename_hints",
        options,
        (
            &[
                "model_name",
                "previous",
                "current",
                "excluded",
                "live_columns",
            ],
            &[],
        ),
    )?;
    let previous = arguments.value("previous")?;
    let current = arguments.value("current")?;
    let hints = native_hints::rename_hints(
        (&projections(&previous)?, &projections(&current)?),
        &arguments.value("excluded")?.extract()?,
        &arguments.value("live_columns")?.extract()?,
        native_field(&previous, "body")?.eq(native_field(&current, "body")?)?,
    );
    let mut result: Vec<Bound<'py, PyAny>> = Vec::new();
    for hint in hints {
        result.push(new_record_with(
            py,
            RecordId::ColumnRenameHint,
            &[
                ("model_name", arguments.value("model_name")?),
                (
                    "added_column",
                    hint.added_column.into_pyobject(py)?.into_any(),
                ),
                (
                    "candidate_columns",
                    PyTuple::new(py, hint.candidate_columns)?.into_any(),
                ),
                (
                    "identical",
                    hint.identical.into_pyobject(py)?.to_owned().into_any(),
                ),
            ],
        )?);
    }
    PyTuple::new(py, result)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(identical_renames, module)?)?;
    module.add_function(wrap_pyfunction!(rename_hints, module)?)
}
