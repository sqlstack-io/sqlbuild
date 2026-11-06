//! A model's compile input, read from the native compiled project that owns it.

use sqlbuild_project::models as project_models;
use sqlbuild_project::models::ExpansionRecord;
use std::sync::Arc;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{FromPyObject, IntoPyObject, pyclass, pymethods};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_analysis::sql_macros::main::typed_analysis_sql::typed_analysis_sql;
use sqlbuild_analysis::sql_macros::models::SqlMacroCheck;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::rows::{ReferenceTuple, reference_row_tuple};
use crate::bindings::_helpers::compiled_project::schema_facts::python_column_facts;
use crate::bindings::_helpers::compiled_views::compiled_model_view::{
    CompiledArguments, CompiledViewClasses, NativeCompiledModel, compiled_record,
};
use crate::bindings::_helpers::compiled_views::project_host::PyProjectHost;
use crate::bindings::_helpers::compiled_views::relation_keys::{RelationKeyRow, relation_key_row};
use crate::bindings::_helpers::discovery::model_file_record::{NativeModelFile, projected_text};
use crate::bindings::_helpers::model_config::header_audits::HeaderAudits;
use crate::bindings::_helpers::model_config::header_families::{HeaderFamilies, family_specs};
use crate::bindings::_helpers::model_config::header_metadata::HeaderColumns;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::render::macro_call_rows::{SpanRow, span_tuple};
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_project::models::ReferenceRow;

const MODEL_RELATION_KIND: &str = "ref";

/// The Python classes a model input's projected attributes are built with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct InputViewClasses<'py> {
    pub(crate) compiled: CompiledViewClasses<'py>,
    sql_reference: Bound<'py, PyAny>,
    reference_kind: Bound<'py, PyAny>,
    schema_model_entry: Bound<'py, PyAny>,
    sql_expansion: Bound<'py, PyAny>,
    expansion_span: Bound<'py, PyAny>,
}

/// A model's attached header schema; the host keeps its columns, audits and dynamic columns.
pub(crate) type SchemaRecord = project_models::SchemaRecord<PyProjectHost>;

/// What the model loop finished for one model beyond its facts.
pub(crate) type ModelInputRecord = project_models::ModelInputRecord<PyProjectHost>;

/// A model schema's header columns and specs, which carry user-authored audit arguments.
pub(crate) struct SchemaHandles {
    pub(crate) columns: Arc<HeaderColumns>,
    pub(crate) audits: Arc<HeaderAudits>,
    pub(crate) dynamic_columns: Arc<HeaderFamilies>,
}

/// A model input's user values and the attribute projections its views build.
pub(crate) struct ModelInputHandles {
    /// The discovered model file, owned by the discovery inputs.
    pub(crate) model_file: Py<NativeModelFile>,
    /// The resolved config, whose values hold user values.
    pub(crate) config: Py<PyAny>,
    pub(crate) rejected_opt_out: Option<Py<PyAny>>,
    /// The model's own enums and constants, whose values are user values.
    pub(crate) enum_declarations: Py<PyTuple>,
    pub(crate) constant_declarations: Py<PyTuple>,
    pub(crate) enum_columns: Py<PyDict>,
    /// Usage records, the identities the scope index is built from.
    pub(crate) macro_usages: Py<PyTuple>,
    pub(crate) declaration_usages: Py<PyTuple>,
    /// Attribute projections built on first access, shared by every view of the model.
    pub(crate) projections: InputProjections,
}

/// A model input's Python attribute projections, each built once on first access.
pub(crate) struct InputProjections {
    query_sql: PyOnceLock<Py<PyString>>,
    references: PyOnceLock<Py<PyTuple>>,
    schema_entry: PyOnceLock<Py<PyAny>>,
    sql_expansion: PyOnceLock<Py<PyAny>>,
    lint_expansion: PyOnceLock<Py<PyAny>>,
}

impl Default for InputProjections {
    fn default() -> Self {
        Self {
            query_sql: PyOnceLock::new(),
            references: PyOnceLock::new(),
            schema_entry: PyOnceLock::new(),
            sql_expansion: PyOnceLock::new(),
            lint_expansion: PyOnceLock::new(),
        }
    }
}

/// One model's compile input, projected from the owner on demand.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeModelInput {
    owner: Py<NativeCompiledProject>,
    index: usize,
}

