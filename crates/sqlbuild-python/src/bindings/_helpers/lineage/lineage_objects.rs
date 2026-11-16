//! The Python lineage models and enums native lineage builds.

use sqlbuild_host::entries::models::ExecutorHost;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_analysis::lineage::models::{LineageColumn, LineageSource};
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const LOGGING_MODULE: &str = "logging";
const LINEAGE_LOGGER: &str = "sqlbuild.lineage";

/// The lineage model classes and enums, imported once per call.
pub(crate) struct LineageClasses<'py> {
    pub(crate) qualified_column: Bound<'py, PyAny>,
    pub(crate) lineage_source: Bound<'py, PyAny>,
    pub(crate) lineage_edge: Bound<'py, PyAny>,
    pub(crate) column_lineage: Bound<'py, PyAny>,
    pub(crate) model_lineage: Bound<'py, PyAny>,
    pub(crate) semantic_use: Bound<'py, PyAny>,
    pub(crate) column_fact: Bound<'py, PyAny>,
    pub(crate) source_fact: Bound<'py, PyAny>,
    pub(crate) compact_facts: Bound<'py, PyAny>,
    pub(crate) transform_kind: Bound<'py, PyAny>,
    pub(crate) confidence: Bound<'py, PyAny>,
    pub(crate) nullability: Bound<'py, PyAny>,
    pub(crate) analysis_dialect: Bound<'py, PyAny>,
    pub(crate) resource_type: Bound<'py, PyAny>,
    pub(crate) model_resource: Bound<'py, PyAny>,
    pub(crate) unknown_nullability: Bound<'py, PyAny>,
}

impl<'py> LineageClasses<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        let resource_type: Bound<'py, PyAny> = record_class(py, RecordId::CompiledResourceType)?;
        let nullability: Bound<'py, PyAny> = record_class(py, RecordId::InferredNullability)?;
        Ok(Self {
            qualified_column: record_class(py, RecordId::QualifiedLineageColumn)?,
            lineage_source: record_class(py, RecordId::ColumnLineageSource)?,
            lineage_edge: record_class(py, RecordId::ColumnLineageEdge)?,
            column_lineage: record_class(py, RecordId::ColumnLineage)?,
            model_lineage: record_class(py, RecordId::ModelColumnLineage)?,
            semantic_use: record_class(py, RecordId::DirectSemanticColumnUse)?,
            column_fact: record_class(py, RecordId::CompiledLineageColumnFact)?,
            source_fact: record_class(py, RecordId::CompiledLineageSourceFact)?,
            compact_facts: record_class(py, RecordId::CompactLineageFacts)?,
            transform_kind: record_class(py, RecordId::ColumnTransformKind)?,
            confidence: record_class(py, RecordId::ColumnLineageConfidence)?,
            analysis_dialect: record_class(py, RecordId::PolyglotAnalysisDialect)?,
            model_resource: native_field(&resource_type, "MODEL")?,
            unknown_nullability: native_field(&nullability, "UNKNOWN")?,
            nullability,
            resource_type,
        })
    }

    pub(crate) fn py(&self) -> Python<'py> {
        self.qualified_column.py()
    }

    /// `QualifiedLineageColumn(resource_type, resource_name, column_name)`.
    pub(crate) fn qualified(
        &self,
        resource_type: &Bound<'py, PyAny>,
        resource_name: &str,
        column_name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.qualified_column.call1((
            self.resource_type.call1((resource_type,))?,
            resource_name,
            column_name,
        ))
    }

    /// `ColumnLineageEdge(source, target, transform_kind, confidence)`.
    pub(crate) fn edge(
        &self,
        source: Bound<'py, PyAny>,
        target: &Bound<'py, PyAny>,
        transform_kind: &Bound<'py, PyAny>,
        confidence: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.lineage_edge
            .call1((source, target, transform_kind, confidence))
    }

    /// `ModelColumnLineage(model_name, columns, has_star)`.
    pub(crate) fn model(
        &self,
        model_name: &str,
        columns: Vec<Bound<'py, PyAny>>,
        has_star: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.model_lineage
            .call1((model_name, PyTuple::new(self.py(), columns)?, has_star))
    }

    /// `ColumnLineage` of one native column; `nullability` is the enum member to record.
    pub(crate) fn column(
        &self,
        column: &LineageColumn,
        nullability: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut sources: Vec<Bound<'py, PyAny>> = Vec::with_capacity(column.upstream_columns.len());
        for source in &column.upstream_columns {
            let resource_type: Bound<'py, PyAny> =
                self.resource_type.call1((source.resource_type.as_str(),))?;
            sources.push(self.lineage_source.call1((
                resource_type,
                &source.resource_name,
                &source.column_name,
            ))?);
        }
        self.column_lineage.call1((
            &column.output_column,
            self.transform_kind
                .call1((column.transform_kind.as_str(),))?,
            self.py().None(),
            PyTuple::new(self.py(), sources)?,
            nullability,
            self.confidence.call1((column.confidence.as_str(),))?,
        ))
    }

    /// `CompiledLineageColumnFact` of one native star-expansion column.
    pub(crate) fn column_fact(&self, column: &LineageColumn) -> PyResult<Bound<'py, PyAny>> {
        let sources: Vec<Bound<'py, PyAny>> = column
            .upstream_columns
            .iter()
            .map(|source| self.source_fact(source))
            .collect::<PyResult<_>>()?;
        let fields: Bound<'py, PyDict> = PyDict::new(self.py());
        fields.set_item("output_column", &column.output_column)?;
        fields.set_item("upstream_columns", PyTuple::new(self.py(), sources)?)?;
        fields.set_item(
            "transform_kind",
            self.transform_kind
                .call1((column.transform_kind.as_str(),))?,
        )?;
        fields.set_item(
            "confidence",
            self.confidence.call1((column.confidence.as_str(),))?,
        )?;
        self.column_fact.call((), Some(&fields))
    }

    fn source_fact(&self, source: &LineageSource) -> PyResult<Bound<'py, PyAny>> {
        let fields: Bound<'py, PyDict> = PyDict::new(self.py());
        fields.set_item("resource_type", source.resource_type.as_str())?;
        fields.set_item("resource_name", &source.resource_name)?;
        fields.set_item("column_name", &source.column_name)?;
        self.source_fact.call((), Some(&fields))
    }
}

/// A structured debug event on the `sqlbuild.lineage` logger for one model.
pub(crate) fn log_lineage_event(
    py: Python<'_>,
    message: &str,
    model_name: &str,
    error: Option<&str>,
) -> PyResult<()> {
    let logger: Bound<'_, PyAny> = py
        .import(LOGGING_MODULE)?
        .getattr("getLogger")?
        .call1((LINEAGE_LOGGER,))?;
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("logger", logger)?;
    fields.set_item("message", message)?;
    fields.set_item("sqlbuild_model", model_name)?;
    fields.set_item("sqlbuild_error", error)?;
    let _ = host_entry(py, ExecutorHost::LogDebugEvent)?.call((), Some(&fields))?;
    Ok(())
}
