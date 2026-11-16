//! Select source entries from native reachability; the resulting entries cross the executor edge.

use crate::compiled_views::main::project::sources::sources;
use crate::graph::main::nodes::native_key::native_key;
use crate::planner_scope::_helpers::loaders::intermediate_map;
use crate::resource_views::main::source_reads::source_entry::source_entry;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::reachable_keys::reachable_keys;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Select source entries from native reachability; the resulting entries cross the executor edge.
pub fn source_load_map<'py>(
    project: &Bound<'py, PyAny>,
    selected: &[ObjectKey],
    upstream_deps: Option<&[(ObjectKey, Vec<ObjectKey>)]>,
) -> PyResult<Bound<'py, PyDict>> {
    let relevant = upstream_deps.map(|edges| reachable_keys(selected, edges));
    let result = PyDict::new(project.py());
    for source in sources(project)?.try_iter()? {
        let source = source?;
        if let Some(keys) = &relevant
            && !keys.contains(&native_key(
                &crate::resource_views::main::source_reads::key::key(&source)?,
            )?)
        {
            continue;
        }
        let entry = source_entry(&source)?;
        result.set_item(native_field(&entry, "name")?, entry)?;
    }
    for upstream in [false, true] {
        for (key, entry) in intermediate_map(project, selected, upstream)?.iter() {
            result.set_item(key, entry)?;
        }
    }
    Ok(result)
}
