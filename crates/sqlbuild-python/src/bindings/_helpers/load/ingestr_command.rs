//! The ingestr command line for one declarative ingestr source, built from its runtime values.

use sqlbuild_python_base::native_host::main::host_error::host_error;
use std::collections::BTreeMap;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::ingestr_command::main::format_ingestr_command::format_ingestr_command as native_format_ingestr_command;
use sqlbuild_core::ingestr_command::main::ingestr_command_arguments::ingestr_command_arguments;
use sqlbuild_core::ingestr_command::main::ingestr_destination_table::ingestr_destination_table;
use sqlbuild_core::ingestr_command::main::ingestr_destination_uri::ingestr_destination_uri;
use sqlbuild_core::ingestr_command::models::{IngestrDestination, IngestrSource};
use sqlbuild_core::loader_connection::models::LoaderConnection;
use sqlbuild_host::entries::models::ThirdPartyHost;
use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::bindings::_helpers::model_config::template_data::expanded_template_data;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const INGESTR_ERROR: ThirdPartyHost = ThirdPartyHost::IngestrIntegrationError;
const INGESTR_SOURCE_CONFIG: ThirdPartyHost = ThirdPartyHost::IngestrSourceConfig;
const INGESTR_KIND: ThirdPartyHost = ThirdPartyHost::IngestrIntegrationKind;
const TEMPLATE_LABEL: &str = "ingestr source config";
const TEMPLATE_FIELDS: [&str; 7] = [
    "source_uri",
    "source_table",
    "strategy",
    "incremental_key",
    "primary_key",
    "columns",
    "extra_args",
];
const LIST_FIELDS: [&str; 2] = ["primary_key", "extra_args"];

/// An `IngestrIntegrationError`, or the failure to construct one.
fn ingestr_error(py: Python<'_>, message: String) -> PyErr {
    host_error(py, INGESTR_ERROR, &message)
}

/// The source's ingestr config, raising when the source declares no valid one.
fn ingestr_config<'py>(source_entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source_entry.py();
    let name: String = native_field(source_entry, "name")?.str()?.extract()?;
    let loader: Bound<'py, PyAny> = native_field(source_entry, "integration_loader")?;
    if loader.is_none() || !native_field(&loader, "kind")?.eq(host_entry(py, INGESTR_KIND)?)? {
        return Err(ingestr_error(
            py,
            format!("Source '{name}' does not define ingestr config"),
        ));
    }
    let config: Bound<'py, PyAny> = native_field(&loader, "config")?;
    if !config.is_instance(&host_entry(py, INGESTR_SOURCE_CONFIG)?)? {
        return Err(ingestr_error(
            py,
            format!("Source '{name}' defines invalid ingestr config"),
        ));
    }
    Ok(config)
}

/// `str(value)`, or `None` when the value is `None` or renders empty.
fn optional_text(value: Option<Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let text: String = value.str()?.extract()?;
    Ok((!text.is_empty()).then_some(text))
}

/// One string per item of a list value, or the value itself as one string.
fn text_list(value: Option<Bound<'_, PyAny>>) -> PyResult<Vec<String>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(Vec::new());
    };
    if value.is_instance_of::<PyString>() {
        return Ok(vec![value.extract()?]);
    }
    match value.try_iter() {
        Ok(items) => items
            .map(|item| item?.str()?.extract())
            .collect::<PyResult<Vec<String>>>(),
        Err(_) => Ok(vec![value.str()?.extract()?]),
    }
}

/// The source config with `${...}` templates expanded against vars and run context.
fn resolved_source(
    config: &Bound<'_, PyAny>,
    vars: &Bound<'_, PyDict>,
    (environment, run_id): (Option<&str>, &str),
) -> PyResult<IngestrSource> {
    let py: Python<'_> = config.py();
    let value: Bound<'_, PyDict> = PyDict::new(py);
    for field in TEMPLATE_FIELDS {
        let raw: Bound<'_, PyAny> = native_field(config, field)?;
        if LIST_FIELDS.contains(&field) {
            value.set_item(
                field,
                PyList::new(py, raw.try_iter()?.collect::<PyResult<Vec<_>>>()?)?,
            )?;
        } else {
            value.set_item(field, raw)?;
        }
    }
    let context: Bound<'_, PyDict> = PyDict::new(py);
    context.set_item("environment", environment)?;
    context.set_item("run_id", run_id)?;
    let expanded: Bound<'_, PyAny> = expanded_template_data(
        value.as_any(),
        (vars, &context),
        (
            TemplateOptions {
                allow_context: true,
                preserve_context_tokens: false,
                preserve_unknown_context: true,
            },
            TEMPLATE_LABEL,
        ),
    )?;
    let expanded: &Bound<'_, PyDict> = expanded.downcast()?;
    let get = |key: &str| expanded.get_item(key);
    Ok(IngestrSource {
        source_uri: expanded.as_any().get_item("source_uri")?.str()?.extract()?,
        source_table: expanded
            .as_any()
            .get_item("source_table")?
            .str()?
            .extract()?,
        strategy: optional_text(get("strategy")?)?,
        incremental_key: optional_text(get("incremental_key")?)?,
        primary_key: text_list(get("primary_key")?)?,
        columns: optional_text(get("columns")?)?,
        extra_args: text_list(get("extra_args")?)?,
    })
}

/// The string settings of a connection and its rendered port.
pub(crate) fn connection(connection_config: &Bound<'_, PyDict>) -> PyResult<LoaderConnection> {
    let mut strings: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in connection_config.iter() {
        if let (Ok(key), Ok(value)) = (key.extract::<String>(), value.downcast::<PyString>()) {
            strings.insert(key, value.extract()?);
        }
    }
    let port: Option<String> = connection_config
        .get_item("port")?
        .map(|port| port.str()?.extract())
        .transpose()?;
    Ok(LoaderConnection { strings, port })
}

/// The ingestr CLI command for one source entry.
#[pyfunction]
#[pyo3(signature = (**request))]
fn build_ingestr_command<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request: Request<'_, 'py> = Request::new("build_ingestr_command", request)?;
    let py: Python<'py> = request.py();
    let source_entry: Bound<'py, PyAny> = request.get("source_entry")?;
    let config: Bound<'py, PyAny> = ingestr_config(&source_entry)?;
    let environment: Option<String> = request.get("environment")?.extract()?;
    let run_id: String = request.get("run_id")?.extract()?;
    let source: IngestrSource = resolved_source(
        &config,
        request.get("vars")?.downcast()?,
        (environment.as_deref(), &run_id),
    )?;
    let adapter_name: String = request.get("adapter_name")?.extract()?;
    let uri: String = ingestr_destination_uri(
        &adapter_name,
        &connection(request.get("connection_config")?.downcast()?)?,
    )
    .map_err(|error| ingestr_error(py, error.message))?;
    let schema: Option<String> = native_field(&source_entry, "schema")?.extract()?;
    let destination_name: String = request.get("destination_name")?.extract()?;
    let destination: IngestrDestination = IngestrDestination {
        uri,
        table: ingestr_destination_table(schema.as_deref(), &destination_name),
        is_reload: request.get("is_reload")?.is_truthy()?,
    };
    PyTuple::new(py, ingestr_command_arguments(&source, &destination))
}

/// The display-safe, shell-quoted form of an ingestr command.
#[pyfunction]
#[pyo3(signature = (command, /))]
fn format_ingestr_command(command: Vec<String>) -> String {
    native_format_ingestr_command(&command)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_ingestr_command, module)?)?;
    module.add_function(wrap_pyfunction!(format_ingestr_command, module)?)?;
    Ok(())
}
