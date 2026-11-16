//! The relation lineage index read from a project graph, and its Python selector views.

use crate::lineage::main::lineage_key::lineage_key;
use crate::lineage::main::lineage_keys::lineage_keys;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_analysis::lineage_command::models::{LineageKey, LineageNode};
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::RecordId;

/// Python `CompiledObjectKey`s for lineage keys, one object per key.
pub(crate) struct PythonKeys<'py> {
    class: Bound<'py, PyAny>,
    made: HashMap<LineageKey, Bound<'py, PyAny>>,
}

impl<'py> PythonKeys<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            class: record_class(py, RecordId::CompiledObjectKey)?,
            made: HashMap::new(),
        })
    }

    pub(crate) fn key(&mut self, key: &LineageKey) -> PyResult<Bound<'py, PyAny>> {
        if let Some(made) = self.made.get(key) {
            return Ok(made.clone());
        }
        let made: Bound<'py, PyAny> = self
            .class
            .call1((key.resource_type.as_str(), key.name.as_str()))?;
        self.made.insert(key.clone(), made.clone());
        Ok(made)
    }

    pub(crate) fn tuple(&mut self, keys: &[LineageKey]) -> PyResult<Bound<'py, PyTuple>> {
        let items: Vec<Bound<'py, PyAny>> = keys
            .iter()
            .map(|key| self.key(key))
            .collect::<PyResult<_>>()?;
        PyTuple::new(self.class.py(), items)
    }
}

pub(crate) fn ordered_edges(
    edges: &Bound<'_, PyAny>,
) -> PyResult<Vec<(LineageKey, Vec<LineageKey>)>> {
    let mut ordered: Vec<(LineageKey, Vec<LineageKey>)> = Vec::new();
    for item in mapping_items(edges)?.try_iter()? {
        let (key, targets): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        ordered.push((lineage_key(&key)?, lineage_keys(&targets)?));
    }
    Ok(ordered)
}

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

/// A source entry's `database.schema.table`, as far as it is set.
fn source_relation_name(entry: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let part = |name: &str| -> PyResult<Option<String>> {
        match native_field(entry, name) {
            Ok(value) if !value.is_none() => Ok(Some(value.extract()?)),
            _ => Ok(None),
        }
    };
    let Some(table) = part("table")? else {
        return Ok(None);
    };
    Ok(Some(match (part("database")?, part("schema")?) {
        (Some(database), Some(schema)) => format!("{database}.{schema}.{table}"),
        (_, Some(schema)) => format!("{schema}.{table}"),
        _ => table,
    }))
}

/// Display facts of every model, seed and source, models first as Python looks them up.
pub(crate) fn project_nodes(
    project: &Bound<'_, PyAny>,
) -> PyResult<HashMap<LineageKey, LineageNode>> {
    let mut nodes: HashMap<LineageKey, LineageNode> = HashMap::new();
    let mut add = |node: LineageNode| {
        nodes.entry(node.key.clone()).or_insert(node);
    };
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        add(LineageNode {
            key: lineage_key(&native_field(&model, "key")?)?,
            relative_path: optional_text(native_field(&model, "relative_path")?)?,
            qualified_name: optional_text(
                native_field(&model, "destination")?.getattr("qualified_name")?,
            )?,
        });
    }
    for seed in native_field(project, "seeds")?.try_iter()? {
        let seed: Bound<'_, PyAny> = seed?;
        add(LineageNode {
            key: lineage_key(&native_field(&seed, "key")?)?,
            relative_path: optional_text(
                native_field(&seed, "seed_file")?.getattr("relative_path")?,
            )?,
            qualified_name: optional_text(
                native_field(&seed, "destination")?.getattr("qualified_name")?,
            )?,
        });
    }
    for source in native_field(project, "sources")?.try_iter()? {
        let source: Bound<'_, PyAny> = source?;
        add(LineageNode {
            key: lineage_key(&native_field(&source, "key")?)?,
            relative_path: optional_text(
                native_field(&source, "source_file")?.getattr("relative_path")?,
            )?,
            qualified_name: source_relation_name(&native_field(&source, "source_entry")?)?,
        });
    }
    Ok(nodes)
}
