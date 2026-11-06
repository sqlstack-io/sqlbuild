//! The native model config build handed Python inputs once, and its results projected for Python.

use std::collections::HashMap;

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{PyErr, pyclass, pymethods};
use sqlbuild_discovery::model_files::main::config_value::config_value;
use sqlbuild_discovery::model_files::models::{EntryFacts, HeaderFacts};
use sqlbuild_model_config::config_presence::models::MacroPath;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::model_config_build::main::build_model_config::build_model_config;
use sqlbuild_model_config::model_config_build::main::inherited_config::inherited_config;
use sqlbuild_model_config::model_config_build::main::model_context_values::model_context_values;
use sqlbuild_model_config::model_config_build::models::{
    BuiltModelConfig, ConfigBuildHalt, ConfigBuildInputs, HeaderEntryFacts, ModelHeader,
};
use sqlbuild_model_config::model_config_build::types::ConfigBuildHost;
use sqlbuild_model_config::model_validation::models::{RetentionOverride, TableTypeOverride};
use sqlbuild_model_config::path_defaults::main::select_path_default::select_path_default;
use sqlbuild_model_config::path_defaults::models::PathDefaultChoice;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

use crate::bindings::_helpers::authored_values::project_vars::project_var_text;
use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::model_config::config_classes::{ConfigClasses, built_config};
use crate::bindings::_helpers::model_config::config_errors::config_exception;
use crate::bindings::_helpers::model_config::config_errors::native_config_error;
use crate::bindings::_helpers::model_config::config_nodes::{
    HostHandle, PyConfigEntries, PyConfigNode, carried_node, config_dict, config_entries,
    config_node, config_object, hook_entry, is_exact_container,
};
use crate::bindings::_helpers::model_config::errors::NativeConfigError;
use crate::bindings::_helpers::model_config::project_defaults::project_defaults_mapping;
use crate::bindings::_helpers::model_config::typed_config::{BuiltConfigValues, built_values};

/// The config keys a model's context values read.
const CONTEXT_KEYS: [&str; 3] = ["database", "schema", "alias"];
const BUILD_STOPPED: &str = "a config value read as text without an error";

/// A header's public mapping, parsed facts and native authored values.
type HeaderBuildInputs<'a, 'py> = (
    &'a Bound<'py, PyDict>,
    Option<&'a HeaderFacts>,
    Option<&'a [(String, AuthoredValue)]>,
);

/// The project-wide inputs every model's config build reads, converted once per compile.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeModelConfigBuilder {
    inputs: ConfigBuildInputs<HostHandle>,
    path_keys: Vec<String>,
    environment: Py<PyAny>,
}

/// One model's built config before Python wraps it in `CompileModelConfig`.
pub(crate) struct BuiltConfig<'py> {
    pub(crate) values: Bound<'py, PyDict>,
    pub(crate) nodes: Option<BuiltConfigValues>,
    pub(crate) header_keys: Vec<String>,
    pub(crate) logical_schema: Option<Bound<'py, PyAny>>,
    pub(crate) layer_schema_configured: bool,
    pub(crate) logical_database: Option<Bound<'py, PyAny>>,
    pub(crate) retention: RetentionOverride,
    pub(crate) table_type: TableTypeOverride,
    pub(crate) reads: Vec<(&'static str, String)>,
}

/// A native build's outcome: the built config, or the config error it raises.
pub(crate) enum ConfigBuild<'py> {
    Built(BuiltConfig<'py>),
    Rejected(Py<NativeConfigError>),
}

/// `((target name, run id), (database, schema) or None)` of the active target.
pub(crate) type TargetInputs<'py> = (
    (Bound<'py, PyAny>, Bound<'py, PyAny>),
    Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
);

