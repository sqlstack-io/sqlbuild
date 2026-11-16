//! `LineageGraphViews` of an index, for the planner's selector functions.

use crate::lineage::_helpers::lineage::lineage_index::PythonKeys;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet};
use sqlbuild_analysis::lineage_command::models::{LineageIndex, LineageKey};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// `LineageGraphViews` of an index, for the planner's selector functions.
pub fn index_views<'py>(py: Python<'py>, index: &LineageIndex) -> PyResult<Bound<'py, PyAny>> {
    let mut keys: PythonKeys<'py> = PythonKeys::new(py)?;
    let edges = |pairs: &[(LineageKey, Vec<LineageKey>)],
                 keys: &mut PythonKeys<'py>|
     -> PyResult<Bound<'py, PyDict>> {
        let dict: Bound<'py, PyDict> = PyDict::new(py);
        for (key, targets) in pairs {
            dict.set_item(keys.key(key)?, keys.tuple(targets)?)?;
        }
        Ok(dict)
    };
    let upstream: Bound<'py, PyDict> = edges(&index.upstream, &mut keys)?;
    let downstream: Bound<'py, PyDict> = edges(&index.downstream, &mut keys)?;
    let tags: Bound<'py, PyDict> = PyDict::new(py);
    for (tag, tagged) in &index.tags {
        let items: Vec<Bound<'py, PyAny>> = tagged
            .iter()
            .map(|key| keys.key(key))
            .collect::<PyResult<_>>()?;
        tags.set_item(tag, PyFrozenSet::new(py, items)?)?;
    }
    let paths: Bound<'py, PyDict> = PyDict::new(py);
    for (key, folder) in &index.paths {
        paths.set_item(keys.key(key)?, folder)?;
    }
    let names: Bound<'py, PyDict> = PyDict::new(py);
    for (name, key) in &index.all_keys {
        names.set_item(name, keys.key(key)?)?;
    }
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("upstream_deps", upstream)?;
    options.set_item("downstream_deps", downstream)?;
    options.set_item("tag_index", tags)?;
    options.set_item("path_index", paths)?;
    options.set_item("all_keys", names)?;
    new_record(py, RecordId::LineageGraphViews, (), Some(&options))
}
