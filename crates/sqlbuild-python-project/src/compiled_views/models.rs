//! Public records of compiled views.

use crate::compiled_views::_helpers::project::loads::load_records;
use crate::graph::main::project::load_rows::load_rows;
use sqlbuild_rules::engine::main::model_query_place::model_query_place;
use sqlbuild_rules::models::ModelQueryPlace;

use crate::compiled_project::main::reference_row_tuple::reference_row_tuple;
use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_project::models::ReferenceTuple;
use crate::compiled_views::_helpers::compiled_models::compiled_model_analysis::AnalysisRecord;
use crate::compiled_views::_helpers::compiled_models::compiled_model_analysis::AnalysisViews;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::AuditedSchema;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::CompiledArguments;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::CompiledRecord;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::DestinationRow;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::MODEL_RESOURCE;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::ModelOverlay;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::NULLABLE;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::SQL_ANALYSIS_KEY;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::ViewProjections;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::casefolded_families;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::compiled_of;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::compiled_record;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::lineage_input;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::model_name;
use crate::compiled_views::_helpers::model_inputs::inputs_view::InputsFields;
use crate::compiled_views::_helpers::model_inputs::model_input_view::attached_schema_name;
use crate::compiled_views::_helpers::model_inputs::model_input_view::authored_expansion;
use crate::compiled_views::_helpers::model_inputs::model_input_view::expansion_object;
use crate::compiled_views::_helpers::model_inputs::model_input_view::model_relation_key_row;
use crate::compiled_views::_helpers::model_inputs::model_input_view::query_sql_text;
use crate::compiled_views::_helpers::model_inputs::model_input_view::read_input;
use crate::compiled_views::_helpers::model_inputs::model_input_view::read_input_with_owner;
use crate::compiled_views::_helpers::model_inputs::model_input_view::references;
use crate::compiled_views::_helpers::model_inputs::model_input_view::rejected_opt_out;
use crate::compiled_views::_helpers::model_inputs::model_input_view::schema_entry;
use crate::compiled_views::_helpers::project::project_view::CompilerHandles;
use crate::compiled_views::_helpers::project::project_view::ProjectFields;
use crate::compiled_views::_helpers::project::project_view::SCHEMA_ENTRY_ATTRIBUTE;
use crate::compiled_views::_helpers::project::project_view::TAGS_ATTRIBUTE;
use crate::compiled_views::_helpers::project::project_view::resource_row;
use crate::compiled_views::main::compiled_models::defaulted_destination::defaulted_destination;
use crate::compiled_views::main::compiled_models::model_graph_row::model_graph_row;
use crate::compiled_views::main::compiled_models::object_key::object_key;
use crate::compiled_views::main::compiled_models::relation_location::relation_location;
use crate::compiled_views::main::model_inputs::view_classes::view_classes;
use crate::compiled_views::types::DeclaredRow;
use crate::compiled_views::types::LineageInput;
use crate::compiled_views::types::ModelInputRecord;
use crate::compiled_views::types::RelationKeyTuple;
use crate::graph::main::project::project_graph::project_graph;
use crate::graph::models::NativeProjectGraph;
use crate::graph::models::ResourceRow;
use crate::resource_views::models::AuditHandles;
use crate::resource_views::models::AuditProjections;
use crate::resource_views::models::FunctionHandles;
use crate::resource_views::models::FunctionProjections;
use crate::resource_views::models::NativeCompiledFunction;
use crate::resource_views::models::NativeCompiledSeed;
use crate::resource_views::models::NativeCompiledSource;
use crate::resource_views::models::SeedHandles;
use crate::resource_views::models::SourceHandles;
use crate::resource_views::models::SourceProjections;
use pyo3::FromPyObject;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyFrozenSet;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3::types::PyTupleMethods;
use sqlbuild_analysis::assembly::analysis_session::main::contract_proof_json::contract_proof_json;
use sqlbuild_analysis::assembly::analysis_session::models::LineageRow;
use sqlbuild_analysis::compiled_project::main::model_facts::model_facts;
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;
use sqlbuild_analysis::compiled_project::models::DynamicProofFact;
use sqlbuild_analysis::compiled_project::models::ModelAnalysisFact;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_project::models::DestinationFact;
use sqlbuild_project::models::ExpansionRecord;
use sqlbuild_project::models::ModelMetadataTexts;
use sqlbuild_project::types::InferredRow;
use sqlbuild_project::types::ProjectHost;
use sqlbuild_python_base::compile_driver::constants::LINEAGE_GRAPH_BUILDS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::models::NativeModelFile;
use sqlbuild_python_front::model_config::models::HeaderAudits;
use sqlbuild_python_front::model_config::models::HeaderColumns;
use sqlbuild_python_front::model_config::models::HeaderFamilies;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

/// Python projections of the analysis, each built once on first access.
pub struct AnalysisProjections {
    pub(crate) lineage: PyOnceLock<Py<PyAny>>,
    pub(crate) binding: PyOnceLock<Py<PyTuple>>,
    pub(crate) proof: PyOnceLock<Py<PyAny>>,
}

