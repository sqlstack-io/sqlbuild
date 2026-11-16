//! Add declared project sources that no selected SQL resource reads to a source map.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_views::main::project::sources::sources;
use sqlbuild_python_project::resource_views::main::source_reads::source_entry::source_entry;
use std::collections::BTreeSet;

/// Add declared project sources that no selected SQL resource reads to a source map.
pub fn with_declared_source_reads<'py>(
    project: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyDict>,
    source_names: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut missing: BTreeSet<_> = BTreeSet::new();
    for name in source_names.try_iter()? {
        let name: String = name?.extract()?;
        if !source_map.contains(&name)? {
            missing.insert(name);
        }
    }
    if missing.is_empty() {
        return Ok(source_map.clone());
    }
    let result = source_map.copy()?;
    for source in sources(project)?.try_iter()? {
        let entry = source_entry(&source?)?;
        let name: String = native_field(&entry, "name")?.extract()?;
        if missing.contains(&name) {
            result.set_item(name, entry)?;
        }
    }
    Ok(result)
}
