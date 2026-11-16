//! `__dbt_ref` references, checked by the external resolver that owns the dbt manifest.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyDict;

use crate::input_loops::_helpers::model::model_loop::LoopContext;
use crate::input_loops::_helpers::model::model_loop::compile_error;
use sqlbuild_project::models::ReferenceRow;

const DBT_REF_KIND: &str = "dbt_ref";
const VALIDATE_METHOD: &str = "validate_reference";
const MISSING_MANIFEST_CODE: &str = "C214";
const MISSING_MANIFEST_HELP: &str =
    "Run dbt compile or configure dbt target_path so SQLBuild can read manifest.json.";
const QUOTE: char = '"';

/// `_external_reference_error` per reference, in order: the resolver's error, or `None`.
pub(crate) fn external_errors(
    context: &LoopContext<'_, '_>,
    references: &[ReferenceRow],
    relative_path: &str,
) -> PyResult<Vec<Option<PyErr>>> {
    let mut errors: Vec<Option<PyErr>> = Vec::with_capacity(references.len());
    for ReferenceRow {
        kind,
        name,
        package,
        ..
    } in references
    {
        errors.push(if kind == DBT_REF_KIND {
            external_error(context, (kind, name, package.as_deref()), relative_path)?
        } else {
            None
        });
    }
    Ok(errors)
}

fn external_error(
    context: &LoopContext<'_, '_>,
    reference: (&str, &str, Option<&str>),
    relative_path: &str,
) -> PyResult<Option<PyErr>> {
    let (kind, name, package) = reference;
    let resolver: &Bound<'_, PyAny> = &context.classes.external_resolver;
    let py: Python<'_> = resolver.py();
    if resolver.is_none() {
        let quoted: String = name.replace(QUOTE, "\"\"");
        let message: String = format!(
            "Model file {relative_path} uses __{kind}({QUOTE}{quoted}{QUOTE}) but no dbt \
             manifest was found"
        );
        let error: PyErr = compile_error(context.classes, &message, Some(MISSING_MANIFEST_CODE));
        error.value(py).setattr("help", MISSING_MANIFEST_HELP)?;
        return Ok(Some(error));
    }
    let arguments: Bound<'_, PyDict> = PyDict::new(py);
    arguments.set_item("ref_kind", context.classes.reference_kind.call1((kind,))?)?;
    arguments.set_item("ref_name", name)?;
    arguments.set_item("ref_package", package)?;
    arguments.set_item(
        "owner_relative_sql_path",
        context.classes.path_class.call1((relative_path,))?,
    )?;
    match resolver.call_method(VALIDATE_METHOD, (), Some(&arguments)) {
        Ok(_) => Ok(None),
        Err(error)
            if error
                .value(py)
                .is_instance(&context.classes.compile_error)? =>
        {
            Ok(Some(error))
        }
        Err(error) => Err(error),
    }
}