impl Default for AnalysisProjections {
    fn default() -> Self {
        Self {
            lineage: PyOnceLock::new(),
            binding: PyOnceLock::new(),
            proof: PyOnceLock::new(),
        }
    }
}

/// A model's SQL rule inputs; `excluded` is `None` when its config values could not be read natively.
#[derive(Debug)]
pub struct SqlRuleInputs {
    pub posix_path: String,
    pub excluded: Option<bool>,
    pub key: Option<SqlRuleKey>,
}

/// What a model's SQL rule cache key reads.
#[derive(Debug)]
pub struct SqlRuleKey {
    pub authored_sql: String,
    pub query_sql: String,
    /// The canonical JSON text of the model's dynamic pivot proof.
    pub contract: Option<String>,
}

/// The Python classes a compiled model's projected attributes are built with.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct CompiledViewClasses<'py> {
    pub(crate) object_key: Bound<'py, PyAny>,
    pub(crate) relation_location: Bound<'py, PyAny>,
    pub inferred_column: Bound<'py, PyAny>,
    pub nullability: Bound<'py, PyAny>,
    pub compiler_diagnostic: Bound<'py, PyAny>,
    pub source_location: Bound<'py, PyAny>,
    pub compact_lineage_facts: Bound<'py, PyAny>,
    pub lineage_column_fact: Bound<'py, PyAny>,
    pub lineage_source_fact: Bound<'py, PyAny>,
    pub contract_proof: Bound<'py, PyAny>,
    pub family_proof: Bound<'py, PyAny>,
}

/// A compiled model's Python attribute projections, each built once on first access.
pub struct CompiledProjections {
    pub(crate) key: PyOnceLock<Py<PyAny>>,
    pub(crate) deps: PyOnceLock<Py<PyTuple>>,
}

impl Default for CompiledProjections {
    fn default() -> Self {
        Self {
            key: PyOnceLock::new(),
            deps: PyOnceLock::new(),
        }
    }
}

/// One compiled model, projected from the owner on demand.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeCompiledModel {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
    pub(crate) overlay: ModelOverlay,
    pub(crate) projections: ViewProjections,
}

