//! The discovered Python nodes declaring `job(...)` or `tag(...)` dependencies.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::node_set_dependencies::models::{DependantNode, NodeSetDependency};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

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
