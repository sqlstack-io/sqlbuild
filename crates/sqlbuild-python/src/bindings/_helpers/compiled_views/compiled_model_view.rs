//! A compiled model, read from the native compiled project that owns it.

use crate::bindings::_helpers::model_config::header_audits::HeaderAudits;
use sqlbuild_project::models as project_models;
use sqlbuild_project::models::{DestinationFact, ModelMetadataTexts};
use sqlbuild_project::types::InferredRow;
use std::sync::Arc;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use pyo3::{FromPyObject, pyclass, pymethods};

use sqlbuild_analysis::assembly::analysis_session::main::contract_proof_json::contract_proof_json;
use sqlbuild_analysis::assembly::analysis_session::models::LineageRow;
use sqlbuild_analysis::compiled_project::main::model_facts::model_facts;
use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, DynamicProofFact, ModelAnalysisFact,
};

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::rows::{ReferenceTuple, reference_row_tuple};
use crate::bindings::_helpers::compiled_views::compiled_model_analysis::{
    AnalysisRecord, AnalysisViews, ProofArgument, SessionArgument, analysis_record,
};
use crate::bindings::_helpers::compiled_views::model_input_view::{
    InputViewClasses, ModelInputRecord, model_relation_key_row, query_sql_text, read_input,
    read_input_with_owner, references, rejected_opt_out, schema_entry, view_classes,
};
use crate::bindings::_helpers::compiled_views::project_host::PyProjectHost;
use crate::bindings::_helpers::compiled_views::relation_keys::RelationKeyRow;
use crate::bindings::_helpers::discovery::model_file_record::NativeModelFile;
use crate::bindings::_helpers::graph::project_graph::ResourceRow;
use crate::bindings::_helpers::model_config::header_metadata::HeaderColumns;
use crate::bindings::_helpers::records::fields::field as native_field;

const DATABASE_KEY: &str = "database";
const SCHEMA_KEY: &str = "schema";
const ALIAS_KEY: &str = "alias";
const VALUES_ATTRIBUTE: &str = "values";
const LOGICAL_SCHEMA_ATTRIBUTE: &str = "logical_schema";
const LOGICAL_DATABASE_ATTRIBUTE: &str = "logical_database";
const MODEL_RESOURCE: &str = "model";
const SQL_ANALYSIS_KEY: &str = "sql_analysis";
const TAGS_KEY: &str = "tags";
const NULLABLE: &str = "nullable";

/// A model's SQL rule inputs; `excluded` is `None` when its config values could not be read natively.
pub(crate) struct SqlRuleInputs {
    pub(crate) posix_path: String,
    pub(crate) excluded: Option<bool>,
    pub(crate) key: Option<SqlRuleKey>,
}

/// What a model's SQL rule cache key reads.
pub(crate) struct SqlRuleKey {
    pub(crate) authored_sql: String,
    pub(crate) query_sql: String,
    /// The canonical JSON text of the model's dynamic pivot proof.
    pub(crate) contract: Option<String>,
}

/// A schema's entry audits, column specs and each audited column's index and name.
pub(crate) type AuditedSchema = (Arc<HeaderAudits>, Arc<HeaderColumns>, Vec<(usize, String)>);

/// `(name, type)` of a declared column.
pub(crate) type DeclaredRow = (String, Option<String>);

/// `(database, schema, name)` of a relation.
pub(crate) type DestinationRow = (Option<String>, Option<String>, String);

/// `(output column, [(resource name, column name)])` of one lineage output.
pub(crate) type LineageInput = (String, Vec<(String, String)>);