impl NativeCompiledModel {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self::with_overlay(owner, index, ModelOverlay::default())
    }

    pub(crate) fn with_overlay(
        owner: Py<NativeCompiledProject>,
        index: usize,
        overlay: ModelOverlay,
    ) -> Self {
        Self {
            owner,
            index,
            overlay,
            projections: ViewProjections::default(),
        }
    }

    pub(crate) fn changed(&self, py: Python<'_>, change: impl FnOnce(&mut ModelOverlay)) -> Self {
        let mut overlay: ModelOverlay = self.overlay.clone_ref(py);
        change(&mut overlay);
        Self::with_overlay(self.owner.clone_ref(py), self.index, overlay)
    }

    pub fn read<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&ModelInputRecord, Python<'py>) -> PyResult<T>,
    ) -> PyResult<T> {
        read_input(&self.owner, self.index, py, read)
    }

    pub(crate) fn compiled<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&CompiledRecord, Python<'py>) -> PyResult<T>,
    ) -> PyResult<T> {
        self.read(py, |record, py| read(compiled_of(record)?, py))
    }

    /// Read the effective destination, including namespace overlays, without a Python projection.
    pub fn read_destination<T>(
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
    pub fn analysis_fact(&self, py: Python<'_>) -> PyResult<(String, ModelAnalysisFact)> {
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
    pub fn lineage_inputs(&self, py: Python<'_>) -> PyResult<Option<Vec<LineageInput>>> {
        self.compiled(py, |compiled, _| {
            Ok(compiled
                .analysis
                .lineage_rows()
                .map(|rows| rows.iter().map(lineage_input).collect()))
        })
    }

    /// Whether the model's dynamic pivot proof proved its output columns.
    pub fn output_proven(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| {
            Ok(compiled
                .analysis
                .proof
                .as_ref()
                .is_some_and(|proof| proof.output_proven))
        })
    }

    /// Output columns whose types semantic recovery forgot.
    pub fn unchecked_columns(&self) -> &[String] {
        &self.overlay.poisoned
    }

    /// The kept lineage rows with their transform and confidence codes, or `None`.
    pub fn lineage_rows(&self, py: Python<'_>) -> PyResult<Option<Vec<LineageRow>>> {
        self.compiled(py, |compiled, _| {
            Ok(compiled.analysis.lineage_rows().map(<[LineageRow]>::to_vec))
        })
    }

    /// The input relations of a dynamic pivot proof that proved its outputs, else `None`.
    pub fn proven_inputs(&self, py: Python<'_>) -> PyResult<Option<Vec<String>>> {
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
    pub fn declared_columns(&self, py: Python<'_>) -> PyResult<Option<Py<PyTuple>>> {
        self.read(py, |record, py| {
            record
                .schema
                .as_ref()
                .map(|schema| schema.host.columns.project(py).map(Bound::unbind))
                .transpose()
        })
    }

    /// The attached schema's entry audits, its column specs and each audited column's index and name.
    pub fn schema_audits_and_columns(&self, py: Python<'_>) -> PyResult<Option<AuditedSchema>> {
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
    pub fn sql_rule_inputs(&self, py: Python<'_>, keyed: bool) -> PyResult<SqlRuleInputs> {
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
    pub fn file_texts(&self, py: Python<'_>) -> PyResult<(String, String, Arc<str>)> {
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
    pub fn metadata_texts(
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
    pub fn output_texts(&self, py: Python<'_>) -> PyResult<(String, Arc<str>)> {
        self.read(py, |record, py| {
            Ok((
                record.host.model_file.get().posix_path(py)?,
                Arc::clone(&record.query_sql),
            ))
        })
    }

    /// The model's authored file text and expanded query, from its recorded facts.
    pub fn sql_texts(&self, py: Python<'_>) -> PyResult<(String, String)> {
        self.with_facts(py, |facts| {
            (facts.authored_sql.to_string(), facts.query_sql.to_string())
        })
    }

    /// `read` applied to the model's recorded facts.
    pub fn with_facts<T>(
        &self,
        py: Python<'_>,
        read: impl FnOnce(&CompiledModelFacts) -> T,
    ) -> PyResult<T> {
        let name: String = self.name(py)?;
        let owner = self.owner.bind(py).borrow();
        let facts: &CompiledModelFacts = model_facts(owner.facts(), &name).ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("the model was not assembled")
        })?;
        Ok(read(facts))
    }

    pub(crate) fn model_file<T>(
        &self,
        py: Python<'_>,
        read: impl FnOnce(&NativeModelFile) -> PyResult<T>,
    ) -> PyResult<T> {
        self.read(py, |record, _| read(record.host.model_file.get()))
    }
}

#[pymethods]
impl NativeCompiledModel {
    #[getter]
    pub fn key(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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
    pub fn deps(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
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
    pub fn name(&self, py: Python<'_>) -> PyResult<String> {
        self.read(py, |record, _| Ok(model_name(record)))
    }

    #[getter]
    pub fn relative_path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.model_file(py, |file| file.relative_path(py))
    }

    #[getter]
    pub fn query_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.read(py, |record, py| Ok(query_sql_text(py, record)))
    }

    #[getter]
    pub fn config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(record.host.config.clone_ref(py)))
    }

    #[getter]
    pub fn destination(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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
    pub fn references(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| references(&classes, record))
    }

    /// Each reference as `(kind, name, package, call argument count)`.
    #[getter]
    pub fn reference_rows(&self, py: Python<'_>) -> PyResult<Vec<ReferenceTuple>> {
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
    pub fn relation_key_row(&self, py: Python<'_>) -> PyResult<RelationKeyRow> {
        read_input_with_owner(&self.owner, self.index, py, |owner, record| {
            model_relation_key_row(owner, record, py)
        })
    }

    /// `(resource type, name)` of the model's key.
    #[getter]
    pub fn key_row(&self, py: Python<'_>) -> PyResult<(String, String)> {
        self.read(py, |record, _| {
            Ok((MODEL_RESOURCE.to_owned(), model_name(record)))
        })
    }

    /// `(resource type, name)` of each dependency.
    #[getter]
    pub fn dep_rows(&self, py: Python<'_>) -> PyResult<Vec<(String, String)>> {
        self.compiled(py, |compiled, _| Ok(compiled.deps.clone()))
    }

    /// `(database, schema, name)` of the model's destination.
    #[getter]
    pub fn destination_row(&self, py: Python<'_>) -> PyResult<DestinationRow> {
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
    pub fn inferred_rows(&self, py: Python<'_>) -> PyResult<Option<Vec<InferredRow>>> {
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
    pub fn schema_column_rows(&self, py: Python<'_>) -> PyResult<Option<Vec<DeclaredRow>>> {
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
    pub fn dynamic_columns_declared(&self, py: Python<'_>) -> PyResult<bool> {
        self.read(py, |record, _py| {
            Ok(record
                .schema
                .as_ref()
                .is_some_and(|schema| !schema.host.dynamic_columns.specs.is_empty()))
        })
    }

    /// Inferred columns, else the attached schema's declared columns, else zero.
    #[getter]
    pub fn output_column_count(&self, py: Python<'_>) -> PyResult<usize> {
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
    pub fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| schema_entry(&classes, record))
    }

    #[getter]
    pub fn inferred_columns(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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
    pub fn lineage_edge_count(&self, py: Python<'_>) -> PyResult<Option<usize>> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.lineage_edge_count()))
    }

    #[getter]
    pub fn fast_lineage_columns(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.compiled(py, |compiled, py| {
            compiled.analysis.lineage_object(py, &classes.compiled)
        })
    }

    #[getter]
    pub fn fast_lineage_has_star(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.has_star))
    }

    #[getter]
    pub fn fast_lineage_star_resolved(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.star_resolved))
    }

    #[getter]
    pub fn authored_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.model_file(py, |file| Ok(file.contents(py)))
    }

    /// Where the expanded query sits in the model file, as JSON a rule's SQL document maps positions through.
    #[getter]
    pub fn query_place(&self, py: Python<'_>) -> PyResult<String> {
        let place: ModelQueryPlace = self.with_facts(py, model_query_place)?;
        serde_json::to_string(&place)
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))
    }

    #[getter]
    pub fn authored_query_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.model_file(py, |file| Ok(file.query_sql(py)))
    }

    #[getter]
    pub fn output_column_locations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        self.model_file(py, |file| file.output_column_locations(py))
    }

    #[getter]
    pub fn extract_implicit_alias_columns(&self, py: Python<'_>) -> PyResult<bool> {
        self.model_file(py, |file| Ok(file.extract_implicit_alias_columns()))
    }

    #[getter]
    pub fn macro_deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.compiled(py, |compiled, py| PyTuple::new(py, &compiled.macro_deps))
    }

    #[getter]
    pub fn enum_declarations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.enum_declarations.clone_ref(py))
        })
    }

    #[getter]
    pub fn constant_declarations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.constant_declarations.clone_ref(py))
        })
    }

    #[getter]
    pub fn enum_columns(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.read(py, |record, py| Ok(record.host.enum_columns.clone_ref(py)))
    }

    #[getter]
    pub fn binding_diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
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
    pub fn binding_validated(&self, py: Python<'_>) -> PyResult<bool> {
        self.compiled(py, |compiled, _| Ok(compiled.analysis.binding_validated))
    }

    #[getter]
    pub fn dynamic_column_contract(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.compiled(py, |compiled, py| {
            compiled.analysis.proof_object(py, &classes.compiled)
        })
    }

    #[getter]
    pub fn unchecked_output_columns<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyFrozenSet>> {
        PyFrozenSet::new(py, &self.overlay.poisoned)
    }

    #[getter]
    pub fn rejected_sql_analysis_opt_out(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(rejected_opt_out(record, py)))
    }

    /// This model with its final binding diagnostics.
    pub(crate) fn with_binding_diagnostics(
        &self,
        py: Python<'_>,
        diagnostics: Py<PyTuple>,
    ) -> Self {
        self.changed(py, |overlay| {
            overlay.binding_diagnostics = Some(diagnostics)
        })
    }

    /// This model after semantic type recovery: its bindings and the outputs left unchecked.
    pub(crate) fn with_type_recovery(
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
    pub fn with_target_defaults(
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

/// One compile's attached inputs, its model inputs projected from the owner.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeInputsView {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) fields: InputsFields,
}

impl NativeInputsView {
    /// The view over `owner` with these input fields.
    pub fn new(owner: Py<NativeCompiledProject>, fields: &Bound<'_, PyDict>) -> PyResult<Self> {
        Ok(Self {
            owner,
            fields: InputsFields::from_fields(fields)?,
        })
    }
}

#[pymethods]
impl NativeInputsView {
    /// These inputs with `changes` applied; the owner's model inputs are shared.
    #[pyo3(signature = (**changes))]
    pub(crate) fn with_changes(
        &self,
        py: Python<'_>,
        changes: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let current: Bound<'_, PyDict> = self.fields.as_fields(py)?;
        if let Some(changes) = changes {
            for (name, value) in changes.iter() {
                if !current.contains(&name)? {
                    return Err(PyTypeError::new_err(format!(
                        "unknown compile input field {name}"
                    )));
                }
                current.set_item(name, value)?;
            }
        }
        Self::new(self.owner.clone_ref(py), &current)
    }

    /// Every field by name, for an explicit `CompileProjectInputs` projection.
    pub(crate) fn fields<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let fields: Bound<'py, PyDict> = self.fields.as_fields(py)?;
        fields.set_item("native_project", self.owner.bind(py))?;
        Ok(fields)
    }

    #[getter]
    pub fn run_id(&self) -> String {
        self.fields.run_id.clone()
    }

    #[getter]
    pub fn effective_target_name(&self) -> Option<String> {
        self.fields.effective_target_name.clone()
    }

    #[getter(no_sql_validation)]
    pub fn is_sql_validation_skipped(&self) -> bool {
        self.fields.no_sql_validation
    }

    #[getter]
    pub fn model_inputs(&self, py: Python<'_>) -> Py<PyTuple> {
        self.fields.model_inputs.clone_ref(py)
    }

    #[getter]
    pub fn native_project(&self, py: Python<'_>) -> Py<NativeCompiledProject> {
        self.owner.clone_ref(py)
    }

    #[getter]
    pub fn project_config(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.project_config.clone_ref(py)
    }

    #[getter]
    pub fn local_config(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.local_config.clone_ref(py)
    }

    #[getter]
    pub fn discovered_inputs(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.discovered_inputs.clone_ref(py)
    }

    #[getter]
    pub fn sql_lexical_syntax(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.sql_lexical_syntax.clone_ref(py)
    }

    #[getter]
    pub fn effective_target(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.effective_target.clone_ref(py)
    }

    #[getter]
    pub fn compile_cache_dir(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.compile_cache_dir.clone_ref(py)
    }

    #[getter]
    pub fn effective_connection(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.effective_connection.clone_ref(py)
    }

    #[getter]
    pub fn effective_vars(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.effective_vars.clone_ref(py)
    }

    #[getter]
    pub fn macro_context(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.macro_context.clone_ref(py)
    }

    #[getter]
    pub fn loaded_macros(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.loaded_macros.clone_ref(py)
    }

    #[getter]
    pub fn public_enums(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.public_enums.clone_ref(py)
    }

    #[getter]
    pub fn public_constants(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.public_constants.clone_ref(py)
    }

    #[getter]
    pub fn external_sql_reference_resolver(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields
            .boundary
            .external_sql_reference_resolver
            .clone_ref(py)
    }

    #[getter]
    pub fn effective_settings(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.effective_settings.clone_ref(py)
    }

    #[getter]
    pub fn seed_inputs(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.seed_inputs.clone_ref(py)
    }

    #[getter]
    pub fn source_inputs(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.source_inputs.clone_ref(py)
    }

    #[getter]
    pub fn sql_function_inputs(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.sql_function_inputs.clone_ref(py)
    }

    #[getter]
    pub fn test_inputs(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.test_inputs.clone_ref(py)
    }

    #[getter]
    pub fn scenario_inputs(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.scenario_inputs.clone_ref(py)
    }

    #[getter]
    pub fn audit_inputs(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.audit_inputs.clone_ref(py)
    }

    #[getter]
    pub fn diagnostics(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.diagnostics.clone_ref(py)
    }

    #[getter]
    pub fn scope_index(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.scope_index.clone_ref(py)
    }

    #[getter]
    pub fn declaration_scope(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.declaration_scope.clone_ref(py)
    }
}

