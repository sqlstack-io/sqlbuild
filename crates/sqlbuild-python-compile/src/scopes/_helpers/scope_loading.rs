//! Offline scope facts: the persistent cache, a compile-input build, or the tolerant index.

use std::path::PathBuf;

use pyo3::exceptions::{PyImportError, PyOSError, PyUnicodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyCFunction, PyDict, PyDictMethods, PyTuple};

use sqlbuild_host::entries::models::{AdapterHost, UserCodeHost};

use crate::compile_driver::main::setup::attached_inputs::attached_inputs;
use crate::compile_driver::main::setup::compile_adapter_context::compile_adapter_context;
use crate::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use crate::compile_driver::models::InputsRequest;
use crate::frontier::main::bridged::bridged;
use crate::frontier::main::cache_directory::cache_directory;
use crate::frontier::main::produced::produced;
use crate::frontier::models::FrontierStage;
use pyo3::IntoPyObjectExt;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::discovery::main::snapshots::discover_scope_snapshot::discover_scope_snapshot;
use sqlbuild_python_front::scopes::main::checks::scope_index_with_compile_usages::scope_index_with_compile_usages;
use sqlbuild_python_front::scopes::main::checks::scope_tolerant_index::scope_tolerant_index;
use sqlbuild_python_front::scopes::main::index::open_native_scope_index::open_native_scope_index;
use sqlbuild_python_project::sql_test_glue::constants::SQL_TEST_EXPECTED_MODELS_SCAN_ALGORITHM;

const SCOPE_CACHE_DIRECTORY_NAME: &str = "declaration-scopes-v2";
const SCOPE_CACHE_FILENAME: &str = "scope-index.json";

/// Where the scope index cache file of a project lives: `(directory, file name)`.
pub(crate) fn cache_location(project_dir: &Bound<'_, PyAny>) -> PyResult<(PathBuf, String)> {
    let directory: Bound<'_, PyAny> =
        cache_directory(project_dir)?.div(SCOPE_CACHE_DIRECTORY_NAME)?;
    Ok((directory.extract()?, SCOPE_CACHE_FILENAME.to_owned()))
}

/// Whether `error` is a project input error that selects the tolerant index.
pub(crate) fn is_project_input_error(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyOSError>(py)
        || error.is_instance_of::<PyUnicodeError>(py)
        || error.is_instance_of::<PyValueError>(py)
        || error.is_instance_of::<PyImportError>(py)
}

/// The discovered project inputs and the project adapter that compiles them.
pub(crate) fn project_adapter<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = project_dir.py();
    let discover_keywords: Bound<'py, PyDict> = PyDict::new(py);
    discover_keywords.set_item("project_dir", project_dir)?;
    discover_keywords.set_item("extract_output_column_locations", false)?;
    let discovered: Bound<'py, PyAny> = discover_project_inputs(py, Some(discover_keywords))?;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    let adapter: Bound<'py, PyAny> = call_with(
        &host_entry(py, AdapterHost::ResolveAdapter)?,
        &[("adapter_name", adapter_name)],
    )?;
    Ok((discovered, adapter))
}

/// The compile inputs' canonical index with usage and placement facts.
pub(crate) fn compiled_index<'py>(
    discovered: Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let adapter_context: Bound<'py, PyAny> = compile_adapter_context(&discovered, adapter, syntax)?;
    let inputs: InputsRequest = InputsRequest {
        discovered_inputs: discovered.unbind(),
        adapter_context: adapter_context.unbind(),
        selected_target: py.None(),
        cli_vars: py.None(),
        run_id: py.None(),
        no_sql_validation: PyBool::new(py, true).to_owned().into_any().unbind(),
        defer_model_sql_validation: PyBool::new(py, true).to_owned().into_any().unbind(),
        external_sql_reference_resolver: py.None(),
        resolved_connection: PyDict::new(py).into_any().unbind(),
        no_cache: PyBool::new(py, true).to_owned().into_any().unbind(),
    };
    let stage: Bound<'py, PyCFunction> = PyCFunction::new_closure(
        py,
        None,
        None,
        move |arguments: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| {
            attached_inputs(arguments.py(), &inputs).map(Bound::unbind)
        },
    )?;
    let compile_inputs: Bound<'py, PyAny> = produced(
        FrontierStage::CompileProjectInputs,
        bridged(stage.as_any())?,
    )?;
    scope_index_with_compile_usages(&compile_inputs)
}

/// The partial static index of tolerant discovery, with its faults as diagnostics.
pub(crate) fn tolerant_index<'py>(
    project_dir: &Bound<'py, PyAny>,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let snapshot: Bound<'py, PyAny> = discover_scope_snapshot(project_dir)?;
    let discovered: Bound<'py, PyAny> = native_field(&snapshot, "discovered_inputs")?;
    let inventory: Bound<'py, PyAny> = call_with(
        &host_entry(py, UserCodeHost::InventoryProjectMacros)?,
        &[("macro_files", native_field(&discovered, "macro_files")?)],
    )?;
    let native: Bound<'py, PyAny> = Bound::new(
        py,
        open_native_scope_index(
            py,
            &discovered,
            &PyDict::new(py).into_any(),
            Some(native_field(&inventory, "exports")?),
        )?,
    )?
    .into_any();
    let scan_store: Bound<'py, PyTuple> = PyTuple::new(
        py,
        [
            py.None().into_bound(py),
            pyo3::types::PyString::new(py, SQL_TEST_EXPECTED_MODELS_SCAN_ALGORITHM).into_any(),
        ],
    )?;
    let relationship_faults: Bound<'py, PyAny> = call_with(
        &native_field(&native, "grant_relationships")?,
        &[
            ("discovered_inputs", discovered),
            ("syntax", syntax.clone()),
            ("scan_store", scan_store.into_any()),
        ],
    )?;
    Ok(scope_tolerant_index(
        py,
        &native.call_method0("index_with_relationships")?,
        &snapshot,
        (native_field(&inventory, "faults")?, relationship_faults),
    )?
    .into_bound(py))
}