/// The Python classes a compiled model's projected attributes are built with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct CompiledViewClasses<'py> {
    object_key: Bound<'py, PyAny>,
    relation_location: Bound<'py, PyAny>,
    pub(crate) inferred_column: Bound<'py, PyAny>,
    pub(crate) nullability: Bound<'py, PyAny>,
    pub(crate) compiler_diagnostic: Bound<'py, PyAny>,
    pub(crate) source_location: Bound<'py, PyAny>,
    pub(crate) compact_lineage_facts: Bound<'py, PyAny>,
    pub(crate) lineage_column_fact: Bound<'py, PyAny>,
    pub(crate) lineage_source_fact: Bound<'py, PyAny>,
    pub(crate) contract_proof: Bound<'py, PyAny>,
    pub(crate) family_proof: Bound<'py, PyAny>,
}

/// What assembly and the semantic checks finished for one compiled model.
pub(crate) type CompiledRecord = project_models::CompiledRecord<PyProjectHost>;

/// A compiled model's Python attribute projections, each built once on first access.
pub(crate) struct CompiledProjections {
    key: PyOnceLock<Py<PyAny>>,
    deps: PyOnceLock<Py<PyTuple>>,
}

impl Default for CompiledProjections {
    fn default() -> Self {
        Self {
            key: PyOnceLock::new(),
            deps: PyOnceLock::new(),
        }
    }
}

/// What later stages changed for one view of a model; the owner's record stays as assembled.
#[derive(Default)]
struct ModelOverlay {
    binding_diagnostics: Option<Py<PyTuple>>,
    /// Output columns whose types semantic recovery forgot, left unchecked by the contract.
    poisoned: Vec<String>,
    destination: Option<DestinationFact>,
}

impl ModelOverlay {
    fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            binding_diagnostics: self
                .binding_diagnostics
                .as_ref()
                .map(|diagnostics| diagnostics.clone_ref(py)),
            poisoned: self.poisoned.clone(),
            destination: self.destination.clone(),
        }
    }
}

/// One view's projections of attributes its overlay can change, each built once.
struct ViewProjections {
    destination: PyOnceLock<Py<PyAny>>,
    inferred_columns: PyOnceLock<Py<PyAny>>,
}

impl Default for ViewProjections {
    fn default() -> Self {
        Self {
            destination: PyOnceLock::new(),
            inferred_columns: PyOnceLock::new(),
        }
    }
}

/// Deps, session analysis, failed analysis's columns, standalone proof, and macro deps.
#[derive(FromPyObject)]
pub(crate) struct CompiledArguments(
    Vec<(String, String)>,
    Option<SessionArgument>,
    Option<Vec<InferredRow>>,
    Option<ProofArgument>,
    Vec<String>,
);

/// The compiled record of one model, with its configured destination.
pub(crate) fn compiled_record(
    py: Python<'_>,
    input: &ModelInputRecord,
    name: &str,
    arguments: CompiledArguments,
) -> PyResult<CompiledRecord> {
    let CompiledArguments(deps, session, fallback, standalone_proof, macro_deps) = arguments;
    let analysis: AnalysisRecord =
        analysis_record(py, input, session, (fallback, standalone_proof))?;
    let config: &Bound<'_, PyAny> = input.host.config.bind(py);
    let values: Bound<'_, PyDict> = native_field(config, VALUES_ATTRIBUTE)?.downcast_into()?;
    Ok(CompiledRecord {
        deps,
        destination: DestinationFact {
            database: text_value(&values, DATABASE_KEY)?,
            schema: text_value(&values, SCHEMA_KEY)?,
            name: text_value(&values, ALIAS_KEY)?.unwrap_or_else(|| name.to_owned()),
            qualified_name: None,
            logical_schema: native_field(config, LOGICAL_SCHEMA_ATTRIBUTE)?.extract()?,
            logical_database: native_field(config, LOGICAL_DATABASE_ATTRIBUTE)?.extract()?,
        },
        analysis,
        macro_deps,
        host: CompiledProjections::default(),
    })
}

fn text_value(values: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match values.get_item(key)? {
        Some(value) if value.is_instance_of::<pyo3::types::PyString>() => {
            Ok(Some(value.extract()?))
        }
        _ => Ok(None),
    }
}

