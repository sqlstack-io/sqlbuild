//! The manifest of `project` and the optional plan, over `graph` or `upstream_deps`/`downstream_deps`.

use crate::manifest::_helpers::builder::DATETIME_MODULE;
use crate::manifest::_helpers::builder::python_error;
use crate::manifest::_helpers::rows::ProjectRows;
use crate::manifest::_helpers::rows::plan_input;
use crate::manifest::_helpers::rows::project_rows;
use crate::manifest::_helpers::rows::python_hooks;
use crate::manifest::models::NativeManifest;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_analysis::manifest::main::build_manifest_json::build_manifest_json;
use sqlbuild_analysis::manifest::models::ManifestInput;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::graph::main::project::native_graph_from_edges::native_graph_from_edges;
use sqlbuild_python_project::graph::models::NativeProjectGraph;

/// The manifest of `project` and the optional plan, over `graph` or `upstream_deps`/`downstream_deps`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_manifest(options: Option<&Bound<'_, PyDict>>) -> PyResult<Py<NativeManifest>> {
    let request = Request::new("build_manifest", options)?;
    let py: Python<'_> = request.py();
    let project: Bound<'_, PyAny> = request.get("project")?;
    let plan_output: Bound<'_, PyAny> = request
        .optional("plan_output")?
        .unwrap_or_else(|| py.None().into_bound(py));
    let project_name: String = request.get("project_name")?.extract()?;
    let adapter_type: String = request.get("adapter_type")?.extract()?;
    let graph: Bound<'_, NativeProjectGraph> = match request.optional("graph")? {
        Some(graph) if !graph.is_none() => graph.cast_into()?,
        _ => Bound::new(
            py,
            native_graph_from_edges(
                &request.get("upstream_deps")?,
                &request.get("downstream_deps")?,
            )?,
        )?,
    };
    let utc: Bound<'_, PyAny> = py.import(DATETIME_MODULE)?.getattr("UTC")?;
    let generated_at: String = py
        .import(DATETIME_MODULE)?
        .getattr("datetime")?
        .call_method1("now", (utc,))?
        .call_method0("isoformat")?
        .extract()?;
    let hook_identities: Bound<'_, PyAny> = host_entry(py, UserCodeHost::BuildHookIdentities)?
        .call1((native_field(&project, "hook_functions")?,))?;
    compiler_guard(|| {
        let python_hooks = python_hooks(&project, &hook_identities)?;
        let ProjectRows {
            run_id,
            sql_analysis_dialect,
            models,
            sources,
            seeds,
            macros,
            macro_declarations,
            sql_hooks,
        } = project_rows(&project)?;
        let input: ManifestInput = ManifestInput {
            project_name,
            adapter_type,
            run_id,
            generated_at,
            sql_analysis_dialect,
            models,
            sources,
            seeds,
            macros,
            macro_declarations,
            sql_hooks,
            python_hooks,
            plan: plan_input(&plan_output)?,
        };
        let graph = graph.borrow();
        let graph: &ProjectGraph = graph.graph();
        let contents = py
            .detach(|| build_manifest_json(&input, graph))
            .map_err(|error| python_error(error, py))?
            .map(String::into_bytes);
        Py::new(py, NativeManifest { contents })
    })
}
