//! Boundary conversions for the column migration planner.

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::model_config::typed_config::{config_text_object, model_values};
use crate::bindings::_helpers::planner_identity::hashing::model_definition_hash;
use crate::bindings::_helpers::planner_phases::objects::object;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;

pub(super) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

pub(super) fn lower(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.call_method0("lower")?.extract()
}

pub(super) fn replace<'py>(
    value: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(value.py());
    for (name, field) in fields {
        kwargs.set_item(name, field)?;
    }
    replaced_keywords(value, &kwargs)
}

pub(super) fn model<'py>(
    py: Python<'py>,
    name: RecordId,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, name, fields)
}

pub(super) fn native<'py>(
    py: Python<'py>,
    name: RecordId,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, name, fields)
}

pub(super) fn config<'py>(model: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    Ok(config_text_object(py, &model_values(model)?, key)?
        .unwrap_or_else(|| py.None().into_bound(py)))
}

pub(super) fn definition_hash<'py>(
    model: &Bound<'py, PyAny>,
    snapshot: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    object(
        model.py(),
        model_definition_hash(
            model.py(),
            &model_reads::name_object(model)?.extract::<String>()?,
            &model_reads::query_sql(model)?.extract::<String>()?,
            native_field(snapshot, "column_dialect")?
                .extract::<Option<String>>()?
                .as_deref(),
        )?,
    )
}
