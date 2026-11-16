//! Delta-compile hooks of semantic shapes: coverage reasons and the model shapes a compile publishes.

use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

use sqlbuild_python_base::compile_context::main::coverage::coverage_reasons::coverage_reasons;
use sqlbuild_python_base::compile_context::main::coverage::merged_coverage::merged_coverage;
use sqlbuild_python_base::compile_context::main::coverage::merged_order::merged_order;
use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::capturing::capturing;
use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::guarded::guarded;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::compile_context::models::ModelCoverage;
use sqlbuild_python_base::compile_context::types::ModelShape;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The partial-check reasons of the `fresh` models, merged in a delta with the stored models'.
pub(crate) fn semantic_reasons(
    order: Vec<(String, Vec<String>)>,
    fresh: Vec<ModelCoverage>,
) -> PyResult<Vec<(String, Vec<String>)>> {
    let (order, facts): (Vec<(String, Vec<String>)>, Vec<ModelCoverage>) = match injection() {
        None => (
            order,
            fresh.into_iter().filter(ModelCoverage::notable).collect(),
        ),
        Some(injection) => {
            let Some(stored) = &injection.coverage else {
                return Err(delta_unsupported(
                    "the stored compile recorded no semantic coverage",
                ));
            };
            (
                merged_order(&injection.coverage_order, &order),
                merged_coverage(stored, &fresh),
            )
        }
    };
    let reasons: Vec<(String, Vec<String>)> = coverage_reasons(&order, &facts);
    if capturing() {
        capture(|capture| {
            if capture.coverage.is_none() {
                capture.coverage = Some(facts);
            }
        });
    }
    Ok(reasons)
}

/// In a delta compile, publish the shapes of every model it does not compile again first.
pub(crate) fn inject_model_shapes(
    models: &[Bound<'_, PyAny>],
    shapes: &Bound<'_, PyDict>,
) -> PyResult<()> {
    let Some(injection) = injection() else {
        return Ok(());
    };
    let mut rerun: HashSet<String> = HashSet::with_capacity(models.len());
    for model in models {
        rerun.insert(native_field(model, "name")?.extract()?);
    }
    let py = shapes.py();
    for (name, columns) in &injection.model_shapes {
        if rerun.contains(name) {
            continue;
        }
        let shape: Bound<'_, PyDict> = PyDict::new(py);
        for (column, column_type) in columns {
            shape.set_item(column, column_type)?;
        }
        shapes.set_item(name, shape)?;
    }
    Ok(())
}

/// Record the model shapes the first semantic shape pass published.
fn record_model_shapes_inner(
    project: &Bound<'_, PyAny>,
    shapes: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if !capturing() {
        return Ok(());
    }
    let mut names: HashSet<String> = HashSet::new();
    for model in native_field(project, "models")?.try_iter()? {
        names.insert(native_field(&model?, "name")?.extract()?);
    }
    if let Some(injection) = injection() {
        names.extend(injection.model_shapes.iter().map(|(name, _)| name.clone()));
    }
    let mut published: Vec<ModelShape> = Vec::new();
    for (name, shape) in shapes.iter() {
        let name: String = name.extract()?;
        if !names.contains(&name) {
            continue;
        }
        let shape: Bound<'_, PyDict> = shape.cast_into()?;
        let mut columns: Vec<(String, String)> = Vec::with_capacity(shape.len());
        for (column, column_type) in shape.iter() {
            columns.push((column.extract()?, column_type.extract()?));
        }
        published.push((name, columns));
    }
    capture(|capture| {
        if capture.model_shapes.is_none() {
            capture.model_shapes = Some(published);
        }
    });
    Ok(())
}

/// Record the model shapes the first semantic shape pass published.
pub(crate) fn record_model_shapes(
    project: &Bound<'_, PyAny>,
    shapes: &Bound<'_, PyDict>,
) -> PyResult<()> {
    guarded(|| record_model_shapes_inner(project, shapes))
}
