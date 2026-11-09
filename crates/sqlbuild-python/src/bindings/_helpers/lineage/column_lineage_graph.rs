//! `ProjectColumnLineage`: the project column lineage graph with lazily materialised views.

use std::collections::HashMap;

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRefMut, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple, PyType};
use pyo3::{pyclass, pymethods};
use sqlbuild_analysis::lineage_graph::main::edge_index::edge_index;
use sqlbuild_analysis::lineage_graph::models::{EdgeIndex, EdgeKey};

use crate::bindings::_helpers::lineage::lineage_facts::{confidence_member, transform_kind_member};
use crate::bindings::_helpers::lineage::lineage_objects::LineageClasses;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;

/// Where a compact model's facts are: given, or projected from its compiled model on first read.
pub(crate) enum CompactColumns {
    Facts(Py<PyAny>),
    /// A compiled model whose `fast_lineage_columns` are the facts, and their upstream count.
    Model {
        model: Py<PyAny>,
        edges: usize,
    },
}

/// One compact model's facts and root-star flag.
pub(crate) struct CompactModel {
    pub(crate) columns: CompactColumns,
    pub(crate) has_star: bool,
}

impl CompactModel {
    /// The model's lineage facts as Python objects.
    fn facts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.columns {
            CompactColumns::Facts(columns) => Ok(columns.bind(py).clone()),
            CompactColumns::Model { model, .. } => {
                native_field(model.bind(py), "fast_lineage_columns")
            }
        }
    }
}

/// The Python objects one edge record materialises from.
enum EdgeParts {
    /// An edge given to the eager constructor, returned as is.
    Given(Py<PyAny>),
    /// Fact values: source resource type, transform kind and confidence objects.
    Facts {
        resource_type: Py<PyAny>,
        transform: Py<PyAny>,
        confidence: Py<PyAny>,
    },
}

positional_record! {
    extract;
    /// One compact lineage row: output name index, transform and confidence codes, sources.
    struct CompactColumnRow<'py> {
        name_index: Bound<'py, PyAny>,
        transform_code: i64,
        confidence_code: i64,
        sources: Bound<'py, PyAny>,
    }
}

/// Indexed edge records, materialised edges shared between equal records.
struct EdgeRecords {
    keys: Vec<EdgeKey>,
    parts: Vec<EdgeParts>,
    index: EdgeIndex,
    materialised: Vec<Option<Py<PyAny>>>,
}

#[pyclass(module = "sqlbuild._native", name = "ProjectColumnLineage")]
pub(crate) struct ProjectColumnLineage {
    models: Py<PyAny>,
    compact: HashMap<String, CompactModel>,
    model_order: Vec<String>,
    edges: Option<Py<PyAny>>,
    records: Option<EdgeRecords>,
    edge_counts: HashMap<String, usize>,
}

