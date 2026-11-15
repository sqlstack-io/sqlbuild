//! Typed SQL value normalization between Python authored values and native `SqlValue`s.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyListMethods, PyString,
    PyStringMethods, PyTuple,
};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_core::models::{SqlLogicalType, SqlValue};
use sqlbuild_model_config::sql_values::errors::SqlValueFailure;
use sqlbuild_model_config::sql_values::main::normalize_sql_value::normalize_sql_value as normalized;
use sqlbuild_model_config::sql_values::models::{RawKey, RawSqlValue, SqlValueLimits};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;
use sqlbuild_python_base::records::types::RecordId;

/// The Python classes authored values are told apart by and normalized values are built from.
pub(crate) struct SqlValueClasses<'py> {
    decimal: Bound<'py, PyAny>,
    authored_set: Bound<'py, PyAny>,
    value_call: Bound<'py, PyAny>,
    sql_value: Bound<'py, PyAny>,
    logical_type: Bound<'py, PyAny>,
    kind: Bound<'py, PyAny>,
}

impl<'py> SqlValueClasses<'py> {
    pub(crate) fn load(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            decimal: py.import("decimal")?.getattr("Decimal")?,
            authored_set: record_class(py, RecordId::AuthoredSqlSet)?,
            value_call: record_class(py, RecordId::AuthoredSqlValueCall)?,
            sql_value: record_class(py, RecordId::SqlValue)?,
            logical_type: record_class(py, RecordId::SqlLogicalType)?,
            kind: record_class(py, RecordId::SqlValueKind)?,
        })
    }
}

fn is_exactly(value: &Bound<'_, PyAny>, class: &Bound<'_, PyAny>) -> bool {
    value.get_type().is(class)
}

fn sequence<'py>(
    items: impl Iterator<Item = Bound<'py, PyAny>>,
    classes: &SqlValueClasses<'_>,
) -> PyResult<Vec<RawSqlValue>> {
    items.map(|item| raw_value(&item, classes)).collect()
}

/// An authored Python value read by exact type, as `normalize_sql_value` dispatches.
pub(crate) fn raw_value(
    value: &Bound<'_, PyAny>,
    classes: &SqlValueClasses<'_>,
) -> PyResult<RawSqlValue> {
    let py = value.py();
    if value.is_none() {
        return Ok(RawSqlValue::Null);
    }
    if is_exactly(value, &py.get_type::<PyBool>().into_any()) {
        return Ok(RawSqlValue::Bool(value.extract()?));
    }
    if is_exactly(value, &py.get_type::<PyInt>().into_any()) {
        return Ok(match value.extract::<i64>() {
            Ok(number) => RawSqlValue::Int(number),
            Err(_) => RawSqlValue::BigInt(value.str()?.to_string_lossy().into_owned()),
        });
    }
    if is_exactly(value, &py.get_type::<PyFloat>().into_any()) {
        return Ok(RawSqlValue::Float(value.extract()?));
    }
    if is_exactly(value, &classes.decimal) {
        return Ok(RawSqlValue::Decimal(
            value.str()?.to_string_lossy().into_owned(),
        ));
    }
    if is_exactly(value, &py.get_type::<PyString>().into_any()) {
        return Ok(RawSqlValue::Str(
            value.downcast::<PyString>()?.to_string_lossy().into_owned(),
        ));
    }
    if is_exactly(value, &py.get_type::<PyList>().into_any()) {
        return sequence(value.downcast::<PyList>()?.iter(), classes).map(RawSqlValue::List);
    }
    if value.is_instance(&classes.authored_set)? {
        let items = native_field(value, "values")?;
        return sequence(
            items.try_iter()?.collect::<PyResult<Vec<_>>>()?.into_iter(),
            classes,
        )
        .map(RawSqlValue::Set);
    }
    if value.is_instance(&classes.value_call)? {
        let mut arguments: Vec<(String, RawSqlValue)> = Vec::new();
        for argument in native_field(value, "arguments")?.try_iter()? {
            let (name, item): (String, Bound<'_, PyAny>) = argument?.extract()?;
            arguments.push((name, raw_value(&item, classes)?));
        }
        return Ok(RawSqlValue::Call(arguments));
    }
    if is_exactly(value, &py.get_type::<PyDict>().into_any()) {
        let mut entries: Vec<(RawKey, RawSqlValue)> = Vec::new();
        for (key, item) in value.downcast::<PyDict>()?.iter() {
            let raw_key = if is_exactly(&key, &py.get_type::<PyString>().into_any()) {
                RawKey::Str(key.downcast::<PyString>()?.to_string_lossy().into_owned())
            } else {
                RawKey::Other(key.repr()?.to_string_lossy().into_owned())
            };
            entries.push((raw_key, raw_value(&item, classes)?));
        }
        return Ok(RawSqlValue::Object(entries));
    }
    let name = value.get_type().getattr("__name__")?;
    Ok(RawSqlValue::Other(
        name.str()?.to_string_lossy().into_owned(),
    ))
}

fn logical_type<'py>(
    logical: &SqlLogicalType,
    classes: &SqlValueClasses<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let kind = classes.kind.call1((logical.kind.as_str(),))?;
    let element = match &logical.element_type {
        Some(element) => logical_type(element, classes)?,
        None => classes.kind.py().None().into_bound(classes.kind.py()),
    };
    classes.logical_type.call1((kind, element))
}

