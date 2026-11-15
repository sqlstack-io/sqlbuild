//! Tolerant discovery of the scope-relevant authored roots, keeping each independent file fault.

use pyo3::exceptions::{PyOSError, PySyntaxError, PyUnicodeError, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::discovery::declaration_records::{
    discover_audit_files, discover_constant_files, discover_enum_files, discover_macro_files,
    discover_model_schema_files, discover_sql_function_files, discover_sql_hook_files, path_class,
};
use crate::bindings::_helpers::discovery::model_records::{
    ModelFileSelection, discovered_model_files,
};
use crate::bindings::_helpers::discovery::project_config::{
    load_local_config_file, load_project_config_file,
};
use crate::bindings::_helpers::discovery::snapshot::in_snapshot;
use crate::bindings::_helpers::discovery::sql_test_records::{
    discover_scenario_files, discover_test_files,
};
use crate::bindings::_helpers::discovery::yaml_records::discover_source_files;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
const FALLBACK_PROJECT_NAME: &str = "project";
const FALLBACK_ADAPTER: &str = "duckdb";

/// A discoverer that reports each failing file to `on_fault` and returns the rest.
type FaultingDiscoverer =
    for<'py> fn(&Bound<'py, PyAny>, Option<Bound<'py, PyAny>>) -> PyResult<Bound<'py, PyTuple>>;

/// The faults one tolerant discovery pass collects, and how it names the project in them.
struct Faults<'py> {
    project_dir: Bound<'py, PyAny>,
    shown_project_dir: String,
}

impl<'py> Faults<'py> {
    fn py(&self) -> Python<'py> {
        self.project_dir.py()
    }

    /// A fresh fault list and its `append`, the `on_fault` a discoverer reports to.
    fn collector(&self) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyAny>)> {
        let faults: Bound<'py, PyList> = PyList::empty(self.py());
        let append: Bound<'py, PyAny> = native_field(&faults, "append")?;
        Ok((faults, append))
    }

    /// `DiscoveryFileFault(path, str(error).replace(str(project_dir), "."))`.
    fn fault(&self, path: Option<&str>, error: &PyErr) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.py();
        let message: String = error
            .value(py)
            .str()?
            .to_string()
            .replace(&self.shown_project_dir, ".");
        let path: Bound<'py, PyAny> = match path {
            Some(path) => path_class(py)?.call1((path,))?,
            None => py.None().into_bound(py),
        };
        new_record_with(
            py,
            RecordId::DiscoveryFileFault,
            &[
                ("path", path),
                ("message", PyString::new(py, &message).into_any()),
            ],
        )
    }
}

/// Whether `error` is one of the input errors tolerant discovery records as a fault.
fn is_input_error(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyOSError>(py)
        || error.is_instance_of::<PyUnicodeError>(py)
        || error.is_instance_of::<PyValueError>(py)
        || error.is_instance_of::<PySyntaxError>(py)
}

/// `value`, or the fault of its input error appended to `faults` and `fallback`.
fn tolerated<'py>(
    faults: &Faults<'py>,
    collected: (&Bound<'py, PyList>, Option<&str>),
    value: PyResult<Bound<'py, PyAny>>,
    fallback: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let (list, path) = collected;
    match value {
        Ok(value) => Ok(value),
        Err(error) if is_input_error(faults.py(), &error) => {
            list.append(faults.fault(path, &error)?)?;
            fallback()
        }
        Err(error) => Err(error),
    }
}

/// `(project_config, local_config, config_faults)`, defaulting each config that fails to load.
fn configs<'py>(
    faults: &Faults<'py>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyList>)> {
    let py: Python<'py> = faults.py();
    let project_dir: &Bound<'py, PyAny> = &faults.project_dir;
    let list: Bound<'py, PyList> = PyList::empty(py);
    let project_config: Bound<'py, PyAny> = tolerated(
        faults,
        (&list, Some(PROJECT_CONFIG_FILENAME)),
        load_project_config_file(py, project_dir).map(|config| config.into_bound(py)),
        || {
            let name: Bound<'py, PyAny> = native_field(project_dir, "name")?;
            let name: Bound<'py, PyAny> = if name.is_truthy()? {
                name
            } else {
                PyString::new(py, FALLBACK_PROJECT_NAME).into_any()
            };
            new_record_with(
                py,
                RecordId::ProjectConfig,
                &[
                    ("name", name),
                    ("adapter", PyString::new(py, FALLBACK_ADAPTER).into_any()),
                ],
            )
        },
    )?;
    let local_config: Bound<'py, PyAny> = tolerated(
        faults,
        (&list, Some(LOCAL_CONFIG_FILENAME)),
        load_local_config_file(py, project_dir).map(|config| config.into_bound(py)),
        || new_record(py, RecordId::LocalConfig, (), None),
    )?;
    Ok((project_config, local_config, list))
}

