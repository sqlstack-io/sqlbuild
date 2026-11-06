//! Boundary helpers shared by the native planner phases.

use sqlbuild_host::entries::models::HostId;

use crate::bindings::_helpers::boundary::python_calls::{call_with, keywords};
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::resource_reads;
use crate::bindings::_helpers::model_config::typed_config::{config_text, model_values};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{enum_member, new_record, new_record_with};
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyFrozenSet, PyString, PyTuple};
use pyo3::{BoundObject, IntoPyObject};

/// A Python `str`.
pub(crate) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

/// `None`, or the text.
pub(crate) fn optional_text<'py>(py: Python<'py>, value: Option<&str>) -> Bound<'py, PyAny> {
    value.map_or_else(|| py.None().into_bound(py), |value| text(py, value))
}

/// The staying-Python class `class(**fields)`.
pub(crate) fn construct<'py>(
    py: Python<'py>,
    class: impl Into<HostId>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    call_with(&host::entry(py, class)?, fields)
}

/// A planner model dataclass.
pub(crate) fn planner_model<'py>(
    py: Python<'py>,
    name: RecordId,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, name, fields)
}

/// The native record `name(**fields)`.
pub(crate) fn native<'py>(
    py: Python<'py>,
    name: RecordId,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, name, fields)
}

/// `dataclasses.replace(value, **fields)`.
pub(crate) fn replace<'py>(
    value: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    replaced_keywords(value, &keywords(value.py(), fields)?)
}

/// A staying-Python enum member from its value.
pub(crate) fn member<'py>(
    py: Python<'py>,
    class: impl Into<HostId>,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    host::entry(py, class)?.call1((value,))
}

/// A native string enum member from its value.
pub(crate) fn native_member<'py>(
    py: Python<'py>,
    class: RecordId,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    new_record(py, class, (value,), None)
}

/// `PlannerInputError(message, code=code, help=help)`.
pub(crate) fn planner_error(
    py: Python<'_>,
    message: &str,
    (code, help): (Option<&str>, Option<&str>),
) -> PyErr {
    let raised = error_class(py, ErrorId::PlannerInput).and_then(|class| {
        let fields = keywords(
            py,
            &[
                ("code", optional_text(py, code)),
                ("help", optional_text(py, help)),
            ],
        )?;
        class.call((message,), Some(&fields))
    });
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// A tuple of the items.
pub(crate) fn tuple<'py>(
    py: Python<'py>,
    items: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, items)?.into_any())
}

/// A frozenset of the items.
pub(crate) fn frozenset<'py>(
    py: Python<'py>,
    items: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyFrozenSet::new(py, items)?.into_any())
}

/// `sorted(items)` as a tuple of strings.
pub(crate) fn sorted_names<'py>(
    py: Python<'py>,
    mut names: Vec<String>,
) -> PyResult<Bound<'py, PyAny>> {
    names.sort();
    tuple(py, names.iter().map(|name| text(py, name)).collect())
}

/// The strings of any Python iterable, in iteration order.
pub(crate) fn strings(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values
        .try_iter()?
        .map(|value| value?.extract::<String>())
        .collect()
}

/// The project nodes of `field` by name.
pub(crate) fn by_name<'py>(
    project: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let nodes = pyo3::types::PyDict::new(project.py());
    for node in project_reads::collection(project, field)?.try_iter()? {
        let node = node?;
        pyo3::types::PyDictMethods::set_item(&nodes, resource_reads::name(&node)?, node)?;
    }
    Ok(nodes)
}

/// A model config value as text, `None` when absent or not text.
pub(crate) fn config_str(model: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    config_text(model.py(), &model_values(model)?, key)
}

/// The compiled object key of the model `name`.
pub(crate) fn model_key<'py>(
    py: Python<'py>,
    name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::CompiledObjectKey,
        &[
            (
                "resource_type",
                enum_member(py, RecordId::CompiledResourceType, "MODEL")?,
            ),
            ("name", name.clone()),
        ],
    )
}

/// A native helper's Rust result as a Python object.
pub(crate) fn object<'py, T>(py: Python<'py>, value: T) -> PyResult<Bound<'py, PyAny>>
where
    T: IntoPyObject<'py>,
    PyErr: From<T::Error>,
{
    Ok(value.into_pyobject(py)?.into_bound().into_any())
}