/// The Python classes a model input's projected attributes are built with.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct InputViewClasses<'py> {
    pub compiled: CompiledViewClasses<'py>,
    pub(crate) sql_reference: Bound<'py, PyAny>,
    pub(crate) reference_kind: Bound<'py, PyAny>,
    pub(crate) schema_model_entry: Bound<'py, PyAny>,
    pub(crate) sql_expansion: Bound<'py, PyAny>,
    pub(crate) expansion_span: Bound<'py, PyAny>,
}

/// A model schema's header columns and specs, which carry user-authored audit arguments.
pub struct SchemaHandles {
    pub columns: Arc<HeaderColumns>,
    pub audits: Arc<HeaderAudits>,
    pub dynamic_columns: Arc<HeaderFamilies>,
}

/// A model input's user values and the attribute projections its views build.
pub struct ModelInputHandles {
    /// The discovered model file, owned by the discovery inputs.
    pub model_file: Py<NativeModelFile>,
    /// The resolved config, whose values hold user values.
    pub config: Py<PyAny>,
    pub rejected_opt_out: Option<Py<PyAny>>,
    /// The model's own enums and constants, whose values are user values.
    pub enum_declarations: Py<PyTuple>,
    pub constant_declarations: Py<PyTuple>,
    pub enum_columns: Py<PyDict>,
    /// Usage records, the identities the scope index is built from.
    pub macro_usages: Py<PyTuple>,
    pub declaration_usages: Py<PyTuple>,
    /// Attribute projections built on first access, shared by every view of the model.
    pub projections: InputProjections,
}

