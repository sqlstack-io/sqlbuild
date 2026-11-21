//! Call one `@discover` function with a target's providers; only a location and `--discover` do.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, PyTypeMethods, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFrozenSet, PyString};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::native_host::setup_context::project_setup_context;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry as entry;
use sqlbuild_python_base::orchestration_reads::main::provider_secret_values::provider_secret_values;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::main::python_files::discover_provider_classes::discover_provider_classes;
use sqlbuild_python_front::discovery::main::python_files::provider_by_name::provider_by_name;
use sqlbuild_python_front::discovery::main::python_files::provider_usages::provider_usages;
use sqlbuild_python_front::discovery::models::SecretPolicy;

/// The result's JSON, or why the call failed with every provider secret masked.
pub(crate) fn run_discover<'py>(
    py: Python<'py>,
    (project_dir, target): (&Bound<'py, PyAny>, Option<&str>),
    (record, assets): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> Result<String, String> {
    let providers = discover_provider_classes(project_dir.clone(), SecretPolicy::Defer.name())
        .map_err(|error| reason(py, &error, &[]))?;
    let secrets: Vec<String> =
        provider_secret_values(&providers).map_err(|error| reason(py, &error, &[]))?;
    called(py, (project_dir, target), (record, assets), providers)
        .map_err(|error| reason(py, &error, &secrets))
}

fn called<'py>(
    py: Python<'py>,
    (project_dir, target): (&Bound<'py, PyAny>, Option<&str>),
    (record, assets): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    providers: Bound<'py, PyAny>,
) -> PyResult<String> {
    let function = native_field(record, "function")?;
    let by_name = provider_by_name(&providers)?;
    let target_name: Bound<'py, PyAny> = match target {
        Some(target) => PyString::new(py, target).into_any(),
        None => py.None().into_bound(py),
    };
    let setup_context = project_setup_context(
        (project_dir, &providers),
        (&target_name, &py.None().into_bound(py)),
        true,
    )?;
    let session = call_with(
        &entry(py, UserCodeHost::BuildProviderSession)?,
        &[
            ("discovered_providers", providers),
            ("setup_context", setup_context),
            ("allow_shared", PyBool::new(py, false).to_owned().into_any()),
        ],
    )?;
    within(&session, |_| {
        let arguments = PyDict::new(py);
        for usage in provider_usages(&function, &by_name)?.try_iter()? {
            let usage = usage?;
            let instance = session.call_method1("get", (usage.getattr("provider_name")?,))?;
            arguments.set_item(usage.getattr("parameter_name")?, instance)?;
        }
        let returned = function.call((), Some(&arguments))?;
        let names = PyFrozenSet::new(py, by_name.keys())?;
        call_with(
            &entry(py, UserCodeHost::DiscoveredResultJson)?,
            &[
                ("returned", returned),
                ("functions", assets.clone()),
                ("provider_names", names.into_any()),
            ],
        )?
        .extract()
    })
}

/// `error` as `Type: message`, without any of `secrets`.
fn reason(py: Python<'_>, error: &PyErr, secrets: &[String]) -> String {
    let mut text: String = format!(
        "{}: {}",
        error
            .get_type(py)
            .name()
            .map_or_else(|_| "Error".to_owned(), |name| name.to_string()),
        error.value(py)
    );
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        text = text.replace(secret.as_str(), "***");
    }
    text
}
