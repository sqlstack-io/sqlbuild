//! The asset records of each `for_each` asset: one per item of its set, or one placeholder.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use serde_json::{Value, json};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::discovered_sets::main::item_node_name::item_node_name;
use sqlbuild_orchestration::discovered_sets::main::item_roots::item_roots;
use sqlbuild_orchestration::discovered_sets::models::{
    DiscoveredItem, DiscoveredResult, ItemDependency,
};
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::json_value_to_python::json_value_to_python;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::ErrorId;

use crate::discovery::main::python_files::node_identity::node_identity;

const ASSET_IDENTITY: (&str, bool) = ("ASSET", true);

/// The project's assets with each `for_each` asset expanded from `sets`.
pub(crate) fn expanded_assets<'py>(
    inputs: &Bound<'py, PyAny>,
    sets: &DiscoveredSets,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = inputs.py();
    let mut assets: Vec<Bound<'py, PyAny>> = Vec::new();
    for asset in native_field(inputs, "asset_functions")?.try_iter()? {
        let asset: Bound<'py, PyAny> = asset?;
        if native_field(&asset, "discovered")?.is_none() {
            assets.push(asset);
        }
    }
    for template in native_field(inputs, "for_each_assets")?.try_iter()? {
        let template: Bound<'py, PyAny> = template?;
        let asset: String = native_field(&template, "name")?.extract()?;
        let set_name: String = native_field(&template, "for_each")?.extract()?;
        let Some(set) = sets.get(&set_name) else {
            return Err(new_plain_error(
                py,
                ErrorId::PythonNodeDiscovery,
                &format!(
                    "Asset '{asset}' runs for_each '{set_name}', but no @discover function is named '{set_name}'"
                ),
            ));
        };
        let names = (asset.as_str(), set_name.as_str());
        match &set.result {
            Some(result) => {
                let roots: BTreeSet<String> = item_roots(result);
                for item in &result.items {
                    let root: bool = roots.contains(&item.id);
                    assets.push(item_record(&template, names, (result, item, root))?);
                }
            }
            None => assets.push(placeholder_record(&template, names, set.error.as_deref())?),
        }
    }
    Ok(assets)
}

/// One item's asset: named `<asset>[<id>]`, waiting on its items, or on the asset's own dependencies at a root.
fn item_record<'py>(
    template: &Bound<'py, PyAny>,
    (asset, set_name): (&str, &str),
    (result, item, root): (&DiscoveredResult, &DiscoveredItem, bool),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = template.py();
    let mut depends_on: Vec<Bound<'py, PyAny>> = Vec::new();
    if root {
        for dependency in native_field(template, "depends_on")?.try_iter()? {
            depends_on.push(dependency?);
        }
    }
    let references: Bound<'py, PyAny> = call_with(
        &host_entry(py, UserCodeHost::ItemDependencyRefs)?,
        &[(
            "depends_on",
            json_value_to_python(py, &dependencies_json(asset, &item.depends_on))?,
        )],
    )?;
    for reference in references.try_iter()? {
        depends_on.push(reference?);
    }
    let discovered: Value = json!({
        "asset": asset,
        "set": set_name,
        "item": item.id,
        "args": item.args,
        "tags": result.tags,
        "max_parallel": result.max_parallel,
        "placeholder": false,
        "error": null,
    });
    named_record(
        template,
        &item_node_name(asset, Some(&item.id)),
        (PyTuple::new(py, depends_on)?.into_any(), &discovered),
    )
}

/// The one node of a set that never succeeded: `<asset>[*]`, waiting on the asset's own dependencies.
fn placeholder_record<'py>(
    template: &Bound<'py, PyAny>,
    (asset, set_name): (&str, &str),
    error: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let discovered: Value = json!({
        "asset": asset,
        "set": set_name,
        "item": null,
        "args": {},
        "tags": {},
        "max_parallel": null,
        "placeholder": true,
        "error": error,
    });
    named_record(
        template,
        &item_node_name(asset, None),
        (native_field(template, "depends_on")?, &discovered),
    )
}

/// `template` renamed `name`, with `depends_on` and `discovered`, and its identity read again.
fn named_record<'py>(
    template: &Bound<'py, PyAny>,
    name: &str,
    (depends_on, discovered): (Bound<'py, PyAny>, &Value),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = template.py();
    let record: Bound<'py, PyAny> = replaced(
        template,
        &[
            ("name", PyString::new(py, name).into_any()),
            ("depends_on", depends_on),
            ("discovered", json_value_to_python(py, discovered)?),
        ],
    )?;
    let identity: Bound<'py, PyAny> =
        node_identity(py, ASSET_IDENTITY, &|field| native_field(&record, field))?;
    replaced(&record, &[("identity", identity)])
}

/// An item's dependencies as saved JSON, its sibling items named by their nodes.
fn dependencies_json(asset: &str, dependencies: &[ItemDependency]) -> Value {
    Value::Array(
        dependencies
            .iter()
            .map(|dependency| match dependency {
                ItemDependency::Item(id) => json!({"item": item_node_name(asset, Some(id))}),
                ItemDependency::Model(_)
                | ItemDependency::Source(_)
                | ItemDependency::Seed(_)
                | ItemDependency::Job(_)
                | ItemDependency::Tag(_) => json!(dependency),
            })
            .collect(),
    )
}
