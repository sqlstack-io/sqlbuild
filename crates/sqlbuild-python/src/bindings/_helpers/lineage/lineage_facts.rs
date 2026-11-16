//! Sequence behaviour of the natively hosted `CompactLineageFacts` record.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PySlice, PyTuple, PyTupleMethods};
use sqlbuild_python_base::positional_record;

use sqlbuild_python_base::boundary::main::call_argument::call_argument;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

/// `ColumnTransformKind` members by native transform code.
const TRANSFORM_KINDS: [&str; 6] = [
    "DIRECT",
    "CAST",
    "EXPRESSION",
    "AGGREGATION",
    "STAR",
    "CONSTANT",
];
/// `ColumnLineageConfidence` members by native confidence code.
const CONFIDENCES: [&str; 3] = ["UNKNOWN", "HIGH", "MEDIUM"];

/// The `ColumnTransformKind` member of a native transform code.
pub(crate) fn transform_kind_member(py: Python<'_>, code: usize) -> PyResult<Py<PyAny>> {
    coded_member(py, (RecordId::ColumnTransformKind, &TRANSFORM_KINDS), code)
}

/// The `ColumnLineageConfidence` member of a native confidence code.
pub(crate) fn confidence_member(py: Python<'_>, code: usize) -> PyResult<Py<PyAny>> {
    coded_member(py, (RecordId::ColumnLineageConfidence, &CONFIDENCES), code)
}

/// The enum member at `code` of `members`.
fn coded_member(
    py: Python<'_>,
    (class, members): (RecordId, &[&str]),
    code: usize,
) -> PyResult<Py<PyAny>> {
    let Some(member) = members.get(code) else {
        return Err(PyIndexError::new_err("tuple index out of range"));
    };
    Ok(enum_member(py, class, member)?.unbind())
}

/// `CompactLineageFacts.resource_name(index)`: the model's name for a native relation index.
fn resource_name<'py>(
    facts: &Bound<'py, PyAny>,
    index: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let names: Bound<'py, PyAny> = native_field(facts, "resource_names")?;
    let name: Bound<'py, PyAny> = mapping_get(&names, index)?;
    if !name.is_none() {
        return Ok(name);
    }
    let pooled: Bound<'py, PyAny> =
        native_field(facts, "resource_name_indexes")?.call_method1("get", (index, index))?;
    native_field(facts, "string_pool")?.get_item(pooled)
}

/// `CompactLineageFacts.resource_name(index)`.
pub(crate) fn facts_resource_name(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let index: Bound<'_, PyAny> = call_argument((args, kwargs), "index", 0)?;
    Ok(resource_name(instance, &index)?.unbind())
}

positional_record! {
    extract;
    /// One compact lineage row: output name index, transform and confidence codes, sources.
    struct CompactColumnRow<'py> {
        name_index: Bound<'py, PyAny>,
        transform_code: usize,
        confidence_code: usize,
        sources: Bound<'py, PyAny>,
    }
}

/// One compact row projected to a `CompiledLineageColumnFact`.
fn project_row(facts: &Bound<'_, PyAny>, row: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = facts.py();
    let pool: Bound<'_, PyAny> = native_field(facts, "string_pool")?;
    let CompactColumnRow {
        name_index,
        transform_code,
        confidence_code,
        sources,
    }: CompactColumnRow<'_> = row.extract()?;
    let mut upstream: Vec<Bound<'_, PyAny>> = Vec::new();
    for source in sources.try_iter()? {
        let source: Bound<'_, PyTuple> = source?.cast_into::<PyTuple>()?;
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("resource_type", pool.get_item(source.get_item(0)?)?)?;
        keywords.set_item("resource_name", resource_name(facts, &source.get_item(1)?)?)?;
        keywords.set_item("column_name", pool.get_item(source.get_item(2)?)?)?;
        upstream.push(new_record(
            py,
            RecordId::CompiledLineageSourceFact,
            (),
            Some(&keywords),
        )?);
    }
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("output_column", pool.get_item(name_index)?)?;
    keywords.set_item("upstream_columns", PyTuple::new(py, upstream)?)?;
    keywords.set_item(
        "transform_kind",
        coded_member(
            py,
            (RecordId::ColumnTransformKind, &TRANSFORM_KINDS),
            transform_code,
        )?,
    )?;
    keywords.set_item(
        "confidence",
        coded_member(
            py,
            (RecordId::ColumnLineageConfidence, &CONFIDENCES),
            confidence_code,
        )?,
    )?;
    Ok(new_record(py, RecordId::CompiledLineageColumnFact, (), Some(&keywords))?.unbind())
}

/// `CompactLineageFacts.facts`: every row projected once, in order.
pub(crate) fn facts_projected(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let projected: Vec<Py<PyAny>> = native_field(instance, "rows")?
        .try_iter()?
        .map(|row| project_row(instance, &row?))
        .collect::<PyResult<_>>()?;
    Ok(PyTuple::new(instance.py(), projected)?.into_any().unbind())
}

/// `len(facts)`.
pub(crate) fn facts_len(
    instance: &Bound<'_, PyAny>,
    _args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    native_field(instance, "rows")?
        .len()?
        .into_py_any(instance.py())
}

/// `facts[index]`: one fact, or a tuple of facts for a slice.
pub(crate) fn facts_getitem(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let index: Bound<'_, PyAny> = args.get_item(0)?;
    let projected: Bound<'_, PyAny> = native_field(instance, "facts")?;
    if index.is_instance_of::<PySlice>() {
        return Ok(projected.get_item(index)?.unbind());
    }
    let length: isize = isize::try_from(projected.len()?)?;
    let requested: isize = index.extract()?;
    let normalized: isize = if requested >= 0 {
        requested
    } else {
        length + requested
    };
    if normalized < 0 || normalized >= length {
        return Err(PyIndexError::new_err(index.unbind()));
    }
    Ok(projected.get_item(normalized)?.unbind())
}

/// `iter(facts)`.
pub(crate) fn facts_iter(
    instance: &Bound<'_, PyAny>,
    _args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    Ok(native_field(instance, "facts")?
        .try_iter()?
        .into_any()
        .unbind())
}

/// `facts == other`: item-wise equality with any sequence.
pub(crate) fn facts_eq(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let other: Bound<'_, PyAny> = args.get_item(0)?;
    let sequence: Bound<'_, PyAny> = py.import("collections.abc")?.getattr("Sequence")?;
    if !other.is_instance(&sequence)? {
        return Ok(py.NotImplemented());
    }
    let mine: Bound<'_, PyAny> = native_field(instance, "facts")?;
    let theirs: Bound<'_, PyTuple> = PyTuple::new(
        py,
        other
            .try_iter()?
            .collect::<PyResult<Vec<Bound<'_, PyAny>>>>()?,
    )?;
    mine.eq(theirs)?.into_py_any(py)
}
