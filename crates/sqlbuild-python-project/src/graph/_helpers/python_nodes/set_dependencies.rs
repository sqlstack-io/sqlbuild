//! The discovered Python nodes declaring `job(...)` or `tag(...)` dependencies.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_analysis::node_set_dependencies::main::unknown_tags::unknown_tags;
use sqlbuild_analysis::node_set_dependencies::models::{DependantNode, NodeSetDependency};
use sqlbuild_analysis::python_selection::models::PythonSelectionGraph;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::discovered_sets::main::record_tag_failures::record_tag_failures;

/// Discovered function collections and the node kind each holds.
const COLLECTIONS: [(&str, &str); 3] = [
    ("loader_functions", "loader"),
    ("asset_functions", "asset"),
    ("check_functions", "check"),
];

/// Each discovered Python node declaring a `job(...)` or `tag(...)` dependency.
pub(crate) fn dependants(discovered: &Bound<'_, PyAny>) -> PyResult<Vec<DependantNode>> {
    let node_set: Bound<'_, PyAny> = host_entry(discovered.py(), UserCodeHost::NodeSetRef)?;
    let mut found: Vec<DependantNode> = Vec::new();
    for (collection, kind) in COLLECTIONS {
        for function in native_field(discovered, collection)?.try_iter()? {
            let function: Bound<'_, PyAny> = function?;
            let mut sets: Vec<NodeSetDependency> = Vec::new();
            for dependency in native_field(&function, "depends_on")?.try_iter()? {
                let dependency: Bound<'_, PyAny> = dependency?;
                if dependency.is_instance(&node_set)? {
                    sets.push(NodeSetDependency {
                        kind: native_field(&native_field(&dependency, "kind")?, "value")?
                            .extract()?,
                        name: native_field(&dependency, "name")?.extract()?,
                        exclude: native_field(&dependency, "exclude")?.extract()?,
                    });
                }
            }
            if !sets.is_empty() {
                found.push(DependantNode {
                    name: native_field(&function, "name")?.extract()?,
                    kind: kind.to_owned(),
                    sets,
                });
            }
        }
    }
    Ok(found)
}

/// Record each discovered set with an item depending on a tag the `graphs` don't carry.
pub(crate) fn record_set_tag_failures(
    discovered: &Bound<'_, PyAny>,
    (graph, python): (&ProjectGraph, &PythonSelectionGraph),
    found: &[DependantNode],
) -> PyResult<()> {
    record_tag_failures(set_tag_failures(
        discovered,
        &unknown_tags(graph, python, found),
    )?);
    Ok(())
}

/// Each discovered set with an item among `unknown`, and the error naming the item and its tag.
fn set_tag_failures(
    discovered: &Bound<'_, PyAny>,
    unknown: &[(String, String)],
) -> PyResult<Vec<(String, String)>> {
    let mut failures: Vec<(String, String)> = Vec::new();
    if unknown.is_empty() {
        return Ok(failures);
    }
    for function in native_field(discovered, "asset_functions")?.try_iter()? {
        let function: Bound<'_, PyAny> = function?;
        let item: Bound<'_, PyAny> = native_field(&function, "discovered")?;
        if item.is_none() {
            continue;
        }
        let name: String = native_field(&function, "name")?.extract()?;
        for (dependant, tag) in unknown {
            if *dependant == name {
                let set: String = mapping_get(&item, "set")?.extract()?;
                let id: String = mapping_get(&item, "item")?.extract()?;
                failures.push((
                    set,
                    format!("item '{id}' depends on tag '{tag}', which no node carries"),
                ));
            }
        }
    }
    Ok(failures)
}
