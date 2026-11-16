//! The native model config build handed Python inputs once, and its results projected for Python.

use crate::model_config::main::config::carried_node::carried_node;
use crate::model_config::main::config::config_node::config_node;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::models::BuiltConfig;
use std::collections::HashMap;

use pyo3::PyErr;
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods};
use sqlbuild_discovery::model_files::main::config_value::config_value;
use sqlbuild_discovery::model_files::models::{EntryFacts, HeaderFacts};
use sqlbuild_model_config::config_presence::models::MacroPath;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::model_config_build::models::{
    BuiltModelConfig, ConfigBuildHalt, HeaderEntryFacts,
};
use sqlbuild_model_config::model_config_build::types::ConfigBuildHost;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::authored_values::main::project_var_text::project_var_text;
use crate::model_config::_helpers::config::config_nodes::{
    PyConfigEntries, hook_entry, is_exact_container,
};
use crate::model_config::_helpers::config::typed_config::built_values;
use crate::model_config::models::HostHandle;
use crate::model_config::types::PyConfigNode;

/// The config keys a model's context values read.
pub(crate) const CONTEXT_KEYS: [&str; 3] = ["database", "schema", "alias"];
const BUILD_STOPPED: &str = "a config value read as text without an error";

/// A header's public mapping, parsed facts and native authored values.
pub(crate) type HeaderBuildInputs<'a, 'py> = (
    &'a Bound<'py, PyDict>,
    Option<&'a HeaderFacts>,
    Option<&'a [(String, AuthoredValue)]>,
);

/// `((target name, run id), (database, schema) or None)` of the active target.
pub(crate) type TargetInputs<'py> = (
    (Bound<'py, PyAny>, Bound<'py, PyAny>),
    Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
);

/// The process environment and project-var rendering, read through Python.
pub(crate) struct PythonConfigHost<'py> {
    pub(crate) environment: Bound<'py, PyAny>,
}

impl ConfigBuildHost for PythonConfigHost<'_> {
    type Handle = HostHandle;
    type Error = PyErr;

    fn environment(&self, name: &str) -> PyResult<Option<PyConfigNode>> {
        if !self.environment.contains(name)? {
            return Ok(None);
        }
        Ok(Some(config_node(&self.environment.get_item(name)?)?))
    }

    fn render(&self, value: &PyConfigNode, label: &str) -> PyResult<Result<String, String>> {
        let py: Python<'_> = self.environment.py();
        let object: Bound<'_, PyAny> = config_object(py, value)?;
        if let Ok(text) = object.downcast_exact::<PyString>() {
            return Ok(Ok(text.to_str()?.to_owned()));
        }
        match project_var_text(&object, label) {
            Ok(text) => Ok(Ok(text)),
            Err(error) if error.is_instance_of::<PyValueError>(py) => {
                Ok(Err(error.value(py).to_string()))
            }
            Err(error) => Err(error),
        }
    }

    fn text_error(&self, value: &PyConfigNode) -> PyErr {
        let py: Python<'_> = self.environment.py();
        let read = || -> PyResult<()> {
            let object: Bound<'_, PyAny> = config_object(py, value)?;
            let text: Bound<'_, PyString> = match object.downcast::<PyString>() {
                Ok(text) => text.clone(),
                Err(_) => object.str()?,
            };
            let _ = text.to_str()?;
            Ok(())
        };
        match read() {
            Ok(()) => PyRuntimeError::new_err(BUILD_STOPPED),
            Err(error) => error,
        }
    }

    fn python_str(&self, value: &PyConfigNode, exact: bool) -> PyResult<String> {
        let text: Bound<'_, PyString> = config_object(self.environment.py(), value)?.str()?;
        if exact {
            return Ok(text.to_str()?.to_owned());
        }
        Ok(text.to_string_lossy().into_owned())
    }

    fn typed_hook_entry(&self, value: &PyConfigNode) -> PyResult<bool> {
        hook_entry(&config_object(self.environment.py(), value)?)
    }
}

/// Header keys whose values the build reads item by item, which are always converted.
const INSPECTED_KEYS: [&str; 7] = [
    "tags",
    "row_diff_exclude_columns",
    "row_diff_tolerances",
    "pre_hooks",
    "post_hooks",
    "time_travel_retention",
    "table_type",
];

/// Whether `key`'s container may be carried whole: no template or macro in it, items never read.
fn carried_whole(key: &str, facts: Option<&[(String, HeaderEntryFacts)]>) -> bool {
    !INSPECTED_KEYS.contains(&key)
        && facts
            .and_then(|facts| facts.iter().find(|(name, _)| name == key))
            .is_some_and(|(_, facts)| !facts.template && facts.macro_path == MacroPath::Absent)
}

