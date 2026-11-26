//! The retention and table type a model inherits from its target and materialization defaults.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyString, PyStringMethods, PyTuple};

use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// `value`, unless it is `None`.
fn present(value: Bound<'_, PyAny>) -> Option<Bound<'_, PyAny>> {
    Some(value).filter(|value| !value.is_none())
}

fn source<'py>(py: Python<'py>, class: RecordId, member: &str) -> PyResult<Bound<'py, PyAny>> {
    enum_member(py, class, member)
}

/// The `materialized` defaults entry, or `None` for a model without a table-backed materialization.
fn defaults_entry<'py>(
    materialized: &Bound<'py, PyAny>,
    materialization_defaults: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if materialized.is_none() {
        return Ok(None);
    }
    let name: &Bound<'py, PyString> = materialized.cast()?;
    native_field(materialization_defaults, &name.to_cow()?).map(Some)
}

fn retention<'py>(
    materialized: &Bound<'py, PyAny>,
    entry: Option<&Bound<'py, PyAny>>,
    target: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = materialized.py();
    let class = RecordId::TimeTravelRetentionSource;
    let mut policy: Option<(Bound<'py, PyAny>, &str)> = None;
    if let Some(entry) = entry {
        if !target.is_none() {
            policy = present(native_field(target, "time_travel_retention")?).map(|p| (p, "TARGET"));
        }
        if let Some(found) = present(native_field(entry, "time_travel_retention")?) {
            policy = Some((found, "MATERIALIZATION"));
        }
        if !target.is_none()
            && let Some(found) = present(
                native_field(target, "time_travel_retention_by_materialization")?
                    .call_method1("get", (materialized,))?,
            )
        {
            policy = Some((found, "TARGET"));
        }
    }
    let Some((policy, member)) = policy else {
        return new_record(py, RecordId::ResolvedTimeTravelRetention, (), None);
    };
    new_record_with(
        py,
        RecordId::ResolvedTimeTravelRetention,
        &[
            ("desired_days", native_field(&policy, "desired_days")?),
            ("unmanaged", native_field(&policy, "unmanaged")?),
            ("source", source(py, class, member)?),
        ],
    )
}

fn table_type<'py>(
    entry: Option<&Bound<'py, PyAny>>,
    target: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target.py();
    let class = RecordId::TableTypeSource;
    let mut declared: Option<(Bound<'py, PyAny>, &str)> = None;
    if let Some(entry) = entry {
        if !target.is_none() {
            declared = present(native_field(target, "default_table_type")?).map(|v| (v, "TARGET"));
        }
        if let Some(found) = present(native_field(entry, "table_type")?) {
            declared = Some((found, "MATERIALIZATION"));
        }
    }
    let Some((value, member)) = declared else {
        return new_record(py, RecordId::ResolvedTableType, (), None);
    };
    new_record_with(
        py,
        RecordId::ResolvedTableType,
        &[
            ("value", value),
            ("source", source(py, class, member)?),
            ("declared", PyBool::new(py, true).to_owned().into_any()),
        ],
    )
}

/// The `(retention, table type)` a model with `materialized` inherits; `None` is not table-backed.
pub(crate) fn inherited_storage<'py>(
    materialized: &Bound<'py, PyAny>,
    materialization_defaults: &Bound<'py, PyAny>,
    target: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let entry: Option<Bound<'py, PyAny>> = defaults_entry(materialized, materialization_defaults)?;
    PyTuple::new(
        materialized.py(),
        [
            retention(materialized, entry.as_ref(), target)?,
            table_type(entry.as_ref(), target)?,
        ],
    )
}