/// A model input's Python attribute projections, each built once on first access.
pub struct InputProjections {
    pub(crate) query_sql: PyOnceLock<Py<PyString>>,
    pub(crate) references: PyOnceLock<Py<PyTuple>>,
    pub(crate) schema_entry: PyOnceLock<Py<PyAny>>,
    pub(crate) sql_expansion: PyOnceLock<Py<PyAny>>,
    pub(crate) lint_expansion: PyOnceLock<Py<PyAny>>,
}

impl std::fmt::Debug for InputProjections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InputProjections")
            .finish_non_exhaustive()
    }
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
#[derive(Debug)]
pub struct NativeModelInput {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) index: usize,
}

impl NativeModelInput {
    pub fn new(owner: Py<NativeCompiledProject>, index: usize) -> Self {
        Self { owner, index }
    }

    pub fn read<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&ModelInputRecord, Python<'py>) -> PyResult<T>,
    ) -> PyResult<T> {
        read_input(&self.owner, self.index, py, read)
    }

    /// Read the input record together with the project that owns it.
    pub fn read_with_owner<'py, T>(
        &self,
        py: Python<'py>,
        read: impl FnOnce(&NativeCompiledProject, &ModelInputRecord) -> PyResult<T>,
    ) -> PyResult<T> {
        read_input_with_owner(&self.owner, self.index, py, read)
    }
}

