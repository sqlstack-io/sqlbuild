//! A compiled model, read from the native compiled project that owns it.

use crate::compiled_views::models::CompiledProjections;
use crate::compiled_views::types::LineageInput;
use sqlbuild_project::models as project_models;
use sqlbuild_project::models::DestinationFact;
use sqlbuild_project::types::InferredRow;
use sqlbuild_python_base::boundary::main::text_item::text_item;
use sqlbuild_python_front::model_config::models::HeaderAudits;
use std::sync::Arc;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyTuple};

use sqlbuild_analysis::assembly::analysis_session::models::LineageRow;

use crate::compiled_views::_helpers::compiled_models::compiled_model_analysis::{
    AnalysisRecord, ProofArgument, SessionArgument, analysis_record,
};
use crate::compiled_views::models::PyProjectHost;
use crate::compiled_views::types::ModelInputRecord;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::models::HeaderColumns;

pub(crate) const DATABASE_KEY: &str = "database";
pub(crate) const SCHEMA_KEY: &str = "schema";
const ALIAS_KEY: &str = "alias";
pub(crate) const VALUES_ATTRIBUTE: &str = "values";
pub(crate) const LOGICAL_SCHEMA_ATTRIBUTE: &str = "logical_schema";
pub(crate) const LOGICAL_DATABASE_ATTRIBUTE: &str = "logical_database";
pub(crate) const MODEL_RESOURCE: &str = "model";
pub(crate) const SQL_ANALYSIS_KEY: &str = "sql_analysis";
pub(crate) const TAGS_KEY: &str = "tags";
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
    let values: Bound<'_, PyDict> = native_field(config, VALUES_ATTRIBUTE)?.cast_into()?;
    Ok(CompiledRecord {
        deps,
        destination: DestinationFact {
            database: text_item(&values, DATABASE_KEY)?,
            schema: text_item(&values, SCHEMA_KEY)?,
            name: text_item(&values, ALIAS_KEY)?.unwrap_or_else(|| name.to_owned()),
            qualified_name: None,
            logical_schema: native_field(config, LOGICAL_SCHEMA_ATTRIBUTE)?.extract()?,
            logical_database: native_field(config, LOGICAL_DATABASE_ATTRIBUTE)?.extract()?,
        },
        analysis,
        macro_deps,
        host: CompiledProjections::default(),
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

pub(crate) fn model_name(record: &ModelInputRecord) -> String {
    record.name.clone()
}
