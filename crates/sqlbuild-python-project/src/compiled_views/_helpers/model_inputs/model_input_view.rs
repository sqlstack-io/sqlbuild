//! A model's compile input, read from the native compiled project that owns it.

use crate::compiled_views::main::model_inputs::reference_tuple::reference_tuple;
use crate::compiled_views::models::InputViewClasses;
use crate::compiled_views::types::ModelInputRecord;
use sqlbuild_project::models::ExpansionRecord;
use std::sync::Arc;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::_helpers::compiled_models::relation_keys::relation_key_row;
use crate::compiled_views::models::RelationKeyRow;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::main::records::projected_text::projected_text;
use sqlbuild_python_front::discovery::models::NativeModelFile;
use sqlbuild_python_front::render::main::calls::span_tuple::span_tuple;
use sqlbuild_python_front::render::types::SpanRow;

const MODEL_RELATION_KIND: &str = "ref";

/// Read the input record at `index` of `owner` together with `owner`.
pub(crate) fn read_input_with_owner<T>(
    owner: &Py<NativeCompiledProject>,
    index: usize,
    py: Python<'_>,
    read: impl FnOnce(&NativeCompiledProject, &ModelInputRecord) -> PyResult<T>,
) -> PyResult<T> {
    let owner = owner.bind(py).borrow();
    read(&owner, owner.input(index)?)
}

/// Read the input record at `index` of `owner`.
pub(crate) fn read_input<'py, T>(
    owner: &Py<NativeCompiledProject>,
    index: usize,
    py: Python<'py>,
    read: impl FnOnce(&ModelInputRecord, Python<'py>) -> PyResult<T>,
) -> PyResult<T> {
    let owner = owner.bind(py).borrow();
    read(owner.input(index)?, py)
}

/// The compiled query as the Python string every reader shares; the authored query's when equal.
pub(crate) fn query_sql_text<'py>(
    py: Python<'py>,
    record: &ModelInputRecord,
) -> Bound<'py, PyString> {
    let model_file: &NativeModelFile = record.host.model_file.get();
    if Arc::ptr_eq(&record.query_sql, model_file.query_text()) {
        return model_file.query_sql(py);
    }
    projected_text(py, &record.host.projections.query_sql, &record.query_sql)
}

/// The authored-to-expanded passes, when interpolation left the authored query unchanged.
pub(crate) fn authored_expansion(
    classes: &InputViewClasses<'_>,
    record: &ModelInputRecord,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = classes.sql_expansion.py();
    record
        .host
        .projections
        .sql_expansion
        .get_or_try_init(py, || {
            let expansion: &ExpansionRecord = &record.expansion;
            if !expansion.authored {
                return Ok(py.None());
            }
            expansion_object(
                classes,
                expansion,
                &[&expansion.declaration_spans, &expansion.macro_spans],
            )
        })
        .map(|expansion| expansion.clone_ref(py))
}

/// Where a rejected `sql_analysis false` was authored, or `None`.
pub(crate) fn rejected_opt_out(record: &ModelInputRecord, py: Python<'_>) -> Py<PyAny> {
    record
        .host
        .rejected_opt_out
        .as_ref()
        .map_or_else(|| py.None(), |location| location.clone_ref(py))
}

/// The model's `CompileSqlReference` tuple, built once.
pub(crate) fn references(
    classes: &InputViewClasses<'_>,
    record: &ModelInputRecord,
) -> PyResult<Py<PyTuple>> {
    reference_tuple(
        classes,
        &record.references,
        &record.host.projections.references,
    )
}

/// The model's declared key facts: its config and attached schema columns and audits.
pub(crate) fn model_relation_key_row(
    owner: &NativeCompiledProject,
    record: &ModelInputRecord,
    py: Python<'_>,
) -> PyResult<RelationKeyRow> {
    let values: Bound<'_, PyAny> = native_field(record.host.config.bind(py), "values")?;
    let declared = match (&record.schema, owner.input_schema_fact(record)) {
        (Some(schema), Some(fact)) => {
            Some((fact.columns.as_slice(), schema.host.audits.key_rows(py)?))
        }
        _ => None,
    };
    relation_key_row(
        MODEL_RELATION_KIND,
        record.name.clone(),
        values.downcast::<PyDict>()?,
        declared,
    )
}

/// The name of the model's attached schema entry, its file stem, or `None`.
pub(crate) fn attached_schema_name(record: &ModelInputRecord) -> Option<String> {
    record.schema.as_ref().map(|_| record.name.clone())
}

/// The model's attached `SchemaModelEntry`, or `None`, built once.
pub(crate) fn schema_entry(
    classes: &InputViewClasses<'_>,
    record: &ModelInputRecord,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = classes.schema_model_entry.py();
    record
        .host
        .projections
        .schema_entry
        .get_or_try_init(py, || {
            let Some(schema) = &record.schema else {
                return Ok::<_, pyo3::PyErr>(py.None());
            };
            let fields: Bound<'_, PyDict> = PyDict::new(py);
            fields.set_item("name", &record.name)?;
            fields.set_item("model_schema", &schema.model_schema)?;
            fields.set_item("description", &schema.description)?;
            fields.set_item("type_enforcement", schema.type_enforcement.then_some(true))?;
            fields.set_item("columns", schema.host.columns.project(py)?)?;
            fields.set_item("dynamic_columns", schema.host.dynamic_columns.project(py)?)?;
            fields.set_item("audits", schema.host.audits.project(py)?)?;
            Ok(classes.schema_model_entry.call((), Some(&fields))?.unbind())
        })
        .map(|entry| entry.clone_ref(py))
}

/// `CompiledSqlExpansion(authored_sql, expanded_sql, passes)` for these passes.
pub(crate) fn expansion_object(
    classes: &InputViewClasses<'_>,
    expansion: &ExpansionRecord,
    passes: &[&Vec<SpanRow>],
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = classes.sql_expansion.py();
    let mut pass_objects: Vec<Bound<'_, PyTuple>> = Vec::with_capacity(passes.len());
    for spans in passes {
        let mut span_objects: Vec<Bound<'_, PyAny>> = Vec::with_capacity(spans.len());
        for span in spans.iter() {
            span_objects.push(
                classes
                    .expansion_span
                    .call1(span_tuple(span).into_pyobject(py)?)?,
            );
        }
        pass_objects.push(PyTuple::new(py, span_objects)?);
    }
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("authored_sql", &*expansion.authored_sql)?;
    fields.set_item("expanded_sql", &*expansion.expanded_sql)?;
    fields.set_item("passes", PyTuple::new(py, pass_objects)?)?;
    Ok(classes.sql_expansion.call((), Some(&fields))?.unbind())
}

/// One `CompileSqlReference` record as its row.
pub(crate) fn reference_row_of(reference: &Bound<'_, PyAny>) -> PyResult<ReferenceRow> {
    Ok(ReferenceRow {
        kind: native_field(reference, "ref_kind")?.str()?.extract()?,
        name: native_field(reference, "ref_name")?.extract()?,
        package: native_field(reference, "ref_package")?.extract()?,
        call_argument_count: native_field(reference, "call_argument_count")?.extract()?,
        call_partitioned: native_field(reference, "call_partitioned")?.extract()?,
    })
}
