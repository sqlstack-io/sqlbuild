//! `ProjectColumnLineage`: the project column lineage graph with lazily materialised views.

use sqlbuild_python_base::positional_record;
use std::collections::HashMap;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_analysis::lineage_graph::main::edge_index::edge_index;
use sqlbuild_analysis::lineage_graph::models::{EdgeIndex, EdgeKey};

use crate::lineage::models::LineageClasses;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::lineage::main::confidence_member::confidence_member;
use sqlbuild_python_project::lineage::main::transform_kind_member::transform_kind_member;

/// The Python objects one edge record materialises from.
pub(crate) enum EdgeParts {
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
pub(crate) struct EdgeRecords {
    pub(crate) keys: Vec<EdgeKey>,
    pub(crate) parts: Vec<EdgeParts>,
    pub(crate) index: EdgeIndex,
    pub(crate) materialised: Vec<Option<Py<PyAny>>>,
}

impl std::fmt::Debug for EdgeRecords {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EdgeRecords")
            .finish_non_exhaustive()
    }
}

impl EdgeRecords {
    pub(crate) fn new(keys: Vec<EdgeKey>, parts: Vec<EdgeParts>) -> Self {
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
    pub(crate) fn edge(
        &mut self,
        classes: &LineageClasses<'_>,
        position: usize,
    ) -> PyResult<Py<PyAny>> {
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

pub(crate) fn empty_key() -> EdgeKey {
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
pub(crate) struct RecordBuilder {
    pub(crate) keys: Vec<EdgeKey>,
    pub(crate) parts: Vec<EdgeParts>,
}

impl RecordBuilder {
    /// Records of compact facts, or of plain column facts, targeting `model_name`.
    pub(crate) fn push_facts(
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
    pub(crate) fn push_columns(
        &mut self,
        model_name: &str,
        columns: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
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
pub(crate) fn fact_edge_count(
    classes: &LineageClasses<'_>,
    columns: &Bound<'_, PyAny>,
) -> PyResult<usize> {
    if !columns.is_instance(&classes.compact_facts)? {
        return upstream_count(columns);
    }
    let mut count: usize = 0;
    for row in native_field(columns, "rows")?.try_iter()? {
        count += row?.get_item(3)?.len()?;
    }
    Ok(count)
}

pub(crate) fn upstream_count(columns: &Bound<'_, PyAny>) -> PyResult<usize> {
    let mut count: usize = 0;
    for column in columns.try_iter()? {
        count += native_field(&column?, "upstream_columns")?.len()?;
    }
    Ok(count)
}

/// `ModelColumnLineage` of compact or plain column facts.
pub(crate) fn model_from_facts<'py>(
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