/// A header's entries as config nodes; containers the build never looks into stay opaque.
pub(crate) fn header_entries(
    header: &Bound<'_, PyDict>,
    facts: Option<&[(String, HeaderEntryFacts)]>,
    authored: Option<&[(String, AuthoredValue)]>,
) -> PyResult<PyConfigEntries> {
    let authored: HashMap<&str, &AuthoredValue> = authored
        .into_iter()
        .flatten()
        .map(|(key, value)| (key.as_str(), value))
        .collect();
    let mut entries: PyConfigEntries = Vec::with_capacity(header.len());
    for (key, value) in header.iter() {
        let native: Option<PyConfigNode> = match key.downcast_exact::<PyString>() {
            Ok(key) => match key.to_str() {
                Ok(key) => authored.get(key).and_then(|value| config_value(value)),
                Err(_) => None,
            },
            Err(_) => None,
        };
        if let Some(native) = native {
            entries.push((config_node(&key)?, native));
            continue;
        }
        let whole: bool = is_exact_container(&value)
            && key
                .downcast_exact::<PyString>()
                .is_ok_and(|key| key.to_str().is_ok_and(|key| carried_whole(key, facts)));
        let node: PyConfigNode = if whole {
            carried_node(&value)?
        } else {
            config_node(&value)?
        };
        entries.push((config_node(&key)?, node));
    }
    Ok(entries)
}

/// One header entry's parsed facts as the build reads them.
pub(crate) fn entry_facts((key, entry): &(String, EntryFacts)) -> (String, HeaderEntryFacts) {
    let facts: HeaderEntryFacts = HeaderEntryFacts {
        template: entry.template,
        macro_path: entry.macro_path.clone(),
    };
    (key.clone(), facts)
}

pub(crate) fn optional_node(value: Bound<'_, PyAny>) -> PyResult<Option<PyConfigNode>> {
    if value.is_none() {
        return Ok(None);
    }
    config_node(&value).map(Some)
}

/// The Python error a halt raises, or the config error it is rejected with.
pub(crate) fn halted<'py>(
    py: Python<'py>,
    halt: ConfigBuildHalt<HostHandle, PyErr>,
) -> PyResult<Result<ConfigError, PyErr>> {
    Ok(match halt {
        ConfigBuildHalt::Config(error) => Ok(error),
        ConfigBuildHalt::Host(error) => Err(error),
        ConfigBuildHalt::MissingPathDefault(key) => Err(PyKeyError::new_err(key)),
        ConfigBuildHalt::KeyNotText(key) => {
            Err(match config_object(py, &key)?.downcast::<PyString>() {
                Ok(_) => PyRuntimeError::new_err(BUILD_STOPPED),
                Err(error) => error.into(),
            })
        }
    })
}

/// The built values as a new `dict`; entries still the header's own keep the header's objects.
fn own_values_dict<'py>(
    header: &Bound<'py, PyDict>,
    built: &BuiltModelConfig<HostHandle>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = header.py();
    let mapping: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in &built.values {
        let own: bool = own_key(key, &built.own_header_keys);
        let key: Bound<'py, PyAny> = config_object(py, key)?;
        let value: Bound<'py, PyAny> = match own.then(|| header.get_item(&key)).transpose()? {
            Some(Some(value)) => value,
            _ => config_object(py, value)?,
        };
        mapping.set_item(key, value)?;
    }
    Ok(mapping)
}

/// Whether `key` is a string key named among the header's own keys.
fn own_key(key: &PyConfigNode, own: &[String]) -> bool {
    let text: Option<&str> = match key {
        ConfigNode::Str(text) => Some(text),
        ConfigNode::Host(host) if host.facts.strict_text => host.facts.text.as_deref(),
        _ => None,
    };
    text.is_some_and(|text| own.iter().any(|name| name == text))
}

/// One model's build projected for Python.
pub(crate) fn projected_build<'py>(
    header: &Bound<'py, PyDict>,
    built: BuiltModelConfig<HostHandle>,
) -> PyResult<BuiltConfig<'py>> {
    let py: Python<'py> = header.py();
    Ok(BuiltConfig {
        values: own_values_dict(header, &built)?,
        nodes: built_values(built.values),
        header_keys: built.header_keys,
        logical_schema: projected(py, built.logical_schema)?,
        layer_schema_configured: built.layer_schema_configured,
        logical_database: projected(py, built.logical_database)?,
        retention: built.retention,
        table_type: built.table_type,
        reads: built.reads,
    })
}

fn projected<'py>(
    py: Python<'py>,
    value: Option<PyConfigNode>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    value.map(|value| config_object(py, &value)).transpose()
}
