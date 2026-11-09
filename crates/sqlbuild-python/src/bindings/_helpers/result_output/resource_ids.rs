//! Canonical execution-protocol identities of SQL test cases and attached audits.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::records::fields::field as native_field;
use sqlbuild_core::nested_progress::main::audit_resource_id::audit_resource_id as audit_id;
use sqlbuild_core::nested_progress::main::sql_test_resource_id::sql_test_resource_id as sql_test_id;

/// The exact stable identity of one plain or parameterised SQL test.
#[pyfunction]
#[pyo3(signature = (*, test_name, source_path, block_index, case_name))]
pub(crate) fn sql_test_resource_id(
    test_name: &str,
    source_path: Bound<'_, PyAny>,
    block_index: Option<i64>,
    case_name: Option<String>,
) -> PyResult<String> {
    let path: Option<String> = if source_path.is_none() {
        None
    } else {
        Some(source_path.call_method0("as_posix")?.extract()?)
    };
    Ok(match (path, block_index, case_name) {
        (Some(path), Some(block), Some(case)) => {
            sql_test_id(test_name, Some((&path, block, &case)))
        }
        _ => sql_test_id(test_name, None),
    })
}

/// The stable identity of one attached audit execution.
#[pyfunction]
#[pyo3(signature = (**options))]
fn audit_resource_id(options: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("audit_resource_id", options)?;
    let target_kind: Option<Bound<'_, PyAny>> = request
        .optional("attached_target_kind")?
        .filter(|kind| !kind.is_none());
    let kind: Bound<'_, PyAny> = match target_kind {
        Some(kind) => kind,
        None => request.get("attachment_kind")?,
    };
    audit_identity_of(
        &request.get("audit_name")?,
        &kind,
        (
            &request.get("attached_target_name")?,
            &request.get("attached_column_name")?,
        ),
    )
}

fn audit_identity_of(
    audit_name: &Bound<'_, PyAny>,
    kind: &Bound<'_, PyAny>,
    (target, column): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<String> {
    let target: Option<String> = target.extract()?;
    let column: Option<String> = column.extract()?;
    Ok(audit_id(
        &audit_name.extract::<String>()?,
        &native_field(kind, "value")?.extract::<String>()?,
        (target.as_deref(), column.as_deref()),
    ))
}

/// `sql_test_resource_id` of one result or plan entry carrying `test_name_field` and case fields.
pub(crate) fn sql_test_identity(
    result: &Bound<'_, PyAny>,
    test_name_field: &str,
) -> PyResult<String> {
    sql_test_resource_id(
        &native_field(result, test_name_field)?.extract::<String>()?,
        native_field(result, "source_path")?,
        native_field(result, "block_index")?.extract()?,
        native_field(result, "case_name")?.extract()?,
    )
}

/// `audit_resource_id` of one audit result or plan entry, whose name is in `audit_name_field`.
pub(crate) fn audit_identity(
    result: &Bound<'_, PyAny>,
    audit_name_field: &str,
) -> PyResult<String> {
    let target_kind: Bound<'_, PyAny> = native_field(result, "attached_target_kind")?;
    let kind: Bound<'_, PyAny> = if target_kind.is_none() {
        native_field(result, "attachment_kind")?
    } else {
        target_kind
    };
    audit_identity_of(
        &native_field(result, audit_name_field)?,
        &kind,
        (
            &native_field(result, "attached_target_name")?,
            &native_field(result, "attached_column_name")?,
        ),
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(sql_test_resource_id, module)?)?;
    module.add_function(wrap_pyfunction!(audit_resource_id, module)?)?;
    Ok(())
}