#[pymethods]
impl NativeModelInput {
    #[getter]
    pub fn model_file(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| {
            Ok(record.host.model_file.clone_ref(py).into_any())
        })
    }

    #[getter]
    pub fn config(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(record.host.config.clone_ref(py)))
    }

    #[getter]
    pub fn query_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.read(py, |record, py| Ok(query_sql_text(py, record)))
    }

    #[getter]
    pub fn macro_source_sql<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyString>> {
        self.read(py, |record, py| {
            Ok(PyString::new(py, &record.macro_source_sql))
        })
    }

    #[getter]
    pub fn references(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| references(&classes, record))
    }

    #[getter]
    pub fn schema_entry(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| schema_entry(&classes, record))
    }

    /// Each reference as `(kind, name, package, call argument count)`.
    #[getter]
    pub fn reference_rows(&self, py: Python<'_>) -> PyResult<Vec<ReferenceTuple>> {
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
    pub fn relation_key_row(&self, py: Python<'_>) -> PyResult<RelationKeyRow> {
        self.read_with_owner(py, |owner, record| {
            model_relation_key_row(owner, record, py)
        })
    }

    /// The name of the schema entry attached to the model, or `None`.
    #[getter]
    pub fn attached_schema_name(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.read(py, |record, _| Ok(attached_schema_name(record)))
    }

    #[getter]
    pub fn schema_file(&self, py: Python<'_>) -> Py<PyAny> {
        py.None()
    }

    #[getter]
    pub fn sql_validation_enabled(&self, py: Python<'_>) -> PyResult<bool> {
        self.read(py, |record, _| Ok(record.sql_validation_enabled))
    }

    #[getter]
    pub fn rejected_sql_analysis_opt_out(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.read(py, |record, py| Ok(rejected_opt_out(record, py)))
    }

    #[getter]
    pub fn enum_declarations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.enum_declarations.clone_ref(py))
        })
    }

    #[getter]
    pub fn constant_declarations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.constant_declarations.clone_ref(py))
        })
    }

    #[getter]
    pub fn enum_columns(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.read(py, |record, py| Ok(record.host.enum_columns.clone_ref(py)))
    }

    #[getter]
    pub fn macro_deps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.read(py, |record, py| PyTuple::new(py, &record.macro_deps))
    }

    #[getter]
    pub fn macro_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| Ok(record.host.macro_usages.clone_ref(py)))
    }

    #[getter]
    pub fn declaration_usages(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        self.read(py, |record, py| {
            Ok(record.host.declaration_usages.clone_ref(py))
        })
    }

    /// Record what assembly finished for the model; return its compiled model view.
    pub(crate) fn compiled_model(
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
    pub fn sql_expansion(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
        self.read(py, |record, _| authored_expansion(&classes, record))
    }

    /// The whole expansion, variables included, which early SQL lint reuses.
    #[getter]
    pub fn lint_expansion(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
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

/// A model input's attached schema: its name and dynamic column families.
#[derive(Debug)]
pub struct AttachedSchema {
    pub name: String,
    pub dynamic_columns: Vec<DynamicFamily>,
    pub type_enforcement: bool,
}

/// One lineage output with its transform kind and confidence values, and its sources.
#[derive(Debug)]
pub struct LineageFactRow {
    pub output_column: String,
    pub transform: String,
    pub confidence: String,
    /// `(resource type, resource, column)` of each source.
    pub sources: Vec<(String, String, String)>,
}

/// The extension's host: Python handles and the views built from them.
#[derive(Debug)]
pub struct PyProjectHost;

impl ProjectHost for PyProjectHost {
    type ModelInput = ModelInputHandles;
    type Schema = SchemaHandles;
    type CompiledModel = CompiledProjections;
    type Analysis = AnalysisProjections;
    type AttachedSource = SourceHandles;
    type Source = SourceProjections;
    type AttachedFunction = FunctionHandles;
    type Function = FunctionProjections;
    type AttachedAudit = AuditHandles;
    type Audit = AuditProjections;
    type Seed = SeedHandles;
}

/// One compiled project, its models projected from the owner.
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct NativeProjectView {
    pub(crate) owner: Py<NativeCompiledProject>,
    pub(crate) fields: ProjectFields,
    pub(crate) lineage_graph: PyOnceLock<Py<NativeProjectGraph>>,
    pub(crate) sql_expansions: PyOnceLock<Py<PyDict>>,
}

impl std::fmt::Debug for NativeProjectView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeProjectView")
            .finish_non_exhaustive()
    }
}

impl NativeProjectView {
    /// The view over `owner` with these project fields.
    pub fn new(owner: Py<NativeCompiledProject>, fields: &Bound<'_, PyDict>) -> PyResult<Self> {
        Ok(Self::with_fields(
            owner,
            ProjectFields::from_fields(fields)?,
        ))
    }

    /// The project field custom-rule facts and rules runs read by `name`, or `None` for any other.
    pub fn fact_field(&self, py: Python<'_>, name: &str) -> Option<Py<PyAny>> {
        let field: &Py<PyAny> = match name {
            "settings" => &self.fields.compiler.settings,
            "sql_scenarios" => &self.fields.compiler.sql_scenarios,
            "scope_index" => &self.fields.compiler.scope_index,
            "diagnostics" => &self.fields.compiler.diagnostics,
            "sources" => &self.fields.compiler.sources,
            "seeds" => &self.fields.compiler.seeds,
            "functions" => &self.fields.compiler.functions,
            "audits" => &self.fields.compiler.audits,
            "sql_tests" => &self.fields.compiler.sql_tests,
            "public_enums" => &self.fields.boundary.public_enums,
            "public_constants" => &self.fields.boundary.public_constants,
            "models" => return Some(self.fields.models.clone_ref(py).into_any()),
            _ => return None,
        };
        Some(field.clone_ref(py))
    }