/// The process environment and project-var rendering, read through Python.
struct PythonConfigHost<'py> {
    environment: Bound<'py, PyAny>,
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
fn header_entries(
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
fn entry_facts((key, entry): &(String, EntryFacts)) -> (String, HeaderEntryFacts) {
    let facts: HeaderEntryFacts = HeaderEntryFacts {
        template: entry.template,
        macro_path: entry.macro_path.clone(),
    };
    (key.clone(), facts)
}

fn optional_node(value: Bound<'_, PyAny>) -> PyResult<Option<PyConfigNode>> {
    if value.is_none() {
        return Ok(None);
    }
    config_node(&value).map(Some)
}

#[pymethods]
impl NativeModelConfigBuilder {
    #[new]
    pub(crate) fn new(
        layers: (Bound<'_, PyAny>, Bound<'_, PyDict>, Bound<'_, PyTuple>),
        sources: (Bound<'_, PyDict>, Bound<'_, PyAny>),
        target: TargetInputs<'_>,
    ) -> PyResult<Self> {
        let (run, target_namespace) = target;
        let (defaults, path_defaults, _hook_entry_types) = layers;
        let mut path_keys: Vec<String> = Vec::with_capacity(path_defaults.len());
        let mut path_values: Vec<(String, PyConfigEntries)> =
            Vec::with_capacity(path_defaults.len());
        for (key, values) in path_defaults.iter() {
            let key: String = key.extract()?;
            path_values.push((key.clone(), config_entries(values.downcast::<PyDict>()?)?));
            path_keys.push(key);
        }
        let (target_name, run_id) = run;
        Ok(Self {
            inputs: ConfigBuildInputs {
                defaults: config_entries(&project_defaults_mapping(&defaults)?)?,
                path_defaults: path_values,
                variables: config_entries(&sources.0)?,
                run_id: config_node(&run_id)?,
                target_name: optional_node(target_name)?.unwrap_or(ConfigNode::Null),
                target_namespace: target_namespace
                    .map(|(database, schema)| -> PyResult<_> {
                        Ok((config_node(&database)?, config_node(&schema)?))
                    })
                    .transpose()?,
            },
            path_keys,
            environment: sources.1.unbind(),
        })
    }

    /// Return the model's `CompileModelConfig` or the first config error.
    fn model_config<'py>(
        &self,
        py: Python<'py>,
        header: Bound<'py, PyDict>,
        model: (Option<String>, String),
        classes: ConfigClasses<'py>,
    ) -> PyResult<Py<PyAny>> {
        let (matched_path_default, model_name) = model;
        let matched_path_default: Option<&str> = matched_path_default.as_deref();
        compiler_guard(|| {
            match self.built(py, (&header, None, None), matched_path_default, &model_name)? {
                ConfigBuild::Built(built) => {
                    Ok(built_config(&classes, &built, matched_path_default)?.unbind())
                }
                ConfigBuild::Rejected(error) => Ok(error.into_any()),
            }
        })
    }
}

/// The Python error a halt raises, or the config error it is rejected with.
fn halted<'py>(
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
fn projected_build<'py>(
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

impl NativeModelConfigBuilder {
    fn host<'py>(&self, py: Python<'py>) -> PythonConfigHost<'py> {
        PythonConfigHost {
            environment: self.environment.bind(py).clone(),
        }
    }

    /// The selected path-default key, or the conflict error Python raises.
    pub(crate) fn selected_path_default(
        &self,
        py: Python<'_>,
        model_path: &str,
    ) -> PyResult<Result<Option<String>, Py<NativeConfigError>>> {
        Ok(match select_path_default(model_path, &self.path_keys) {
            PathDefaultChoice::Selected(key) => Ok(key),
            PathDefaultChoice::Conflict(error) => Err(native_config_error(py, error)?),
        })
    }

    /// Build one model's config natively.
    pub(crate) fn built<'py>(
        &self,
        py: Python<'py>,
        (header, facts, authored): HeaderBuildInputs<'_, 'py>,
        matched_path_default: Option<&str>,
        model_name: &str,
    ) -> PyResult<ConfigBuild<'py>> {
        let facts: Option<Vec<(String, HeaderEntryFacts)>> =
            facts.map(|facts| facts.entries.iter().map(entry_facts).collect());
        let entries: PyConfigEntries = header_entries(header, facts.as_deref(), authored)?;
        let model_header = ModelHeader {
            entries: &entries,
            facts: facts.as_deref(),
        };
        let host: PythonConfigHost<'py> = self.host(py);
        match build_model_config(
            (&host, &self.inputs),
            model_header,
            matched_path_default,
            model_name,
        ) {
            Ok(built) => Ok(ConfigBuild::Built(projected_build(header, built)?)),
            Err(halt) => match halted(py, halt)? {
                Ok(error) => Ok(ConfigBuild::Rejected(native_config_error(py, error)?)),
                Err(error) => Err(error),
            },
        }
    }

    /// `build_layered_model_values` with an empty header: what a path default's models inherit.
    pub(crate) fn inherited_values<'py>(
        &self,
        py: Python<'py>,
        matched_path_default: Option<&str>,
    ) -> PyResult<Result<Bound<'py, PyDict>, Py<NativeConfigError>>> {
        let host: PythonConfigHost<'py> = self.host(py);
        match inherited_config((&host, &self.inputs), matched_path_default) {
            Ok(values) => Ok(Ok(config_dict(py, &values)?)),
            Err(halt) => match halted(py, halt)? {
                Ok(error) => Ok(Err(native_config_error(py, error)?)),
                Err(error) => Err(error),
            },
        }
    }

    /// `build_model_context_values` for a model's config values.
    pub(crate) fn model_context_values<'py>(
        &self,
        values: &Bound<'py, PyDict>,
        model_name: &str,
        include_target_values: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        let mut read: PyConfigEntries = Vec::with_capacity(CONTEXT_KEYS.len());
        for key in CONTEXT_KEYS {
            if let Some(value) = values.get_item(key)? {
                read.push((ConfigNode::Str(key.to_owned()), config_node(&value)?));
            }
        }
        let context: PyConfigEntries =
            model_context_values(&self.inputs, &read, model_name, include_target_values);
        config_dict(values.py(), &context)
    }

    /// The active target's `(database, schema)`, or `(None, None)` without a target.
    pub(crate) fn target_namespace<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        match &self.inputs.target_namespace {
            Some((database, schema)) => PyTuple::new(
                py,
                [config_object(py, database)?, config_object(py, schema)?],
            ),
            None => PyTuple::new(py, [py.None(), py.None()]),
        }
    }
}

/// The Python exception a native config error raises as.
pub(crate) fn raised_config_error(py: Python<'_>, error: ConfigError) -> PyErr {
    let raised = || -> PyResult<PyErr> {
        let error: Py<NativeConfigError> = native_config_error(py, error)?;
        Ok(PyErr::from_value(config_exception(
            error.bind(py).as_any(),
            None,
        )?))
    };
    raised().unwrap_or_else(|error| error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelConfigBuilder>()?;
    Ok(())
}
