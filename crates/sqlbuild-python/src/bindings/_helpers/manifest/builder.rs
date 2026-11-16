//! `build_manifest`: the dbt-style manifest bytes, whose encoding error is raised on writing.

use pyo3::exceptions::{PyKeyError, PyRecursionError, PyTypeError, PyValueError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBytes, PyDict};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_analysis::manifest::errors::{ManifestEncodeError, ManifestError};
use sqlbuild_analysis::manifest::main::build_manifest_json::build_manifest_json;
use sqlbuild_analysis::manifest::models::ManifestInput;
use sqlbuild_core::json::errors::JsonEmitError;
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::graph::views::native_graph_from_edges;
use crate::bindings::_helpers::manifest::rows::{
    ProjectRows, plan_input, project_rows, python_hooks,
};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const DATETIME_MODULE: &str = "datetime";

/// A built manifest: its `manifest.json` bytes, or the error `json.dumps` raised encoding it.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeManifest {
    contents: Result<Vec<u8>, ManifestEncodeError>,
}

#[pymethods]
impl NativeManifest {
    /// The bytes `json.dumps(manifest, indent=2) + "\n"` encoded, raising what it raised.
    fn contents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        match &self.contents {
            Ok(bytes) => Ok(PyBytes::new(py, bytes)),
            Err(ManifestEncodeError::Unencodable(message)) => {
                Err(PyTypeError::new_err(message.clone()))
            }
            Err(ManifestEncodeError::Emit(JsonEmitError::NestingTooDeep)) => Err(
                PyRecursionError::new_err("maximum recursion depth exceeded"),
            ),
            Err(ManifestEncodeError::Emit(error)) => {
                Err(PyValueError::new_err(format!("{error:?}")))
            }
        }
    }
}

fn python_error(error: ManifestError, py: Python<'_>) -> PyErr {
    match error {
        ManifestError::QueryFingerprint(message) => error_class(py, ErrorId::QueryFingerprint)
            .and_then(|class| class.call1((message,)))
            .map_or_else(|error| error, PyErr::from_value),
        ManifestError::MissingParameterType(name) => PyKeyError::new_err(name),
    }
}

/// The manifest of `project` and the optional plan, over `graph` or `upstream_deps`/`downstream_deps`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_manifest(options: Option<&Bound<'_, PyDict>>) -> PyResult<Py<NativeManifest>> {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_manifest, module)?)?;
    module.add_class::<NativeManifest>()
}