#[pymethods]
impl ProjectColumnLineage {
    #[new]
    #[pyo3(signature = (*, models, edges))]
    fn new(models: &Bound<'_, PyAny>, edges: &Bound<'_, PyAny>) -> PyResult<Self> {
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
    fn from_fast_facts(
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
    pub(crate) fn models(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyAny>> {
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
    pub(crate) fn edges(mut slf: PyRefMut<'_, Self>) -> PyResult<Py<PyAny>> {
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
    fn contains_model(&self, py: Python<'_>, model_name: &Bound<'_, PyAny>) -> PyResult<bool> {
        if self.models.bind(py).contains(model_name)? {
            return Ok(true);
        }
        Ok(model_name
            .extract::<String>()
            .is_ok_and(|name| self.compact.contains_key(&name)))
    }

    /// Return whether model lineage retains unresolved root-star uncertainty.
    fn model_has_star(&self, py: Python<'_>, model_name: &Bound<'_, PyAny>) -> PyResult<bool> {
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
    fn edge_count_targeting(&self, model_name: &str) -> usize {
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
    fn edges_targeting<'py>(
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
    fn edges_sourced_from<'py>(
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
    pub(crate) fn column_consumers<'py>(
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

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let models: Bound<'_, PyString> = native_field(slf, "models")?.repr()?;
        let edges: Bound<'_, PyString> = native_field(slf, "edges")?.repr()?;
        Ok(format!(
            "ProjectColumnLineage(models={models}, edges={edges})"
        ))
    }

    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
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
    pub(crate) fn compacted(
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
    fn ensure_records(&mut self, classes: &LineageClasses<'_>) -> PyResult<()> {
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
    pub(crate) fn edge_keys(&mut self, py: Python<'_>) -> PyResult<Vec<EdgeKey>> {
        self.ensure_records(&LineageClasses::new(py)?)?;
        Ok(self
            .records
            .as_ref()
            .map(|records| records.keys.clone())
            .unwrap_or_default())
    }

    fn positions(&self, select: impl Fn(&EdgeIndex) -> Option<&Vec<usize>>) -> Vec<usize> {
        self.records
            .as_ref()
            .and_then(|records| select(&records.index))
            .cloned()
            .unwrap_or_default()
    }

    fn materialised(
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

impl EdgeRecords {
    fn new(keys: Vec<EdgeKey>, parts: Vec<EdgeParts>) -> Self {
        let index: EdgeIndex = edge_index(&keys);
        let materialised: Vec<Option<Py<PyAny>>> = keys.iter().map(|_| None).collect();
        Self {
            keys,
            parts,
            index,
            materialised,
        }
    }

    /// The edge at `position`, built once per distinct fact edge.
    fn edge(&mut self, classes: &LineageClasses<'_>, position: usize) -> PyResult<Py<PyAny>> {
        let py: Python<'_> = classes.py();
        let (resource_type, transform, confidence) = match &self.parts[position] {
            EdgeParts::Given(edge) => return Ok(edge.clone_ref(py)),
            EdgeParts::Facts {
                resource_type,
                transform,
                confidence,
            } => (resource_type, transform, confidence),
        };
        let first: usize = self.index.first_equal[position];
        if let Some(edge) = &self.materialised[first] {
            return Ok(edge.clone_ref(py));
        }
        let key: &EdgeKey = &self.keys[position];
        let edge: Py<PyAny> = classes
            .edge(
                classes.qualified(resource_type.bind(py), &key.source_name, &key.source_column)?,
                &classes.qualified(
                    &classes.model_resource,
                    &key.target_model,
                    &key.target_column,
                )?,
                transform.bind(py),
                confidence.bind(py),
            )?
            .unbind();
        self.materialised[first] = Some(edge.clone_ref(py));
        Ok(edge)
    }
}

fn empty_key() -> EdgeKey {
    EdgeKey {
        source_type: String::new(),
        source_name: String::new(),
        source_column: String::new(),
        target_model: String::new(),
        target_column: String::new(),
        transform: String::new(),
        confidence: String::new(),
    }
}

fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.str()?.extract()
}

/// Edge records accumulated in model order.
#[derive(Default)]
struct RecordBuilder {
    keys: Vec<EdgeKey>,
    parts: Vec<EdgeParts>,
}

impl RecordBuilder {
    /// Records of compact facts, or of plain column facts, targeting `model_name`.
    fn push_facts(
        &mut self,
        classes: &LineageClasses<'_>,
        model_name: &str,
        columns: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if !columns.is_instance(&classes.compact_facts)? {
            return self.push_columns(model_name, columns);
        }
        let py: Python<'_> = columns.py();
        let pool: Bound<'_, PyAny> = native_field(columns, "string_pool")?;
        let remapped: bool = native_field(columns, "resource_names")?.is_truthy()?
            || native_field(columns, "resource_name_indexes")?.is_truthy()?;
        let mut members = CodeMembers::default();
        for row in native_field(columns, "rows")?.try_iter()? {
            let CompactColumnRow {
                name_index,
                transform_code,
                confidence_code,
                sources,
            }: CompactColumnRow<'_> = row?.extract()?;
            let output_column: String = pool.get_item(name_index)?.extract()?;
            for source in sources.try_iter()? {
                let (kind, name, column): (Bound<'_, PyAny>, Bound<'_, PyAny>, Bound<'_, PyAny>) =
                    source?.extract()?;
                let resource_type: Bound<'_, PyAny> = pool.get_item(kind)?;
                let resource_name: String = if remapped {
                    columns.call_method1("resource_name", (name,))?.extract()?
                } else {
                    pool.get_item(name)?.extract()?
                };
                let transform: Py<PyAny> = members.transform(columns, transform_code)?;
                let confidence: Py<PyAny> = members.confidence(columns, confidence_code)?;
                self.keys.push(EdgeKey {
                    source_type: text(&resource_type)?,
                    source_name: resource_name,
                    source_column: pool.get_item(column)?.extract()?,
                    target_model: model_name.to_owned(),
                    target_column: output_column.clone(),
                    transform: text(transform.bind(py))?,
                    confidence: text(confidence.bind(py))?,
                });
                self.parts.push(EdgeParts::Facts {
                    resource_type: resource_type.unbind(),
                    transform,
                    confidence,
                });
            }
        }
        Ok(())
    }

    /// Records of columns with `upstream_columns`, `transform_kind` and `confidence`.
    fn push_columns(&mut self, model_name: &str, columns: &Bound<'_, PyAny>) -> PyResult<()> {
        for column in columns.try_iter()? {
            let column: Bound<'_, PyAny> = column?;
            let output_column: String = native_field(&column, "output_column")?.extract()?;
            let transform: Bound<'_, PyAny> = native_field(&column, "transform_kind")?;
            let confidence: Bound<'_, PyAny> = native_field(&column, "confidence")?;
            for source in native_field(&column, "upstream_columns")?.try_iter()? {
                let source: Bound<'_, PyAny> = source?;
                let resource_type: Bound<'_, PyAny> = native_field(&source, "resource_type")?;
                self.keys.push(EdgeKey {
                    source_type: text(&resource_type)?,
                    source_name: native_field(&source, "resource_name")?.extract()?,
                    source_column: native_field(&source, "column_name")?.extract()?,
                    target_model: model_name.to_owned(),
                    target_column: output_column.clone(),
                    transform: text(&transform)?,
                    confidence: text(&confidence)?,
                });
                self.parts.push(EdgeParts::Facts {
                    resource_type: resource_type.unbind(),
                    transform: transform.clone().unbind(),
                    confidence: confidence.clone().unbind(),
                });
            }
        }
        Ok(())
    }
}

/// Looks up the enum member of one native code.
type CodeLookup = fn(Python<'_>, usize) -> PyResult<Py<PyAny>>;

/// The transform kind and confidence members of native codes, looked up once per code.
#[derive(Default)]
struct CodeMembers {
    members: HashMap<(&'static str, i64), Py<PyAny>>,
}

impl CodeMembers {
    fn transform(&mut self, facts: &Bound<'_, PyAny>, code: i64) -> PyResult<Py<PyAny>> {
        self.member(facts.py(), ("transform_kind", transform_kind_member), code)
    }

    fn confidence(&mut self, facts: &Bound<'_, PyAny>, code: i64) -> PyResult<Py<PyAny>> {
        self.member(facts.py(), ("confidence", confidence_member), code)
    }

    fn member(
        &mut self,
        py: Python<'_>,
        (kind, lookup): (&'static str, CodeLookup),
        code: i64,
    ) -> PyResult<Py<PyAny>> {
        if let Some(member) = self.members.get(&(kind, code)) {
            return Ok(member.clone_ref(py));
        }
        let member: Py<PyAny> = lookup(py, usize::try_from(code)?)?;
        let _ = self.members.insert((kind, code), member.clone_ref(py));
        Ok(member)
    }
}

/// The edge count of compact facts or plain column facts.
fn fact_edge_count(classes: &LineageClasses<'_>, columns: &Bound<'_, PyAny>) -> PyResult<usize> {
    if !columns.is_instance(&classes.compact_facts)? {
        return upstream_count(columns);
    }
    let mut count: usize = 0;
    for row in native_field(columns, "rows")?.try_iter()? {
        count += row?.get_item(3)?.len()?;
    }
    Ok(count)
}

fn upstream_count(columns: &Bound<'_, PyAny>) -> PyResult<usize> {
    let mut count: usize = 0;
    for column in columns.try_iter()? {
        count += native_field(&column?, "upstream_columns")?.len()?;
    }
    Ok(count)
}

/// `ModelColumnLineage` of compact or plain column facts.
fn model_from_facts<'py>(
    classes: &LineageClasses<'py>,
    model_name: &str,
    columns: &Bound<'py, PyAny>,
    has_star: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    let mut lineage_columns: Vec<Bound<'py, PyAny>> = Vec::new();
    for column in columns.try_iter()? {
        let column: Bound<'py, PyAny> = column?;
        let mut sources: Vec<Bound<'py, PyAny>> = Vec::new();
        for source in native_field(&column, "upstream_columns")?.try_iter()? {
            let source: Bound<'py, PyAny> = source?;
            sources.push(classes.lineage_source.call1((
                native_field(&source, "resource_type")?,
                native_field(&source, "resource_name")?,
                native_field(&source, "column_name")?,
            ))?);
        }
        lineage_columns.push(classes.column_lineage.call1((
            native_field(&column, "output_column")?,
            native_field(&column, "transform_kind")?,
            py.None(),
            PyTuple::new(py, sources)?,
            &classes.unknown_nullability,
            native_field(&column, "confidence")?,
        ))?);
    }
    classes.model(model_name, lineage_columns, has_star)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ProjectColumnLineage>()
}