/// The destination with adapter defaults filled in; `render` renders its qualified name.
pub(crate) fn defaulted_destination(
    current: &DestinationFact,
    defaults: (Option<String>, Option<String>),
    render: &Bound<'_, PyAny>,
) -> PyResult<DestinationFact> {
    let (default_schema, default_database) = defaults;
    let schema: Option<String> = current.schema.clone().or(default_schema);
    let database: Option<String> = current.database.clone().or(default_database);
    let keywords: Bound<'_, PyDict> = PyDict::new(render.py());
    keywords.set_item(DATABASE_KEY, &database)?;
    keywords.set_item(SCHEMA_KEY, &schema)?;
    keywords.set_item("name", &current.name)?;
    let qualified_name: Option<String> = render.call((), Some(&keywords))?.extract()?;
    Ok(DestinationFact {
        database,
        schema,
        name: current.name.clone(),
        qualified_name,
        logical_schema: current.logical_schema.clone(),
        logical_database: current.logical_database.clone(),
    })
}

/// One compiled model, projected from the owner on demand.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeCompiledModel {
    owner: Py<NativeCompiledProject>,
    index: usize,
    overlay: ModelOverlay,
    projections: ViewProjections,
}

impl NativeCompiledModel {
    pub(crate) fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self::with_overlay(owner, index, ModelOverlay::default())
    }

    fn with_overlay(owner: Py<NativeCompiledProject>, index: usize, overlay: ModelOverlay) -> Self {
        Self {
            owner,
            index,
            overlay,
            projections: ViewProjections::default(),
        }
    }

    fn changed(&self, py: Python<'_>, change: impl FnOnce(&mut ModelOverlay)) -> Self {
        let mut overlay: ModelOverlay = self.overlay.clone_ref(py);
        change(&mut overlay);
        Self::with_overlay(self.owner.clone_ref(py), self.index, overlay)
    }

    pub(crate) fn read<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&ModelInputRecord, Python<'py>) -> PyResult<T>,
    ) -> PyResult<T> {
        read_input(&self.owner, self.index, py, read)
    }

    fn compiled<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&CompiledRecord, Python<'py>) -> PyResult<T>,
    ) -> PyResult<T> {
        self.read(py, |record, py| read(compiled_of(record)?, py))
    }

    /// Read the effective destination, including namespace overlays, without a Python projection.
    pub(crate) fn read_destination<T>(
        &self,
        py: Python<'_>,
        read: impl FnOnce(&DestinationFact) -> PyResult<T>,
    ) -> PyResult<T> {
        self.compiled(py, |compiled, _| {
            read(
                self.overlay
                    .destination
                    .as_ref()
                    .unwrap_or(&compiled.destination),
            )
        })
    }

    /// This view's typed analysis as the owner's model facts read it, and the model's name.
    pub(crate) fn analysis_fact(&self, py: Python<'_>) -> PyResult<(String, ModelAnalysisFact)> {
        self.read(py, |record, _| {
            let analysis: &AnalysisRecord = &compiled_of(record)?.analysis;
            let rows: &[InferredRow] = analysis.inferred.as_deref().unwrap_or_default();
            let mut inferred_columns: Vec<(String, Option<String>)> =
                Vec::with_capacity(rows.len());
            for (name, column_type, _) in rows {
                let poisoned: bool = self.overlay.poisoned.contains(name);
                inferred_columns.push((name.clone(), column_type.clone().filter(|_| !poisoned)));
            }
            let dynamic_proof: Option<DynamicProofFact> =
                analysis.proof.as_ref().map(|proof| DynamicProofFact {
                    output_proven: proof.output_proven,
                    bare_dynamic_pivot: proof.bare_dynamic_pivot,
                    families: casefolded_families(&proof.families),
                    failure_reason: proof.failure_reason.clone(),
                    named_families: proof.families.clone(),
                });
            let fact: ModelAnalysisFact = ModelAnalysisFact {
                inferred_columns,
                dynamic_proof,
                inferred: analysis.inferred.is_some(),
                nullable_columns: rows
                    .iter()
                    .map(|(_, _, nullability)| nullability == NULLABLE)
                    .collect(),
                fast_lineage_has_star: analysis.has_star,
                unchecked_output_columns: self.overlay.poisoned.clone(),
            };
            Ok((model_name(record), fact))
        })
    }

    /// Each output column of the kept lineage with its `(resource name, column)` sources.
    pub(crate) fn lineage_inputs(&self, py: Python<'_>) -> PyResult<Option<Vec<LineageInput>>> {
        self.compiled(py, |compiled, _| {
            Ok(compiled
                .analysis
                .lineage_rows()
                .map(|rows| rows.iter().map(lineage_input).collect()))
        })
    }

    /// Whether the model's dynamic pivot proof proved its output columns.
    pub(crate) fn output_proven(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| {
            Ok(compiled
                .analysis
                .proof
                .as_ref()
                .is_some_and(|proof| proof.output_proven))
        })
    }

    /// Output columns whose types semantic recovery forgot.
    pub(crate) fn unchecked_columns(&self) -> &[String] {
        &self.overlay.poisoned
    }

    /// The kept lineage rows with their transform and confidence codes, or `None`.
    pub(crate) fn lineage_rows(&self, py: Python<'_>) -> PyResult<Option<Vec<LineageRow>>> {
        self.compiled(py, |compiled, _| {
            Ok(compiled.analysis.lineage_rows().map(<[LineageRow]>::to_vec))
        })
    }

    /// The input relations of a dynamic pivot proof that proved its outputs, else `None`.
    pub(crate) fn proven_inputs(&self, py: Python<'_>) -> PyResult<Option<Vec<String>>> {
        self.compiled(py, |compiled, _| {
            Ok(compiled
                .analysis
                .proof
                .as_ref()
                .filter(|proof| proof.output_proven)
                .map(|proof| proof.input_relations.clone()))
        })
    }

    /// The attached schema's declared `SchemaColumn`s, or `None` without a schema.
    pub(crate) fn declared_columns(&self, py: Python<'_>) -> PyResult<Option<Py<PyTuple>>> {
        self.read(py, |record, py| {
            record
                .schema
                .as_ref()
                .map(|schema| schema.host.columns.project(py).map(Bound::unbind))
                .transpose()
        })
    }

    /// The attached schema's entry audits, its column specs and each audited column's index and name.
    pub(crate) fn schema_audits_and_columns(
        &self,
        py: Python<'_>,
    ) -> PyResult<Option<AuditedSchema>> {
        read_input_with_owner(&self.owner, self.index, py, |owner, record| {
            let (Some(schema), Some(fact)) = (&record.schema, owner.input_schema_fact(record))
            else {
                return Ok(None);
            };
            let audited: Vec<(usize, String)> = fact
                .columns
                .iter()
                .enumerate()
                .filter(|(_, column)| !column.audit_definitions.is_empty())
                .map(|(index, column)| (index, column.name.clone()))
                .collect();
            Ok(Some((
                schema.host.audits.clone(),
                schema.host.columns.clone(),
                audited,
            )))
        })
    }

    /// What SQL rules select the model by, plus its cache key inputs when `keyed`.
    pub(crate) fn sql_rule_inputs(&self, py: Python<'_>, keyed: bool) -> PyResult<SqlRuleInputs> {
        read_input_with_owner(&self.owner, self.index, py, |owner, record| {
            let facts: &CompiledModelFacts =
                model_facts(owner.facts(), &record.name).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err("the model was not assembled")
                })?;
            let excluded: Option<bool> = match &facts.config.encoding_error {
                Some(_) => None,
                None => Some(
                    facts.config.values.get(SQL_ANALYSIS_KEY)
                        == Some(&serde_json::Value::Bool(false)),
                ),
            };
            let key = keyed
                .then(|| -> PyResult<SqlRuleKey> {
                    Ok(SqlRuleKey {
                        authored_sql: facts.authored_sql.to_string(),
                        query_sql: facts.query_sql.to_string(),
                        contract: compiled_of(record)?
                            .analysis
                            .proof
                            .as_ref()
                            .map(contract_proof_json),
                    })
                })
                .transpose()?;
            Ok(SqlRuleInputs {
                posix_path: facts.relative_path.clone(),
                excluded,
                key,
            })
        })
    }

    /// The model's name, `str(relative_path)` and authored file text.
    pub(crate) fn file_texts(&self, py: Python<'_>) -> PyResult<(String, String, Arc<str>)> {
        self.read(py, |record, py| {
            let model_file: &NativeModelFile = record.host.model_file.get();
            Ok((
                model_name(record),
                model_file.relative_text(py)?,
                Arc::clone(model_file.contents_text()),
            ))
        })
    }

    /// The name, compiled query and authored file of the model, and whether it calls a function.
    pub(crate) fn metadata_texts(
        &self,
        py: Python<'_>,
        function_kinds: &[&str],
    ) -> PyResult<ModelMetadataTexts> {
        self.read(py, |record, _| {
            Ok(ModelMetadataTexts {
                name: model_name(record),
                query_sql: record.query_sql.to_string(),
                authored_sql: record.host.model_file.get().contents_text().to_string(),
                calls_functions: record
                    .references
                    .iter()
                    .any(|reference| function_kinds.contains(&reference.kind.as_str())),
            })
        })
    }

    /// `relative_path.as_posix()` and the compiled query, which compiled artifacts are written from.
    pub(crate) fn output_texts(&self, py: Python<'_>) -> PyResult<(String, Arc<str>)> {
        self.read(py, |record, py| {
            Ok((
                record.host.model_file.get().posix_path(py)?,
                Arc::clone(&record.query_sql),
            ))
        })
    }

    /// The model's authored file text and expanded query, from its recorded facts.
    pub(crate) fn sql_texts(&self, py: Python<'_>) -> PyResult<(String, String)> {
        let name: String = self.name(py)?;
        let owner = self.owner.bind(py).borrow();
        let facts: &CompiledModelFacts = model_facts(owner.facts(), &name).ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("the model was not assembled")
        })?;
        Ok((facts.authored_sql.to_string(), facts.query_sql.to_string()))
    }

    fn model_file<T>(
        &self,
        py: Python<'_>,
        read: impl FnOnce(&NativeModelFile) -> PyResult<T>,
    ) -> PyResult<T> {
        self.read(py, |record, _| read(record.host.model_file.get()))
    }
}

