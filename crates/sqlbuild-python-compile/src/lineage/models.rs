//! Public records of lineage.

use crate::lineage::_helpers::graph::column_lineage_graph::EdgeParts;
use crate::lineage::_helpers::graph::column_lineage_graph::EdgeRecords;
use crate::lineage::_helpers::graph::column_lineage_graph::RecordBuilder;
use crate::lineage::_helpers::graph::column_lineage_graph::empty_key;
use crate::lineage::_helpers::graph::column_lineage_graph::fact_edge_count;
use crate::lineage::_helpers::graph::column_lineage_graph::model_from_facts;
use crate::lineage::_helpers::graph::column_lineage_graph::upstream_count;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyRefMut;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3::types::PyType;
use sqlbuild_analysis::lineage::models::LineageColumn;
use sqlbuild_analysis::lineage::models::LineageSource;
use sqlbuild_analysis::lineage_graph::models::EdgeIndex;
use sqlbuild_analysis::lineage_graph::models::EdgeKey;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::RecordId;
use std::collections::HashMap;

/// Where a compact model's facts are: given, or projected from its compiled model on first read.
#[derive(Debug)]
pub enum CompactColumns {
    Facts(Py<PyAny>),
    /// A compiled model whose `fast_lineage_columns` are the facts, and their upstream count.
    Model {
        model: Py<PyAny>,
        edges: usize,
    },
}

/// One compact model's facts and root-star flag.
#[derive(Debug)]
pub struct CompactModel {
    pub columns: CompactColumns,
    pub has_star: bool,
}

impl CompactModel {
    /// The model's lineage facts as Python objects.
    pub(crate) fn facts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.columns {
            CompactColumns::Facts(columns) => Ok(columns.bind(py).clone()),
            CompactColumns::Model { model, .. } => {
                native_field(model.bind(py), "fast_lineage_columns")
            }
        }
    }
}

#[pyclass(module = "sqlbuild._native", name = "ProjectColumnLineage")]
#[derive(Debug)]
pub struct ProjectColumnLineage {
    pub(crate) models: Py<PyAny>,
    pub(crate) compact: HashMap<String, CompactModel>,
    pub(crate) model_order: Vec<String>,
    pub(crate) edges: Option<Py<PyAny>>,
    pub(crate) records: Option<EdgeRecords>,
    pub(crate) edge_counts: HashMap<String, usize>,
}

#[pymethods]
impl ProjectColumnLineage {
    #[new]
    #[pyo3(signature = (*, models, edges))]
    pub(crate) fn new(models: &Bound<'_, PyAny>, edges: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut keys: Vec<EdgeKey> = Vec::new();
        let mut parts: Vec<EdgeParts> = Vec::new();
        for edge in edges.try_iter()? {
            let edge: Bound<'_, PyAny> = edge?;
            keys.push(EdgeKey {
                source_name: native_field(&edge, "source")?
                    .getattr("resource_name")?
                    .extract()?,
                target_model: native_field(&edge, "target")?
                    .getattr("resource_name")?
                    .extract()?,
                ..empty_key()
            });
            parts.push(EdgeParts::Given(edge.unbind()));
        }
        Ok(Self {
            model_order: models
                .try_iter()?
                .map(|name| name?.extract())
                .collect::<PyResult<_>>()?,
            models: models.clone().unbind(),
            compact: HashMap::new(),
            edges: Some(edges.clone().unbind()),
            records: Some(EdgeRecords::new(keys, parts)),
            edge_counts: HashMap::new(),
        })
    }

