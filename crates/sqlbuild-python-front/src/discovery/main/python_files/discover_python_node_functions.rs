//! Decorated Python DAG node functions under python/.

use crate::discovery::_helpers::python_files::imported_modules::{
    ProjectDir, discovered, shim, tuple_of,
};
use crate::discovery::_helpers::python_files::imported_python_nodes::Buckets;
use crate::discovery::main::python_files::provider_by_name::provider_by_name;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_core::constants::PYTHON_NODE_ROOT;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::within_identity_pass::within_identity_pass;
use sqlbuild_python_base::records::types::RecordId;

/// Decorated Python DAG node functions under python/.
#[pyfunction]
#[pyo3(signature = (*, project_dir, providers = None))]
pub fn discover_python_node_functions<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project = ProjectDir { path: project_dir };
    let providers: Bound<'py, PyDict> = match providers {
        Some(providers) => provider_by_name(&providers)?,
        None => PyDict::new(py),
    };
    let mut buckets = Buckets {
        nodes: [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        audit_factories: Vec::new(),
    };
    let node_root: Bound<'py, PyAny> = project.child(PYTHON_NODE_ROOT)?;
    if node_root.call_method0("is_dir")?.is_truthy()? {
        let saved: Bound<'py, PyAny> = call_with(
            &shim(py, UserCodeHost::InstallPythonRootPackage)?,
            &[
                ("node_root", node_root),
                ("project_dir", project.path.clone()),
            ],
        )?;
        let discovered_nodes: PyResult<()> =
            within_identity_pass(py, || buckets.discover_nodes(&project, &providers));
        call_with(
            &shim(py, UserCodeHost::RestorePythonRootModules)?,
            &[("saved_modules", saved)],
        )?;
        discovered_nodes?;
    }
    let [loaders, assets, checks, discover_functions, for_each_assets] = buckets.nodes;
    discovered(
        py,
        RecordId::DiscoveredPythonNodeFunctions,
        &[
            ("loaders", tuple_of(py, loaders)?),
            ("assets", tuple_of(py, assets)?),
            ("checks", tuple_of(py, checks)?),
            ("discover_functions", tuple_of(py, discover_functions)?),
            ("for_each_assets", tuple_of(py, for_each_assets)?),
            (
                "audit_factories",
                tuple_of(
                    py,
                    buckets
                        .audit_factories
                        .into_iter()
                        .map(|(_, _, record)| record)
                        .collect(),
                )?,
            ),
        ],
    )
}