    /// The compiled project this view projects.
    pub fn owner(&self) -> &Py<NativeCompiledProject> {
        &self.owner
    }

    pub(crate) fn with_fields(owner: Py<NativeCompiledProject>, fields: ProjectFields) -> Self {
        Self {
            owner,
            fields,
            lineage_graph: PyOnceLock::new(),
            sql_expansions: PyOnceLock::new(),
        }
    }
}

#[pymethods]
impl NativeProjectView {
    /// This project with `changes` applied; the owner's models are shared.
    #[pyo3(signature = (**changes))]
    pub(crate) fn with_changes(
        &self,
        py: Python<'_>,
        changes: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let fields: ProjectFields = match changes {
            Some(changes) => self.fields.changed(py, &changes)?,
            None => self.fields.changed(py, &PyDict::new(py))?,
        };
        Ok(Self::with_fields(self.owner.clone_ref(py), fields))
    }

    /// Every field by name, for an explicit `CompiledProject` projection.
    #[pyo3(signature = (include_sql_expansions = true))]
    pub fn fields<'py>(
        &self,
        py: Python<'py>,
        include_sql_expansions: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        let fields: Bound<'py, PyDict> = self.fields.as_fields(py)?;
        fields.set_item(
            "sql_expansions",
            if include_sql_expansions {
                self.sql_expansions(py)?
            } else {
                PyDict::new(py).unbind()
            },
        )?;
        fields.set_item("native_project", self.owner.bind(py))?;
        Ok(fields)
    }

    #[getter]
    pub fn run_id(&self) -> String {
        self.fields.run_id.clone()
    }

    #[getter]
    pub fn effective_target_name(&self) -> Option<String> {
        self.fields.effective_target_name.clone()
    }

    #[getter]
    pub fn effective_target_database(&self) -> Option<String> {
        self.fields.effective_target_database.clone()
    }

    #[getter]
    pub fn effective_target_schema(&self) -> Option<String> {
        self.fields.effective_target_schema.clone()
    }

    #[getter]
    pub fn sql_analysis_dialect(&self) -> Option<String> {
        self.fields.sql_analysis_dialect.clone()
    }

    #[getter(enforce_explicit_references)]
    pub fn is_explicit_reference_enforced(&self) -> bool {
        self.fields.enforce_explicit_references
    }

    #[getter]
    pub fn models(&self, py: Python<'_>) -> Py<PyTuple> {
        self.fields.models.clone_ref(py)
    }

    #[getter]
    pub fn native_project(&self, py: Python<'_>) -> Py<NativeCompiledProject> {
        self.owner.clone_ref(py)
    }

    #[getter]
    pub fn effective_connection(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.effective_connection.clone_ref(py)
    }

    #[getter]
    pub fn effective_vars(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.effective_vars.clone_ref(py)
    }

    #[getter]
    pub fn binding_catalog(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.binding_catalog.clone_ref(py)
    }

    #[getter]
    pub fn sql_lexical_syntax(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.sql_lexical_syntax.clone_ref(py)
    }

    #[getter]
    pub fn compile_cache_dir(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.compile_cache_dir.clone_ref(py)
    }

    #[getter]
    pub fn loader_functions(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.loader_functions.clone_ref(py)
    }

    #[getter]
    pub fn loads<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        load_records(py, &self.fields.loads)
    }

    #[getter]
    pub fn python_execution_inputs(&self) -> BTreeMap<String, (String, PathBuf)> {
        self.fields.boundary.python_execution_inputs.clone()
    }

    #[getter]
    pub fn hook_functions(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.hook_functions.clone_ref(py)
    }

    #[getter]
    pub fn sql_hook_files(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.sql_hook_files.clone_ref(py)
    }

    #[getter]
    pub fn materialization_files(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.materialization_files.clone_ref(py)
    }

    #[getter]
    pub fn public_enums(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.public_enums.clone_ref(py)
    }

    #[getter]
    pub fn public_constants(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.public_constants.clone_ref(py)
    }

    #[getter]
    pub fn loaded_macros(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.loaded_macros.clone_ref(py)
    }

    #[getter]
    pub fn external_sql_reference_resolver(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields
            .boundary
            .external_sql_reference_resolver
            .clone_ref(py)
    }

    #[getter]
    pub fn native_session(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.native_session.clone_ref(py)
    }

    #[getter]
    pub fn settings(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.settings.clone_ref(py)
    }

    #[getter]
    pub fn scenario(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.scenario.clone_ref(py)
    }

    #[getter]
    pub fn sources(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.sources.clone_ref(py)
    }

    #[getter]
    pub fn seeds(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.seeds.clone_ref(py)
    }

    #[getter]
    pub fn functions(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.functions.clone_ref(py)
    }

    #[getter]
    pub fn audits(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.audits.clone_ref(py)
    }

    #[getter]
    pub fn sql_tests(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.sql_tests.clone_ref(py)
    }

    #[getter]
    pub fn sql_scenarios(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.sql_scenarios.clone_ref(py)
    }

    #[getter]
    pub fn diagnostics(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.diagnostics.clone_ref(py)
    }

    #[getter]
    pub fn unmatched_literal_sql_relations(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields
            .compiler
            .unmatched_literal_sql_relations
            .clone_ref(py)
    }

    #[getter]
    pub fn scope_index(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.scope_index.clone_ref(py)
    }

    /// Each model's authored-to-expanded passes by file path, where interpolation kept the query.
    #[getter]
    pub fn sql_expansions(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.sql_expansions
            .get_or_try_init(py, || {
                let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                let owner = self.owner.bind(py).borrow();
                let expansions: Bound<'_, PyDict> = PyDict::new(py);
                for record in owner.inputs() {
                    let expansion: Py<PyAny> = authored_expansion(&classes, record)?;
                    if !expansion.is_none(py) {
                        let path: Bound<'_, PyAny> =
                            record.host.model_file.get().path_objects(py)?.0;
                        expansions.set_item(path, expansion)?;
                    }
                }
                Ok::<_, pyo3::PyErr>(expansions.unbind())
            })
            .map(|expansions| expansions.clone_ref(py))
    }

    /// The lineage edges, tags, model folders and names, indexed natively once per project.
    #[getter]
    pub fn lineage_graph(&self, py: Python<'_>) -> PyResult<Py<NativeProjectGraph>> {
        self.lineage_graph
            .get_or_try_init(py, || {
                let mut rows: Vec<ResourceRow> = Vec::new();
                for model in self.fields.models.bind(py).iter() {
                    let model: Bound<'_, NativeCompiledModel> = model.cast_into()?;
                    rows.push(model_graph_row(model.get(), py)?);
                }
                let compiler: &CompilerHandles = &self.fields.compiler;
                for source in compiler.sources.bind(py).try_iter()? {
                    let source: Bound<'_, PyAny> = source?;
                    rows.push(match source.cast::<NativeCompiledSource>() {
                        Ok(native) => native.get().graph_row(py)?,
                        Err(_) => resource_row(&source, None)?,
                    });
                }
                for seed in compiler.seeds.bind(py).try_iter()? {
                    let seed: Bound<'_, PyAny> = seed?;
                    rows.push(match seed.cast::<NativeCompiledSeed>() {
                        Ok(native) => native.get().graph_row(py)?,
                        Err(_) => {
                            let tags: Bound<'_, PyAny> = native_field(
                                &native_field(&seed, SCHEMA_ENTRY_ATTRIBUTE)?,
                                TAGS_ATTRIBUTE,
                            )?;
                            resource_row(&seed, Some(&tags))?
                        }
                    });
                }
                for function in compiler.functions.bind(py).try_iter()? {
                    let function: Bound<'_, PyAny> = function?;
                    rows.push(match function.cast::<NativeCompiledFunction>() {
                        Ok(native) => native.get().graph_row(py)?,
                        Err(_) => {
                            let tags: Bound<'_, PyAny> = native_field(&function, TAGS_ATTRIBUTE)?;
                            resource_row(&function, Some(&tags))?
                        }
                    });
                }
                rows.extend(load_rows(load_records(py, &self.fields.loads)?.as_any())?);
                self.owner.bind(py).borrow_mut().record_graph_build();
                count_stage(py, LINEAGE_GRAPH_BUILDS, 1)?;
                Py::new(py, project_graph(rows)?)
            })
            .map(|graph| graph.clone_ref(py))
    }
}