    /// Build indexed project lineage without expanding compact analysis facts.
    #[classmethod]
    #[pyo3(signature = (*, models, compact_models, model_order))]
    pub(crate) fn from_fast_facts(
        cls: &Bound<'_, PyType>,
        models: &Bound<'_, PyAny>,
        compact_models: &Bound<'_, PyDict>,
        model_order: Vec<String>,
    ) -> PyResult<Self> {
        let classes: LineageClasses<'_> = LineageClasses::new(cls.py())?;
        let mut compact: HashMap<String, CompactModel> = HashMap::new();
        for (name, entry) in compact_models.iter() {
            let (columns, has_star): (Bound<'_, PyAny>, bool) = entry.extract()?;
            let _ = compact.insert(
                name.extract()?,
                CompactModel {
                    columns: CompactColumns::Facts(columns.unbind()),
                    has_star,
                },
            );
        }
        Self::compacted(&classes, models, compact, model_order)
    }

    /// Return model lineage, expanding compact model facts on first access.
    #[getter]
    pub fn models(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyAny>> {
        let py: Python<'_> = slf.py();
        if !slf.compact.is_empty() {
            let classes: LineageClasses<'_> = LineageClasses::new(py)?;
            let models: Bound<'_, PyAny> = slf.models.bind(py).clone();
            for name in &slf.model_order {
                let Some(compact) = slf.compact.get(name) else {
                    continue;
                };
                if models.contains(name)? {
                    continue;
                }
                let lineage: Bound<'_, PyAny> =
                    model_from_facts(&classes, name, &compact.facts(py)?, compact.has_star)?;
                models.set_item(name, lineage)?;
            }
            slf.compact.clear();
        }
        Ok(slf.models.clone_ref(py))
    }

    /// Return all graph edges, expanding compact edge records on first access.
    #[getter]
    pub fn edges(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyAny>> {
        let py: Python<'_> = slf.py();
        if let Some(edges) = &slf.edges {
            return Ok(edges.clone_ref(py));
        }
        let classes: LineageClasses<'_> = LineageClasses::new(py)?;
        slf.ensure_records(&classes)?;
        let count: usize = slf.records.as_ref().map_or(0, |records| records.keys.len());
        let edges: Vec<Py<PyAny>> = slf.materialised(&classes, 0..count)?;
        let edges: Py<PyAny> = PyTuple::new(py, edges)?.into_any().unbind();
        slf.edges = Some(edges.clone_ref(py));
        Ok(edges)
    }

    /// Return whether model lineage is available without expanding its columns.
    #[pyo3(name = "has_model")]
    pub(crate) fn contains_model(
        &self,
        py: Python<'_>,
        model_name: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        if self.models.bind(py).contains(model_name)? {
            return Ok(true);
        }
        Ok(model_name
            .extract::<String>()
            .is_ok_and(|name| self.compact.contains_key(&name)))
    }

    /// Return whether model lineage retains unresolved root-star uncertainty.
    pub(crate) fn model_has_star(
        &self,
        py: Python<'_>,
        model_name: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        if let Ok(name) = model_name.extract::<String>()
            && let Some(compact) = self.compact.get(&name)
        {
            return Ok(compact.has_star);
        }
        let model: Bound<'_, PyAny> = self.models.bind(py).call_method1("get", (model_name,))?;
        if model.is_none() {
            return Ok(false);
        }
        native_field(&model, "has_star")?.extract()
    }

    /// Return a target model's direct edge count without expanding edge objects.
    pub(crate) fn edge_count_targeting(&self, model_name: &str) -> usize {
        match &self.records {
            Some(records) => records
                .index
                .by_target_model
                .get(model_name)
                .map_or(0, Vec::len),
            None => self.edge_counts.get(model_name).copied().unwrap_or(0),
        }
    }

    /// Return edges whose target model is `model_name`.
    pub(crate) fn edges_targeting<'py>(
        mut slf: PyRefMut<'py, Self>,
        model_name: &str,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let classes: LineageClasses<'_> = LineageClasses::new(slf.py())?;
        slf.ensure_records(&classes)?;
        let positions: Vec<usize> = slf.positions(|index| index.by_target_model.get(model_name));
        let edges: Vec<Py<PyAny>> = slf.materialised(&classes, positions)?;
        PyTuple::new(slf.py(), edges)
    }

    /// Return edges whose source resource is `resource_name`.
    pub(crate) fn edges_sourced_from<'py>(
        mut slf: PyRefMut<'py, Self>,
        resource_name: &str,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let classes: LineageClasses<'_> = LineageClasses::new(slf.py())?;
        slf.ensure_records(&classes)?;
        let positions: Vec<usize> =
            slf.positions(|index| index.by_source_resource.get(resource_name));
        let edges: Vec<Py<PyAny>> = slf.materialised(&classes, positions)?;
        PyTuple::new(slf.py(), edges)
    }

    /// Return direct downstream consumers of `resource_name.column_name`.
    #[pyo3(signature = (*, resource_name, column_name))]
    pub fn column_consumers<'py>(
        slf: &Bound<'py, Self>,
        resource_name: &str,
        column_name: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let sourced: Bound<'py, PyAny> =
            slf.call_method1("edges_sourced_from", (resource_name,))?;
        let mut consumers: Vec<Bound<'py, PyAny>> = Vec::new();
        for edge in sourced.try_iter()? {
            let edge: Bound<'py, PyAny> = edge?;
            if native_field(&edge, "source")?
                .getattr("column_name")?
                .eq(column_name)?
            {
                consumers.push(edge);
            }
        }
        PyTuple::new(slf.py(), consumers)
    }

    pub(crate) fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let models: Bound<'_, PyString> = native_field(slf, "models")?.repr()?;
        let edges: Bound<'_, PyString> = native_field(slf, "edges")?.repr()?;
        Ok(format!(
            "ProjectColumnLineage(models={models}, edges={edges})"
        ))
    }

    pub(crate) fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py: Python<'_> = slf.py();
        if !other.is_instance_of::<Self>() {
            return Ok(py.NotImplemented());
        }
        let equal: bool = native_field(slf, "models")?.eq(native_field(other, "models")?)?
            && native_field(slf, "edges")?.eq(native_field(other, "edges")?)?;
        Ok(pyo3::types::PyBool::new(py, equal)
            .to_owned()
            .into_any()
            .unbind())
    }
}