/// A lineage row's output column with its `(resource name, column)` sources.
fn lineage_input(row: &LineageRow) -> LineageInput {
    let mut sources: Vec<(String, String)> = Vec::with_capacity(row.sources.len());
    for (_, resource, column) in &row.sources {
        sources.push((resource.clone(), column.clone()));
    }
    (row.output_column.clone(), sources)
}

/// Each family's name casefolded as Python's, with its inferred type.
fn casefolded_families(families: &[(String, Option<String>)]) -> Vec<(String, Option<String>)> {
    families
        .iter()
        .map(|(name, kind)| (str::to_lowercase(name), kind.clone()))
        .collect()
}

/// The compiled record of an input, which assembly records before any view is handed out.
fn compiled_of(record: &ModelInputRecord) -> PyResult<&CompiledRecord> {
    record
        .compiled
        .as_ref()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("the model was not assembled"))
}

#[pymethods]
impl NativeCompiledModel {
    #[getter]
    pub(crate) fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| {
            let compiled: &CompiledRecord = compiled_of(record)?;
            compiled
                .host
                .key
                .get_or_try_init(py, || {
                    let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                    object_key(&classes.compiled, MODEL_RESOURCE, &model_name(record))
                })
                .map(|key| key.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn deps(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.compiled(py, |compiled, py| {
            compiled
                .host
                .deps
                .get_or_try_init(py, || {
                    let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                    let mut keys: Vec<Py<PyAny>> = Vec::with_capacity(compiled.deps.len());
                    for (kind, name) in &compiled.deps {
                        keys.push(object_key(&classes.compiled, kind, name)?);
                    }
                    Ok::<_, pyo3::PyErr>(PyTuple::new(py, keys)?.unbind())
                })
                .map(|deps| deps.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn name(&self, py: Python<'_>) -> PyResult<String> {
        self.read(py, |record, _| Ok(model_name(record)))
    }

    #[getter]
    pub(crate) fn relative_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.model_file(py, |file| file.relative_path(py))
    }

    #[getter]
    pub(crate) fn query_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.read(py, |record, py| Ok(query_sql_text(py, record)))
    }

    #[getter]
    pub(crate) fn config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(record.host.config.clone_ref(py)))
    }

    #[getter]
    pub(crate) fn destination(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read_destination(py, |destination| {
            self.projections
                .destination
                .get_or_try_init(py, || {
                    let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                    relation_location(&classes.compiled, destination)
                })
                .map(|destination| destination.clone_ref(py))
        })
    }

    #[getter]
    pub(crate) fn references(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| references(&classes, record))
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
        read_input_with_owner(&self.owner, self.index, py, |owner, record| {
            model_relation_key_row(owner, record, py)
        })
    }

    /// `(resource type, name)` of the model's key.
    #[getter]
    pub(crate) fn key_row(&self, py: Python<'_>) -> PyResult<(String, String)> {
        self.read(py, |record, _| {
            Ok((MODEL_RESOURCE.to_owned(), model_name(record)))
        })
    }

    /// `(resource type, name)` of each dependency.
    #[getter]
    pub(crate) fn dep_rows(&self, py: Python<'_>) -> PyResult<Vec<(String, String)>> {
        self.compiled(py, |compiled, _| Ok(compiled.deps.clone()))
    }

    /// `(database, schema, name)` of the model's destination.
    #[getter]
    pub(crate) fn destination_row(&self, py: Python<'_>) -> PyResult<DestinationRow> {
        self.read_destination(py, |destination| {
            Ok((
                destination.database.clone(),
                destination.schema.clone(),
                destination.name.clone(),
            ))
        })
    }

    /// Each inferred column as `(name, type, nullability)`, or `None` when none were inferred.
    #[getter]
    pub(crate) fn inferred_rows(&self, py: Python<'_>) -> PyResult<Option<Vec<InferredRow>>> {
        self.compiled(py, |compiled, _| {
            let Some(rows) = &compiled.analysis.inferred else {
                return Ok(None);
            };
            let mut shown: Vec<InferredRow> = Vec::with_capacity(rows.len());
            for (name, column_type, nullability) in rows {
                let poisoned: bool = self.overlay.poisoned.contains(name);
                let column_type: Option<String> = if poisoned { None } else { column_type.clone() };
                shown.push((name.clone(), column_type, nullability.clone()));
            }
            Ok(Some(shown))
        })
    }

    /// Each declared column of the attached schema as `(name, type)`, or `None` without one.
    #[getter]
    pub(crate) fn schema_column_rows(&self, py: Python<'_>) -> PyResult<Option<Vec<DeclaredRow>>> {
        let name: String = self.name(py)?;
        let owner = self.owner.bind(py).borrow();
        let facts: &CompiledModelFacts = model_facts(owner.facts(), &name).ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("the model was not assembled")
        })?;
        let Some(schema) = &facts.schema else {
            return Ok(None);
        };
        Ok(Some(
            schema
                .columns
                .iter()
                .map(|column| (column.name.clone(), column.data_type.clone()))
                .collect(),
        ))
    }

    /// Whether the attached schema declares dynamic columns.
    #[getter(has_dynamic_columns)]
    pub(crate) fn dynamic_columns_declared(&self, py: Python<'_>) -> PyResult<bool> {
        self.read(py, |record, _py| {
            Ok(record
                .schema
                .as_ref()
                .is_some_and(|schema| !schema.host.dynamic_columns.specs.is_empty()))
        })
    }

    /// Inferred columns, else the attached schema's declared columns, else zero.
    #[getter]
    pub(crate) fn output_column_count(&self, py: Python<'_>) -> PyResult<usize> {
        self.read(py, |record, _py| {
            if let Some(rows) = &compiled_of(record)?.analysis.inferred {
                return Ok(rows.len());
            }
            Ok(record
                .schema
                .as_ref()
                .map_or(0, |schema| schema.host.columns.specs.len()))
        })
    }

    #[getter]
    pub(crate) fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| schema_entry(&classes, record))
    }

    #[getter]
    pub(crate) fn inferred_columns(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.compiled(py, |compiled, py| {
            self.projections
                .inferred_columns
                .get_or_try_init(py, || {
                    let Some(rows) = &compiled.analysis.inferred else {
                        return Ok::<_, pyo3::PyErr>(py.None());
                    };
                    let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                    let mut columns: Vec<Bound<'_, PyAny>> = Vec::with_capacity(rows.len());
                    for row in rows {
                        let poisoned_row: InferredRow;
                        let row = if self.overlay.poisoned.contains(&row.0) {
                            poisoned_row = (row.0.clone(), None, row.2.clone());
                            &poisoned_row
                        } else {
                            row
                        };
                        columns.push(self.owner.bind(py).borrow().inferred_column(
                            py,
                            &classes.compiled,
                            row,
                        )?);
                    }
                    Ok(PyTuple::new(py, columns)?.into_any().unbind())
                })
                .map(|columns| columns.clone_ref(py))
        })
    }

    /// The upstream source count of the model's lineage facts, `None` without them.
    pub(crate) fn lineage_edge_count(&self, py: Python<'_>) -> PyResult<Option<usize>> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.lineage_edge_count()))
    }

    #[getter]
    pub(crate) fn fast_lineage_columns(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.compiled(py, |compiled, py| {
            compiled.analysis.lineage_object(py, &classes.compiled)
        })
    }

    #[getter]
    pub(crate) fn fast_lineage_has_star(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.has_star))
    }

    #[getter]
    pub(crate) fn fast_lineage_star_resolved(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.star_resolved))
    }

    #[getter]
    pub(crate) fn authored_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.model_file(py, |file| Ok(file.contents(py)))
    }

    #[getter]
    pub(crate) fn authored_query_sql<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyString>> {
        self.model_file(py, |file| Ok(file.query_sql(py)))
    }

    #[getter]
    pub(crate) fn output_column_locations<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyDict>> {
        self.model_file(py, |file| file.output_column_locations(py))
    }

    #[getter]
    pub(crate) fn extract_implicit_alias_columns(&self, py: Python<'_>) -> PyResult<bool> {
        self.model_file(py, |file| Ok(file.extract_implicit_alias_columns()))
    }

    #[getter]
    pub(crate) fn macro_deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.compiled(py, |compiled, py| PyTuple::new(py, &compiled.macro_deps))
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
    pub(crate) fn binding_diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        if let Some(diagnostics) = &self.overlay.binding_diagnostics {
            return Ok(diagnostics.clone_ref(py));
        }
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, py| {
            let name: String = model_name(record);
            let path: Bound<'_, PyAny> = record.host.model_file.get().path_objects(py)?.1;
            compiled_of(record)?
                .analysis
                .binding_tuple(py, &classes.compiled, (&name, &path))
        })
    }

    #[getter]
    pub(crate) fn binding_validated(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.binding_validated))
    }

    #[getter]
    pub(crate) fn dynamic_column_contract(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.compiled(py, |compiled, py| {
            compiled.analysis.proof_object(py, &classes.compiled)
        })
    }

    #[getter]
    pub(crate) fn unchecked_output_columns<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyFrozenSet>> {
        PyFrozenSet::new(py, &self.overlay.poisoned)
    }

    #[getter]
    pub(crate) fn rejected_sql_analysis_opt_out(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(rejected_opt_out(record, py)))
    }

    /// This model with its final binding diagnostics.
    fn with_binding_diagnostics(&self, py: Python<'_>, diagnostics: Py<PyTuple>) -> Self {
        self.changed(py, |overlay| {
            overlay.binding_diagnostics = Some(diagnostics)
        })
    }

    /// This model after semantic type recovery: its bindings and the outputs left unchecked.
    fn with_type_recovery(
        &self,
        py: Python<'_>,
        diagnostics: Py<PyTuple>,
        poisoned: Vec<String>,
    ) -> Self {
        self.changed(py, |overlay| {
            overlay.binding_diagnostics = Some(diagnostics);
            overlay.poisoned = poisoned;
        })
    }

    /// This model with the adapter's default schema and database filled into its destination.
    pub(crate) fn with_target_defaults(
        &self,
        py: Python<'_>,
        defaults: (Option<String>, Option<String>),
        render: Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let destination: DestinationFact = self.compiled(py, |compiled, _| {
            let current: &DestinationFact = self
                .overlay
                .destination
                .as_ref()
                .unwrap_or(&compiled.destination);
            defaulted_destination(current, defaults, &render)
        })?;
        Ok(self.changed(py, |overlay| overlay.destination = Some(destination)))
    }
}