fn payload<'py>(value: &SqlValue, classes: &SqlValueClasses<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py = classes.kind.py();
    Ok(match value {
        SqlValue::Null => py.None().into_bound(py),
        SqlValue::Boolean(flag) => flag.into_bound_py_any(py)?,
        SqlValue::Integer(number) => number.into_bound_py_any(py)?,
        SqlValue::Float(number) => number.into_bound_py_any(py)?,
        SqlValue::Decimal(text) => classes.decimal.call1((text,))?,
        SqlValue::String(text) => text.into_bound_py_any(py)?,
        SqlValue::List { values, .. } | SqlValue::Set { values, .. } => PyTuple::new(
            py,
            values
                .iter()
                .map(|item| sql_value(item, classes))
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any(),
        SqlValue::Object(entries) => PyTuple::new(
            py,
            entries
                .iter()
                .map(|(key, item)| {
                    PyTuple::new(py, [key.into_bound_py_any(py)?, sql_value(item, classes)?])
                })
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any(),
    })
}

/// A Python `SqlValue` for a native one.
pub(crate) fn sql_value<'py>(
    value: &SqlValue,
    classes: &SqlValueClasses<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    classes.sql_value.call1((
        logical_type(&value.logical_type(), classes)?,
        payload(value, classes)?,
    ))
}

/// The exception a normalization failure raises.
pub(crate) fn sql_value_error(py: Python<'_>, failure: SqlValueFailure) -> PyResult<PyErr> {
    Ok(match failure {
        SqlValueFailure::Invalid(message) => PyErr::from_value(
            native_attr(py, NativeAttr::SqlValueValidationError)?.call1((message,))?,
        ),
        SqlValueFailure::DecimalOverflow => {
            let overflow = py.import("decimal")?.getattr("Overflow")?;
            let classes = pyo3::types::PyList::new(py, [overflow.clone()])?;
            PyErr::from_value(overflow.call1((classes,))?)
        }
    })
}

/// `normalize_sql_value`: the canonical `SqlValue` of an authored value.
#[pyfunction]
#[pyo3(signature = (*, raw_value, context, explicit_type = None, limits = None))]
fn normalize_sql_value<'py>(
    raw_value: &Bound<'py, PyAny>,
    context: &str,
    explicit_type: Option<&str>,
    limits: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw_value.py();
    let classes = SqlValueClasses::load(py)?;
    let raw = self::raw_value(raw_value, &classes)?;
    let applied: Bound<'py, PyAny> = match limits {
        Some(limits) if !limits.is_none() => limits.clone(),
        _ => new_record(py, RecordId::SqlValueLimits, (), None)?,
    };
    let limits = SqlValueLimits {
        max_depth: native_field(&applied, "max_depth")?.extract()?,
        max_elements: native_field(&applied, "max_elements")?.extract()?,
        max_size: native_field(&applied, "max_size")?.extract()?,
    };
    match normalized(&raw, context, explicit_type, limits) {
        Ok(value) => sql_value(&value, &classes),
        Err(failure) => Err(sql_value_error(py, failure)?),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(normalize_sql_value, module)?)
}
