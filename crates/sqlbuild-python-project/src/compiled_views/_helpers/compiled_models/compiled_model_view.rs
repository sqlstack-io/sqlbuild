//! A compiled model, read from the native compiled project that owns it.

use crate::compiled_views::models::CompiledProjections;
use crate::compiled_views::models::CompiledViewClasses;
use crate::compiled_views::models::NativeCompiledModel;
use crate::compiled_views::types::LineageInput;
use sqlbuild_project::models as project_models;
use sqlbuild_project::models::DestinationFact;
use sqlbuild_project::types::InferredRow;
use sqlbuild_python_front::model_config::models::HeaderAudits;
use std::sync::Arc;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

use sqlbuild_analysis::assembly::analysis_session::models::LineageRow;

use crate::compiled_views::_helpers::compiled_models::compiled_model_analysis::AnalysisRecord;
use crate::compiled_views::_helpers::compiled_models::compiled_model_analysis::ProofArgument;
use crate::compiled_views::_helpers::compiled_models::compiled_model_analysis::SessionArgument;
use crate::compiled_views::_helpers::compiled_models::compiled_model_analysis::analysis_record;
use crate::compiled_views::models::PyProjectHost;
use crate::compiled_views::types::ModelInputRecord;
use crate::graph::models::ResourceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::models::HeaderColumns;

const DATABASE_KEY: &str = "database";
const SCHEMA_KEY: &str = "schema";
const ALIAS_KEY: &str = "alias";
const VALUES_ATTRIBUTE: &str = "values";
const LOGICAL_SCHEMA_ATTRIBUTE: &str = "logical_schema";
const LOGICAL_DATABASE_ATTRIBUTE: &str = "logical_database";
pub(crate) const MODEL_RESOURCE: &str = "model";
pub(crate) const SQL_ANALYSIS_KEY: &str = "sql_analysis";
const TAGS_KEY: &str = "tags";
pub(crate) const NULLABLE: &str = "nullable";

/// A schema's entry audits, column specs and each audited column's index and name.
pub(crate) type AuditedSchema = (Arc<HeaderAudits>, Arc<HeaderColumns>, Vec<(usize, String)>);

/// `(database, schema, name)` of a relation.
pub(crate) type DestinationRow = (Option<String>, Option<String>, String);

/// What assembly and the semantic checks finished for one compiled model.
pub(crate) type CompiledRecord = project_models::CompiledRecord<PyProjectHost>;

/// What later stages changed for one view of a model; the owner's record stays as assembled.
#[derive(Debug, Default)]
pub(crate) struct ModelOverlay {
    pub(crate) binding_diagnostics: Option<Py<PyTuple>>,
    /// Output columns whose types semantic recovery forgot, left unchecked by the contract.
    pub(crate) poisoned: Vec<String>,
    pub(crate) destination: Option<DestinationFact>,
}

impl ModelOverlay {
    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
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
pub(crate) struct ViewProjections {
    pub(crate) destination: PyOnceLock<Py<PyAny>>,
    pub(crate) inferred_columns: PyOnceLock<Py<PyAny>>,
}

impl std::fmt::Debug for ViewProjections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ViewProjections")
            .finish_non_exhaustive()
    }
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

/// A lineage row's output column with its `(resource name, column)` sources.
pub(crate) fn lineage_input(row: &LineageRow) -> LineageInput {
    let mut sources: Vec<(String, String)> = Vec::with_capacity(row.sources.len());
    for (_, resource, column) in &row.sources {
        sources.push((resource.clone(), column.clone()));
    }
    (row.output_column.clone(), sources)
}

/// Each family's name casefolded as Python's, with its inferred type.
pub(crate) fn casefolded_families(
    families: &[(String, Option<String>)],
) -> Vec<(String, Option<String>)> {
    families
        .iter()
        .map(|(name, kind)| (str::to_lowercase(name), kind.clone()))
        .collect()
}

/// The compiled record of an input, which assembly records before any view is handed out.
pub(crate) fn compiled_of(record: &ModelInputRecord) -> PyResult<&CompiledRecord> {
    record
        .compiled
        .as_ref()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("the model was not assembled"))
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

pub(crate) fn model_name(record: &ModelInputRecord) -> String {
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