/// One category's records, or no records and its input error as a path-less fault.
fn category<'py>(
    faults: &Faults<'py>,
    list: &Bound<'py, PyList>,
    discover: impl FnOnce(Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>>,
) -> PyResult<Bound<'py, PyAny>> {
    let append: Bound<'py, PyAny> = native_field(list, "append")?;
    tolerated(
        faults,
        (list, None),
        discover(append).map(Bound::into_any),
        || Ok(PyTuple::empty(faults.py()).into_any()),
    )
}

/// Records of a faulting discoverer with its own fault list, as `_discover_category` reads it.
fn own_category<'py>(
    faults: &Faults<'py>,
    discover: FaultingDiscoverer,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyList>)> {
    let (list, _) = faults.collector()?;
    let records: Bound<'py, PyAny> = category(faults, &list, |append| {
        discover(&faults.project_dir, Some(append))
    })?;
    Ok((records, list))
}

/// `tuple(chain(*lists))`.
fn chained<'py>(py: Python<'py>, lists: &[&Bound<'py, PyList>]) -> PyResult<Bound<'py, PyTuple>> {
    let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
    for list in lists {
        items.extend(list.iter());
    }
    PyTuple::new(py, items)
}

fn tolerant_discovery<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let faults = Faults {
        project_dir: project_dir.clone(),
        shown_project_dir: project_dir.str()?.to_string(),
    };
    let (project_config, local_config, config_faults) = configs(&faults)?;
    let (model_faults, append) = faults.collector()?;
    let models: Bound<'py, PyTuple> = discovered_model_files(
        project_dir,
        (false, false),
        ModelFileSelection {
            selected_model_names: None,
            on_fault: Some(append),
        },
    )?;
    let (declaration_faults, _) = faults.collector()?;
    let enums: Bound<'py, PyAny> = category(&faults, &declaration_faults, |append| {
        discover_enum_files(project_dir, Some(append), true)
    })?;
    let constants: Bound<'py, PyAny> = category(&faults, &declaration_faults, |append| {
        discover_constant_files(project_dir, Some(append), true)
    })?;
    let macros: Bound<'py, PyAny> = category(&faults, &declaration_faults, |_| {
        discover_macro_files(project_dir, true)
    })?;
    let (relationship_faults, append) = faults.collector()?;
    let tests: Bound<'py, PyTuple> = discover_test_files(project_dir, None, Some(append.clone()))?;
    let scenarios: Bound<'py, PyTuple> = discover_scenario_files(project_dir, Some(append))?;
    let (functions, function_faults) = own_category(&faults, discover_sql_function_files)?;
    let (hooks, hook_faults) = own_category(&faults, discover_sql_hook_files)?;
    let (sources, source_faults) = own_category(&faults, discover_source_files)?;
    let (audits, audit_faults) = own_category(&faults, discover_audit_files)?;
    let (model_schemas, model_schema_faults) = own_category(&faults, discover_model_schema_files)?;
    let inputs: Bound<'py, PyDict> = PyDict::new(py);
    for (field, value) in [
        ("project_config", project_config),
        ("local_config", local_config),
        ("project_dir", project_dir.clone()),
        ("model_files", models.into_any()),
        ("enum_files", enums),
        ("constant_files", constants),
        ("sql_function_files", functions),
        ("sql_hook_files", hooks),
        ("model_schema_files", model_schemas),
        ("source_files", sources),
        ("test_files", tests.into_any()),
        ("scenario_files", scenarios.into_any()),
        ("audit_files", audits),
        ("macro_files", macros),
    ] {
        inputs.set_item(field, value)?;
    }
    let discovered_inputs: Bound<'py, PyAny> =
        new_record(py, RecordId::DiscoveredProjectInputs, (), Some(&inputs))?;
    new_record_with(
        py,
        RecordId::TolerantScopeDiscovery,
        &[
            ("discovered_inputs", discovered_inputs),
            (
                "resource_faults",
                chained(
                    py,
                    &[
                        &model_faults,
                        &function_faults,
                        &hook_faults,
                        &source_faults,
                        &audit_faults,
                    ],
                )?
                .into_any(),
            ),
            (
                "declaration_faults",
                chained(py, &[&declaration_faults, &model_schema_faults])?.into_any(),
            ),
            (
                "relationship_faults",
                chained(py, &[&relationship_faults])?.into_any(),
            ),
            ("config_faults", chained(py, &[&config_faults])?.into_any()),
        ],
    )
}

/// Parse the scope-relevant authored roots while retaining independent file faults.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn discover_scope_snapshot<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    in_snapshot(project_dir, || tolerant_discovery(project_dir))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_scope_snapshot, module)?)
}
