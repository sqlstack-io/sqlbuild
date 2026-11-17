//! Discover the (selected) SQL test files natively, reporting failing files to `on_fault`.

use crate::discovery::_helpers::failures::faults::FileFaults;
use crate::discovery::_helpers::project::project_files::path_object;
use crate::discovery::_helpers::records::sql_test_records::discovered_files;
use crate::discovery::_helpers::records::sql_test_records::path_class;
use crate::discovery::_helpers::records::sql_test_records::records;
use crate::discovery::_helpers::records::sql_test_records::test_file;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_discovery::models::FileOutcome;
use sqlbuild_discovery::sql_tests::main::discover_sql_test_files::discover_sql_test_files as discover_tests;
use sqlbuild_discovery::sql_tests::models::DiscoveredSqlTestFile;
use sqlbuild_discovery::tree::models::ProjectTree;

/// Discover the (selected) SQL test files natively, reporting failing files to `on_fault`.
#[pyfunction]
#[pyo3(signature = (*, project_dir, selected_paths=None, on_fault=None))]
pub fn discover_test_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    selected_paths: Option<Bound<'py, PyAny>>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project_dir.py();
    let (tree, files) = discovered_files(project_dir, discover_tests)?;
    let tree: &ProjectTree = &tree.get().inner;
    let mut kept: Vec<(Bound<'py, PyAny>, FileOutcome<DiscoveredSqlTestFile>)> = Vec::new();
    for file in files {
        let relative: Bound<'py, PyAny> = path_object(py, tree, file.relative_path)?.into_bound(py);
        if let Some(selected) = &selected_paths
            && !selected.contains(project_dir.div(&relative)?.call_method0("resolve")?)?
        {
            continue;
        }
        kept.push((path_class(py)?.call1((relative,))?, file.outcome));
    }
    records(
        &FileFaults {
            project_dir: project_dir.clone(),
            on_fault,
        },
        kept,
        test_file,
    )
}