/// The model's project graph row: its key, dependencies, tags and folder.
pub(crate) fn model_graph_row(
    model: &NativeCompiledModel,
    py: Python<'_>,
) -> PyResult<ResourceRow> {
    model.read(py, |record, py| {
        let compiled: &CompiledRecord = compiled_of(record)?;
        let values: Bound<'_, PyAny> = native_field(record.host.config.bind(py), VALUES_ATTRIBUTE)?;
        let mut tags: Vec<String> = Vec::new();
        if let Some(value) = values.downcast::<PyDict>()?.get_item(TAGS_KEY)?
            && (value.is_instance_of::<pyo3::types::PyList>()
                || value.is_instance_of::<pyo3::types::PyTuple>())
        {
            for tag in value.try_iter()? {
                tags.push(tag?.str()?.extract()?);
            }
        }
        let folder: String = record.host.model_file.get().folder(py)?;
        Ok(ResourceRow {
            key: (MODEL_RESOURCE.to_owned(), model_name(record)),
            deps: compiled.deps.clone(),
            tags,
            folder: Some(folder),
        })
    })
}

fn model_name(record: &ModelInputRecord) -> String {
    record.name.clone()
}

/// The `CompiledRelationLocation` of a destination.
pub(crate) fn relation_location(
    classes: &CompiledViewClasses<'_>,
    destination: &DestinationFact,
) -> PyResult<Py<PyAny>> {
    let fields: Bound<'_, PyDict> = PyDict::new(classes.relation_location.py());
    fields.set_item(DATABASE_KEY, &destination.database)?;
    fields.set_item(SCHEMA_KEY, &destination.schema)?;
    fields.set_item("name", &destination.name)?;
    fields.set_item("qualified_name", &destination.qualified_name)?;
    fields.set_item(LOGICAL_SCHEMA_ATTRIBUTE, &destination.logical_schema)?;
    fields.set_item(LOGICAL_DATABASE_ATTRIBUTE, &destination.logical_database)?;
    Ok(classes.relation_location.call((), Some(&fields))?.unbind())
}

pub(crate) fn object_key(
    classes: &CompiledViewClasses<'_>,
    kind: &str,
    name: &str,
) -> PyResult<Py<PyAny>> {
    let fields: Bound<'_, PyDict> = PyDict::new(classes.object_key.py());
    fields.set_item("resource_type", kind)?;
    fields.set_item("name", name)?;
    Ok(classes.object_key.call((), Some(&fields))?.unbind())
}
