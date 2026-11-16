//! The dlt destination factory, arguments and dataset for one SQLBuild warehouse adapter.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::dlt_destination::constants::DEFAULT_DESTINATION_OPTIONS;
use sqlbuild_core::dlt_destination::main::dlt_destination_plan::dlt_destination_plan;
use sqlbuild_core::dlt_destination::models::{
    DltDataset, DltDatasetName, DltDestinationPlan, DltDestinationRequest, DltValue,
};
use sqlbuild_core::loader_connection::models::LoaderConnection;
use sqlbuild_host::entries::models::ThirdPartyHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;
use sqlbuild_python_base::positional_record;

use crate::bindings::_helpers::load::ingestr_command::connection;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

const DLT_ERROR: ThirdPartyHost = ThirdPartyHost::DltIntegrationError;

positional_record! {
    into;
    /// `(factory, arguments, options, dataset_name)` of one dlt destination.
    struct DestinationCall<'py> {
        factory: &'static str,
        arguments: Bound<'py, PyDict>,
        options: Bound<'py, PyDict>,
        dataset: Bound<'py, PyAny>,
    }
}

/// A `DltIntegrationError`, or the failure to construct one.
fn dlt_error(py: Python<'_>, message: String) -> PyErr {
    host_error(py, DLT_ERROR, &message)
}

/// The text of a string value, or `None` for any other value.
fn string(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_instance_of::<PyString>() {
        return Ok(Some(value.extract()?));
    }
    Ok(None)
}

/// One factory argument as a Python value.
fn argument<'py>(
    value: &DltValue,
    connection_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = connection_config.py();
    match value {
        DltValue::Text(text) => Ok(PyString::new(py, text).into_any()),
        DltValue::Port { default } => mapping_get_or(connection_config, "port", *default),
        DltValue::Object(fields) => Ok(arguments(fields, connection_config)?.into_any()),
    }
}

fn arguments<'py>(
    fields: &[(&str, DltValue)],
    connection_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let values: Bound<'py, PyDict> = PyDict::new(connection_config.py());
    for (key, value) in fields {
        values.set_item(*key, argument(value, connection_config)?)?;
    }
    Ok(values)
}

/// `(factory, arguments, options, dataset_name)`; callers unpack `arguments` and `options` apart.
#[pyfunction]
#[pyo3(signature = (*, adapter_name, connection_config, destination_config, dataset_name))]
fn dlt_destination_arguments<'py>(
    adapter_name: &str,
    connection_config: &Bound<'py, PyAny>,
    destination_config: &Bound<'py, PyAny>,
    dataset_name: &Bound<'py, PyAny>,
) -> PyResult<DestinationCall<'py>> {
    let py: Python<'py> = connection_config.py();
    let dict: Bound<'py, PyAny> = imported(py, "builtins", "dict")?;
    let destination: Bound<'py, PyDict> = if destination_config.is_truthy()? {
        dict.call1((destination_config,))?.downcast_into()?
    } else {
        PyDict::new(py)
    };
    let mut destination_keys: Vec<Option<String>> = Vec::new();
    for key in destination.keys() {
        destination_keys.push(string(&key)?);
    }
    let settings: Bound<'py, PyDict> = dict.call1((connection_config,))?.downcast_into()?;
    let name = DltDatasetName {
        text: string(dataset_name)?,
        truthy: dataset_name.is_truthy()?,
    };
    let connection: LoaderConnection = connection(&settings)?;
    let request = DltDestinationRequest {
        adapter: adapter_name,
        connection: &connection,
        destination_keys: &destination_keys,
        dataset_name: &name,
    };
    let plan: DltDestinationPlan =
        dlt_destination_plan(&request).map_err(|error| dlt_error(py, error.message))?;
    let options: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in DEFAULT_DESTINATION_OPTIONS {
        options.set_item(key, value)?;
    }
    options.update(destination.as_mapping())?;
    let dataset: Bound<'py, PyAny> = match plan.dataset {
        DltDataset::Given => dataset_name.clone(),
        DltDataset::Default(name) => PyString::new(py, name).into_any(),
    };
    Ok(DestinationCall {
        factory: plan.factory,
        arguments: arguments(&plan.arguments, connection_config)?,
        options,
        dataset,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(dlt_destination_arguments, module)?)
}
