//! A model's compile input, read from the native compiled project that owns it.

use crate::compiled_views::models::AttachedSchema;
use crate::compiled_views::models::NativeModelInput;
use crate::compiled_views::types::ModelInputRecord;
use sqlbuild_project::models::ExpansionRecord;
use std::sync::Arc;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{FromPyObject, IntoPyObject};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_analysis::sql_macros::main::typed_analysis_sql::typed_analysis_sql;
use sqlbuild_analysis::sql_macros::models::SqlMacroCheck;

use crate::compiled_project::main::schema_facts::python_column_facts::python_column_facts;
use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::_helpers::relation_keys::relation_key_row;
use crate::compiled_views::models::CompiledViewClasses;
use crate::compiled_views::models::RelationKeyRow;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::main::records::projected_text::projected_text;
use sqlbuild_python_front::discovery::models::NativeModelFile;
use sqlbuild_python_front::model_config::main::headers::family_specs::family_specs;
use sqlbuild_python_front::render::main::calls::span_tuple::span_tuple;
use sqlbuild_python_front::render::types::SpanRow;

const MODEL_RELATION_KIND: &str = "ref";

/// The Python classes a model input's projected attributes are built with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct InputViewClasses<'py> {
    pub(crate) compiled: CompiledViewClasses<'py>,
    pub(crate) sql_reference: Bound<'py, PyAny>,
    pub(crate) reference_kind: Bound<'py, PyAny>,
    pub(crate) schema_model_entry: Bound<'py, PyAny>,
    pub(crate) sql_expansion: Bound<'py, PyAny>,
    pub(crate) expansion_span: Bound<'py, PyAny>,
}

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

/// The classes the owner's views project attributes with.
pub(crate) fn view_classes<'py>(
    owner: &Py<NativeCompiledProject>,
    py: Python<'py>,
) -> PyResult<InputViewClasses<'py>> {
    owner.bind(py).borrow().input_classes(py)?.extract()
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

/// `CompileSqlReference` records of `rows`, built once into `cache`.
pub(crate) fn reference_tuple(
    classes: &InputViewClasses<'_>,
    rows: &[ReferenceRow],
    cache: &PyOnceLock<Py<PyTuple>>,
) -> PyResult<Py<PyTuple>> {
    let py: Python<'_> = classes.sql_reference.py();
    cache
        .get_or_try_init(py, || {
            let mut references: Vec<Bound<'_, PyAny>> = Vec::with_capacity(rows.len());
            for ReferenceRow {
                kind,
                name,
                package,
                call_argument_count: count,
            } in rows
            {
                let fields: Bound<'_, PyDict> = PyDict::new(py);
                fields.set_item("ref_kind", classes.reference_kind.call1((kind,))?)?;
                fields.set_item("ref_name", name)?;
                fields.set_item("ref_package", package)?;
                fields.set_item("call_argument_count", count)?;
                references.push(classes.sql_reference.call((), Some(&fields))?);
            }
            Ok::<_, pyo3::PyErr>(PyTuple::new(py, references)?.unbind())
        })
        .map(|references| references.clone_ref(py))
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

/// The attached schema of `model_input`; a native input answers without building its entry.
pub(crate) fn attached_schema<'py>(
    model_input: &Bound<'py, PyAny>,
) -> PyResult<Option<AttachedSchema>> {
    let py: Python<'py> = model_input.py();
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read(py, |record, py| {
            let Some(schema) = &record.schema else {
                return Ok(None);
            };
            Ok(Some(AttachedSchema {
                name: record.name.clone(),
                dynamic_columns: schema.host.dynamic_columns.read(py)?,
                type_enforcement: schema.type_enforcement,
            }))
        });
    }
    let entry: Bound<'py, PyAny> = native_field(model_input, "schema_entry")?;
    if entry.is_none() {
        return Ok(None);
    }
    Ok(Some(AttachedSchema {
        name: native_field(&entry, "name")?.extract()?,
        dynamic_columns: family_specs(&native_field(&entry, "dynamic_columns")?)?,
        type_enforcement: native_field(&entry, "type_enforcement")?.is_truthy()?,
    }))
}

/// The declared column facts of `model_input`'s attached schema, empty without one.
pub(crate) fn attached_columns(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<SchemaColumnFact>> {
    let py: Python<'_> = model_input.py();
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read_with_owner(py, |owner, record| {
            Ok(owner
                .input_schema_fact(record)
                .map(|fact| fact.columns.clone())
                .unwrap_or_default())
        });
    }
    let entry: Bound<'_, PyAny> = native_field(model_input, "schema_entry")?;
    if entry.is_none() {
        return Ok(Vec::new());
    }
    python_column_facts(&native_field(&entry, "columns")?, None)
}

/// The name of `model_input`, its file stem; a native input answers from its record.
pub(crate) fn input_model_name(model_input: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native
            .get()
            .read(model_input.py(), |record, _| Ok(record.name.clone()));
    }
    native_field(model_input, "model_file")?
        .getattr("file_path")?
        .getattr("stem")?
        .extract()
}

/// What the analysed SQL of a native model input must show for its SQL macro calls to fit.
pub(crate) fn input_sql_macro_checks(
    model_input: &Bound<'_, PyAny>,
) -> PyResult<Vec<SqlMacroCheck>> {
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read(model_input.py(), |record, _| {
            Ok(record.sql_macro_checks.clone())
        });
    }
    Ok(Vec::new())
}

/// The analysis text of `model_input`: its compiled query with each typed SQL macro call marked.
pub(crate) fn input_analysis_sql(
    model_input: &Bound<'_, PyAny>,
    dialect: &str,
) -> PyResult<String> {
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read(model_input.py(), |record, _| {
            Ok(typed_analysis_sql(&record.query_sql, &record.typed_calls, dialect).0)
        });
    }
    native_field(model_input, "query_sql")?.extract()
}

/// Each reference of `model_input` as `(kind, name, package, call argument count)`.
pub(crate) fn input_reference_rows(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<ReferenceRow>> {
    let py: Python<'_> = model_input.py();
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native
            .get()
            .read(py, |record, _| Ok(record.references.clone()));
    }
    native_field(model_input, "references")?
        .try_iter()?
        .map(|reference| reference_row_of(&reference?))
        .collect()
}

/// One `CompileSqlReference` record as its row.
fn reference_row_of(reference: &Bound<'_, PyAny>) -> PyResult<ReferenceRow> {
    Ok(ReferenceRow {
        kind: native_field(reference, "ref_kind")?.str()?.extract()?,
        name: native_field(reference, "ref_name")?.extract()?,
        package: native_field(reference, "ref_package")?.extract()?,
        call_argument_count: native_field(reference, "call_argument_count")?.extract()?,
    })
}