/// `(kind, name, materialized, mode, strategy, cursor, unique key, columns, audits)`.
#[derive(Debug)]
pub struct RelationKeyRow {
    pub kind: String,
    pub name: String,
    pub materialized: String,
    pub mode: String,
    pub strategy: String,
    pub has_cursor: bool,
    pub unique_key: Option<Vec<String>>,
    pub columns: Vec<(String, Vec<String>, bool)>,
    pub audits: Vec<(String, Option<String>)>,
}

impl<'py> pyo3::FromPyObject<'_, 'py> for RelationKeyRow {
    type Error = pyo3::PyErr;

    fn extract(value: pyo3::Borrowed<'_, 'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let value: &pyo3::Bound<'py, pyo3::PyAny> = &value;
        let (kind, name, materialized, mode, strategy, has_cursor, unique_key, columns, audits): RelationKeyTuple = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            kind,
            name,
            materialized,
            mode,
            strategy,
            has_cursor,
            unique_key,
            columns,
            audits,
        })
    }
}

impl<'py> pyo3::IntoPyObject<'py> for RelationKeyRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.kind,
                self.name,
                self.materialized,
                self.mode,
                self.strategy,
                self.has_cursor,
                self.unique_key,
                self.columns,
                self.audits,
            ),
            py,
        )
    }
}
