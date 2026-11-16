//! Discover scenario files natively, reporting failing files to `on_fault`.

use crate::discovery::_helpers::failures::faults::FileFaults;
use crate::discovery::_helpers::project::project_files::path_object;
use crate::discovery::_helpers::records::sql_test_records::discovered_files;
use crate::discovery::_helpers::records::sql_test_records::path_class;
use crate::discovery::_helpers::records::sql_test_records::records;
use crate::discovery::_helpers::records::sql_test_records::scenario_file;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_discovery::models::FileOutcome;
use sqlbuild_discovery::sql_tests::main::discover_scenario_files::discover_scenario_files as discover_scenarios;
use sqlbuild_discovery::sql_tests::models::DiscoveredScenarioFile;
use sqlbuild_discovery::tree::models::ProjectTree;

/// Discover scenario files natively, reporting failing files to `on_fault`.
#[pyfunction]
#[pyo3(signature = (*, project_dir, on_fault=None))]
pub fn discover_scenario_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project_dir.py();
    let (tree, files) = discovered_files(project_dir, discover_scenarios)?;
    let tree: &ProjectTree = &tree.get().inner;
    let kept: Vec<(Bound<'py, PyAny>, FileOutcome<DiscoveredScenarioFile>)> = files
        .into_iter()
        .map(|file| {
            Ok((
                path_class(py)?.call1((path_object(py, tree, file.relative_path)?,))?,
                file.outcome,
            ))
        })
        .collect::<PyResult<_>>()?;
    records(
        &FileFaults {
            project_dir: project_dir.clone(),
            on_fault,
        },
        kept,
        scenario_file,
    )
}