impl NativeModelInput {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }

    pub(crate) fn read<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&ModelInputRecord, Python<'py>) -> PyResult<T>,
    ) -> PyResult<T> {
        read_input(&self.owner, self.index, py, read)
    }

    /// Read the input record together with the project that owns it.
    pub(crate) fn read_with_owner<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&NativeCompiledProject, &ModelInputRecord) -> PyResult<T>,
    ) -> PyResult<T> {
        read_input_with_owner(&self.owner, self.index, py, read)
    }
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

#[pymethods]
impl NativeModelInput {
    #[getter]
    pub(crate) fn model_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| {
            Ok(record.host.model_file.clone_ref(py).into_any())
        })
    }

    #[getter]
    pub(crate) fn config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(record.host.config.clone_ref(py)))
    }

    #[getter]
    pub(crate) fn query_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.read(py, |record, py| Ok(query_sql_text(py, record)))
    }

    #[getter]
    pub(crate) fn macro_source_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.read(py, |record, py| {
            Ok(PyString::new(py, &record.macro_source_sql))
        })
    }

    #[getter]
    pub(crate) fn references(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| references(&classes, record))
    }

    #[getter]
    pub(crate) fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| schema_entry(&classes, record))
    }

    /// Each reference as `(kind, name, package, call argument count)`.
    #[getter]
    pub(crate) fn reference_rows(&self, py: Python<'_>) -> PyResult<Vec<ReferenceTuple>> {
        self.read(py, |record, _| {
            Ok(record
                .references
                .iter()
                .cloned()
                .map(reference_row_tuple)
                .collect())
        })
    }

    /// The model's declared key facts, for deriving its unique keys.
    #[getter]
    pub(crate) fn relation_key_row(&self, py: Python<'_>) -> PyResult<RelationKeyRow> {
        self.read_with_owner(py, |owner, record| {
            model_relation_key_row(owner, record, py)
        })
    }

    /// The name of the schema entry attached to the model, or `None`.
    #[getter]
    pub(crate) fn attached_schema_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.read(py, |record, _| Ok(attached_schema_name(record)))
    }

    #[getter]
    pub(crate) fn schema_file(&self, py: Python<'_>) -> Py<PyAny> {
        py.None()
    }

    #[getter]
    pub(crate) fn sql_validation_enabled(&self, py: Python<'_>) -> PyResult<bool> {
        self.read(py, |record, _| Ok(record.sql_validation_enabled))
    }

    #[getter]
    pub(crate) fn rejected_sql_analysis_opt_out(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(rejected_opt_out(record, py)))
    }

    #[getter]
    pub(crate) fn enum_declarations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.enum_declarations.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn constant_declarations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.constant_declarations.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn enum_columns(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.read(py, |record, py| Ok(record.host.enum_columns.clone_ref(py)))
    }

    #[getter]
    pub(crate) fn macro_deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.read(py, |record, py| PyTuple::new(py, &record.macro_deps))
    }

    #[getter]
    pub(crate) fn macro_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| Ok(record.host.macro_usages.clone_ref(py)))
    }

    #[getter]
    pub(crate) fn declaration_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.declaration_usages.clone_ref(py))
        })
    }

    /// Record what assembly finished for the model; return its compiled model view.
    fn compiled_model(
        &self,
        py: Python<'_>,
        arguments: CompiledArguments,
    ) -> PyResult<NativeCompiledModel> {
        let mut owner = self.owner.bind(py).borrow_mut();
        let record: &mut ModelInputRecord = owner.input_mut(self.index)?;
        let name: String = record.name.clone();
        record.compiled = Some(compiled_record(py, record, &name, arguments)?);
        Ok(NativeCompiledModel::new(
            self.owner.clone_ref(py),
            self.index,
        ))
    }

    /// The authored-to-expanded passes, when interpolation left the authored query unchanged.
    #[getter]
    pub(crate) fn sql_expansion(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| authored_expansion(&classes, record))
    }

    /// The whole expansion, variables included, which early SQL lint reuses.
    #[getter]
    pub(crate) fn lint_expansion(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, py| {
            record
                .host
                .projections
                .lint_expansion
                .get_or_try_init(py, || {
                    let expansion: &ExpansionRecord = &record.expansion;
                    expansion_object(
                        &classes,
                        expansion,
                        &[
                            &expansion.interpolation_spans,
                            &expansion.declaration_spans,
                            &expansion.macro_spans,
                        ],
                    )
                })
                .map(|expansion| expansion.clone_ref(py))
        })
    }
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
fn expansion_object(
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

/// A model input's attached schema: its name and dynamic column families.
pub(crate) struct AttachedSchema {
    pub(crate) name: String,
    pub(crate) dynamic_columns: Vec<DynamicFamily>,
    pub(crate) type_enforcement: bool,
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
