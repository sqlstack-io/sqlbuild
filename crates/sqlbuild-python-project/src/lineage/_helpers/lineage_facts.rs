//! Sequence behaviour of the natively hosted `CompactLineageFacts` record.

use pyo3::exceptions::PyIndexError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use sqlbuild_python_base::positional_record;

use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

/// `ColumnTransformKind` members by native transform code.
pub(crate) const TRANSFORM_KINDS: [&str; 6] = [
    "DIRECT",
    "CAST",
    "EXPRESSION",
    "AGGREGATION",
    "STAR",
    "CONSTANT",
];
/// `ColumnLineageConfidence` members by native confidence code.
pub(crate) const CONFIDENCES: [&str; 3] = ["UNKNOWN", "HIGH", "MEDIUM"];

/// The enum member at `code` of `members`.
pub(crate) fn coded_member(
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
pub(crate) fn resource_name<'py>(
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
pub(crate) fn project_row(facts: &Bound<'_, PyAny>, row: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
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