impl ProjectColumnLineage {
    /// Indexed project lineage of built `models` and `compact` facts, in `model_order`.
    pub fn compacted(
        classes: &LineageClasses<'_>,
        models: &Bound<'_, PyAny>,
        compact: HashMap<String, CompactModel>,
        model_order: Vec<String>,
    ) -> PyResult<Self> {
        let py: Python<'_> = models.py();
        let mut edge_counts: HashMap<String, usize> = HashMap::new();
        for name in &model_order {
            if let Some(model) = compact.get(name) {
                let count: usize = match &model.columns {
                    CompactColumns::Facts(columns) => fact_edge_count(classes, columns.bind(py))?,
                    CompactColumns::Model { edges, .. } => *edges,
                };
                let _ = edge_counts.insert(name.clone(), count);
            }
        }
        for item in mapping_items(models)?.try_iter()? {
            let (name, model): (String, Bound<'_, PyAny>) = item?.extract()?;
            let _ = edge_counts.insert(name, upstream_count(&native_field(&model, "columns")?)?);
        }
        Ok(Self {
            models: models.clone().unbind(),
            compact,
            model_order,
            edges: None,
            records: None,
            edge_counts,
        })
    }

    /// Build fast edge records from the compact facts, or the expanded models, in model order.
    pub(crate) fn ensure_records(&mut self, classes: &LineageClasses<'_>) -> PyResult<()> {
        if self.records.is_some() {
            return Ok(());
        }
        let py: Python<'_> = classes.py();
        let mut builder = RecordBuilder::default();
        let models: Bound<'_, PyAny> = self.models.bind(py).clone();
        for name in &self.model_order {
            if let Some(compact) = self.compact.get(name) {
                builder.push_facts(classes, name, &compact.facts(py)?)?;
                continue;
            }
            let model: Bound<'_, PyAny> = mapping_get(&models, name)?;
            if model.is_none() {
                continue;
            }
            builder.push_columns(name, &native_field(&model, "columns")?)?;
        }
        self.records = Some(EdgeRecords::new(builder.keys, builder.parts));
        Ok(())
    }

    /// Every edge's fields in model order, from the compact records, without building Python edges.
    pub fn edge_keys(&mut self, py: Python<'_>) -> PyResult<Vec<EdgeKey>> {
        self.ensure_records(&LineageClasses::new(py)?)?;
        Ok(self
            .records
            .as_ref()
            .map(|records| records.keys.clone())
            .unwrap_or_default())
    }

    pub(crate) fn positions(
        &self,
        select: impl Fn(&EdgeIndex) -> Option<&Vec<usize>>,
    ) -> Vec<usize> {
        self.records
            .as_ref()
            .and_then(|records| select(&records.index))
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn materialised(
        &mut self,
        classes: &LineageClasses<'_>,
        positions: impl IntoIterator<Item = usize>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let Some(records) = self.records.as_mut() else {
            return Ok(Vec::new());
        };
        positions
            .into_iter()
            .map(|position| records.edge(classes, position))
            .collect()
    }
}

/// The lineage model classes and enums, imported once per call.
#[derive(Debug)]
pub struct LineageClasses<'py> {
    pub qualified_column: Bound<'py, PyAny>,
    pub lineage_source: Bound<'py, PyAny>,
    pub lineage_edge: Bound<'py, PyAny>,
    pub column_lineage: Bound<'py, PyAny>,
    pub model_lineage: Bound<'py, PyAny>,
    pub semantic_use: Bound<'py, PyAny>,
    pub column_fact: Bound<'py, PyAny>,
    pub source_fact: Bound<'py, PyAny>,
    pub compact_facts: Bound<'py, PyAny>,
    pub transform_kind: Bound<'py, PyAny>,
    pub confidence: Bound<'py, PyAny>,
    pub nullability: Bound<'py, PyAny>,
    pub analysis_dialect: Bound<'py, PyAny>,
    pub resource_type: Bound<'py, PyAny>,
    pub model_resource: Bound<'py, PyAny>,
    pub unknown_nullability: Bound<'py, PyAny>,
}

impl<'py> LineageClasses<'py> {
    pub fn new(py: Python<'py>) -> PyResult<Self> {
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

    pub fn py(&self) -> Python<'py> {
        self.qualified_column.py()
    }

    /// `QualifiedLineageColumn(resource_type, resource_name, column_name)`.
    pub fn qualified(
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
    pub fn edge(
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
    pub fn model(
        &self,
        model_name: &str,
        columns: Vec<Bound<'py, PyAny>>,
        has_star: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.model_lineage
            .call1((model_name, PyTuple::new(self.py(), columns)?, has_star))
    }

    /// `ColumnLineage` of one native column; `nullability` is the enum member to record.
    pub fn column(
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
    pub fn column_fact(&self, column: &LineageColumn) -> PyResult<Bound<'py, PyAny>> {
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

    pub(crate) fn source_fact(&self, source: &LineageSource) -> PyResult<Bound<'py, PyAny>> {
        let fields: Bound<'py, PyDict> = PyDict::new(self.py());
        fields.set_item("resource_type", source.resource_type.as_str())?;
        fields.set_item("resource_name", &source.resource_name)?;
        fields.set_item("column_name", &source.column_name)?;
        self.source_fact.call((), Some(&fields))
    }
}
