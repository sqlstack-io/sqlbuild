//! Public records of model config.

use crate::attachments::main::argument_pairs::argument_pairs;
use crate::attachments::main::config_argument_value::config_argument_value;
use crate::authored_values::main::project_var_text::project_var_text;
use crate::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use crate::discovery::main::project::each::each;
use crate::discovery::models::NativeModelFile;
use crate::model_config::_helpers::config::config_classes::built_config;
use crate::model_config::_helpers::config::config_nodes::PyConfigEntries;
use crate::model_config::_helpers::config::config_nodes::config_dict;
use crate::model_config::_helpers::config::config_nodes::config_entries;
use crate::model_config::_helpers::config::config_templates::CONTEXT_READ;
use crate::model_config::_helpers::config::config_templates::EFFECTIVE_VARS_LABEL;
use crate::model_config::_helpers::config::config_templates::ENVIRONMENT_READ;
use crate::model_config::_helpers::config::config_templates::EffectiveVars;
use crate::model_config::_helpers::headers::header_audits::authored_audit;
use crate::model_config::_helpers::headers::header_audits::config_json_value;
use crate::model_config::_helpers::headers::header_audits::retain_audit;
use crate::model_config::_helpers::headers::header_metadata::ColumnFields;
use crate::model_config::_helpers::headers::header_metadata::SCHEMA_COLUMN_FIELDS;
use crate::model_config::_helpers::headers::header_metadata::audit_instance;
use crate::model_config::_helpers::headers::header_metadata::checked_field_orders;
use crate::model_config::_helpers::headers::header_metadata::contract_object;
use crate::model_config::_helpers::headers::header_metadata::native_location;
use crate::model_config::_helpers::headers::header_metadata::native_value;
use crate::model_config::_helpers::headers::header_metadata::native_values;
use crate::model_config::_helpers::model_nodes::authored_nodes::entry_nodes;
use crate::model_config::_helpers::model_nodes::model_config_builder::CONTEXT_KEYS;
use crate::model_config::_helpers::model_nodes::model_config_builder::HeaderBuildInputs;
use crate::model_config::_helpers::model_nodes::model_config_builder::PythonConfigHost;
use crate::model_config::_helpers::model_nodes::model_config_builder::TargetInputs;
use crate::model_config::_helpers::model_nodes::model_config_builder::entry_facts;
use crate::model_config::_helpers::model_nodes::model_config_builder::halted;
use crate::model_config::_helpers::model_nodes::model_config_builder::header_entries;
use crate::model_config::_helpers::model_nodes::model_config_builder::optional_node;
use crate::model_config::_helpers::model_nodes::model_config_builder::projected_build;
use crate::model_config::_helpers::model_nodes::model_validator::ModelFacts;
use crate::model_config::_helpers::model_nodes::model_validator::ResourceNames;
use crate::model_config::_helpers::model_nodes::model_validator::discovered_names;
use crate::model_config::_helpers::model_nodes::model_validator::stop_object;
use crate::model_config::_helpers::model_nodes::project_defaults::project_defaults_mapping;
use crate::model_config::errors::NativeConfigError;
use crate::model_config::main::config::config_node::config_node;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::main::config::native_config_error::native_config_error;
use crate::model_config::main::headers::family_specs::family_specs;
use crate::model_config::main::headers::thresholds::thresholds;
use crate::model_config::main::model_nodes::contract_classes::contract_classes;
use crate::model_config::types::BuiltConfigValues;
use crate::model_config::types::HeaderValue;
use crate::model_config::types::PyConfigNode;
use pyo3::FromPyObject;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyBool;
use pyo3::types::PyBoolMethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyFloat;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use pyo3::types::PyTuple;
use pyo3::types::PyTupleMethods;
use sqlbuild_attachments::audits::models::ArgumentValue;
use sqlbuild_attachments::functions::models::KnownResources;
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_discovery::model_files::models::NativeColumn;
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::config_values::types::HostObject;
use sqlbuild_model_config::header_metadata::constants::ACCEPTED_VALUES_ARGUMENT;
use sqlbuild_model_config::header_metadata::constants::ACCEPTED_VALUES_AUDIT;
use sqlbuild_model_config::header_metadata::constants::AUDITS_KEY;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_model_config::header_metadata::models::ParsedAudit;
use sqlbuild_model_config::model_config_build::main::build_model_config::build_model_config;
use sqlbuild_model_config::model_config_build::main::inherited_config::inherited_config;
use sqlbuild_model_config::model_config_build::main::model_context_values::model_context_values;
use sqlbuild_model_config::model_config_build::models::ConfigBuildInputs;
use sqlbuild_model_config::model_config_build::models::HeaderEntryFacts;
use sqlbuild_model_config::model_config_build::models::ModelHeader;
use sqlbuild_model_config::model_validation::main::validate_model_config::validate_model_config;
use sqlbuild_model_config::model_validation::main::validate_model_references::validate_model_references;
use sqlbuild_model_config::model_validation::models::ModelReference;
use sqlbuild_model_config::model_validation::models::ModelValidationFacts;
use sqlbuild_model_config::model_validation::models::ProjectValidationFacts;
use sqlbuild_model_config::model_validation::models::RetentionOverride;
use sqlbuild_model_config::model_validation::models::TableTypeOverride;
use sqlbuild_model_config::path_defaults::main::select_path_default::select_path_default;
use sqlbuild_model_config::path_defaults::models::PathDefaultChoice;
use sqlbuild_model_config::templates::errors::TemplateError;
use sqlbuild_model_config::templates::main::expand_template_text::expand_template_text;
use sqlbuild_model_config::templates::models::ContextValue;
use sqlbuild_model_config::templates::models::Scalar;
use sqlbuild_model_config::templates::models::TemplateFailure;
use sqlbuild_model_config::templates::models::TemplateOptions;
use sqlbuild_model_config::templates::types::TemplateHost;
use sqlbuild_model_config::types::AuthoredNode;
use sqlbuild_model_config::types::NodeKind;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::native_host::models::AuditProjection;
use sqlbuild_python_base::native_host::models::FamilyProjection;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::result_output::main::json_value::json_value;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

/// One Python object viewed as an authored value; parsed results keep the object itself.
#[derive(Clone, Debug)]
pub struct PyNode<'py>(pub Bound<'py, PyAny>);

impl AuthoredNode for PyNode<'_> {
    fn kind(&self) -> NodeKind {
        let value = &self.0;
        if value.is_none() {
            NodeKind::Null
        } else if let Ok(flag) = value.downcast::<PyBool>() {
            NodeKind::Bool(flag.is_true())
        } else if value.is_instance_of::<PyInt>() {
            value
                .lt(0)
                .map_or(NodeKind::Other, |negative| NodeKind::Int { negative })
        } else if value.is_instance_of::<PyString>() {
            NodeKind::Str
        } else if value.is_instance_of::<PyList>() {
            NodeKind::List
        } else if value.is_instance_of::<PyTuple>() {
            NodeKind::Tuple
        } else if value.is_instance_of::<PyDict>() {
            NodeKind::Map
        } else {
            NodeKind::Other
        }
    }

    fn integer(&self) -> Option<i64> {
        if !self.0.is_instance_of::<PyBool>()
            && self.0.is_instance_of::<PyInt>()
            && let Ok(number) = self.0.extract::<i64>()
        {
            Some(number)
        } else {
            None
        }
    }

    fn number(&self) -> Option<f64> {
        let value = &self.0;
        let is_number: bool = value.is_instance_of::<PyFloat>()
            || (value.is_instance_of::<PyInt>() && !value.is_instance_of::<PyBool>());
        if !is_number {
            return None;
        }
        if let Ok(number) = value.extract::<f64>() {
            return Some(number);
        }
        let Ok(number) = self.python_str().parse::<f64>() else {
            return None;
        };
        Some(number)
    }

    fn text(&self) -> Option<String> {
        self.with_text(str::to_owned)
    }

    /// Read a string's text; lone surrogates, rejected where text enters a compile, read as U+FFFD.
    fn with_text<R>(&self, read: impl FnOnce(&str) -> R) -> Option<R> {
        match self.0.downcast::<PyString>() {
            Ok(value) => Some(read(&value.to_string_lossy())),
            Err(_) => None,
        }
    }

    fn is_text(&self, text: &str) -> bool {
        self.with_text(|value| value == text).unwrap_or(false)
    }

    fn items(&self) -> Vec<Self> {
        if let Ok(items) = self.0.downcast::<PyList>() {
            items.iter().map(PyNode).collect()
        } else if let Ok(items) = self.0.downcast::<PyTuple>() {
            items.iter().map(PyNode).collect()
        } else {
            Vec::new()
        }
    }

    fn entries(&self) -> Vec<(Self, Self)> {
        if let Ok(entries) = self.0.downcast::<PyDict>() {
            entries.iter().map(entry_nodes).collect()
        } else {
            Vec::new()
        }
    }

    fn python_str(&self) -> String {
        self.0
            .str()
            .map(|text| text.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn python_repr(&self) -> String {
        self.0
            .repr()
            .map(|text| text.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn is_date_like(&self) -> bool {
        let py = self.0.py();
        py.import("datetime")
            .and_then(|module| native_field(&module, "date"))
            .and_then(|date| self.0.is_instance(&date))
            .unwrap_or(false)
    }
}

/// The builder, storage defaults and Python classes a model's `CompileModelConfig` is made of.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct ConfigClasses<'py> {
    pub builder: Bound<'py, NativeModelConfigBuilder>,
    /// Inherited `(retention, table type)` by table-backed materialization, or `None`.
    pub inherited_storage: Bound<'py, PyDict>,
    pub table_backed: Vec<String>,
    pub compile_model_config: Bound<'py, PyAny>,
    pub resolved_retention: Bound<'py, PyAny>,
    pub retention_source: Bound<'py, PyAny>,
    pub resolved_table_type: Bound<'py, PyAny>,
    pub table_type: Bound<'py, PyAny>,
    pub table_type_source: Bound<'py, PyAny>,
    /// `record_template_reads`, the compile-reuse sink of template reads.
    pub record_template_reads: Bound<'py, PyAny>,
}

/// The Python object an opaque config value stands for.
#[derive(Clone, Debug)]
pub struct HostHandle(pub Arc<Py<PyAny>>);

impl HostObject for HostHandle {
    fn is_date_like(&self) -> bool {
        Python::attach(|py| PyNode(self.0.bind(py).clone()).is_date_like())
    }

    /// Python's text of the projected value; a `str()` or `repr()` that raises reads as empty, as `PyNode`.
    fn display(node: &ConfigNode<Self>, repr: bool) -> String {
        Python::attach(|py| {
            config_object(py, node).map_or_else(
                |_| String::new(),
                |value| {
                    let value: PyNode<'_> = PyNode(value);
                    if repr {
                        value.python_repr()
                    } else {
                        value.python_str()
                    }
                },
            )
        })
    }
}

/// Variables, the process environment and context values, all read through Python.
pub struct PythonHost<'py> {
    pub(crate) py: Python<'py>,
    pub(crate) sources: TemplateSources<'py>,
    pub(crate) label: String,
    pub(crate) effective: Option<EffectiveVars<'py>>,
    pub(crate) reads: RefCell<Vec<(&'static str, String)>>,
    pub(crate) raised: RefCell<Option<pyo3::PyErr>>,
}

impl std::fmt::Debug for PythonHost<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PythonHost").finish_non_exhaustive()
    }
}

impl<'py> PythonHost<'py> {
    /// Read variables, the environment and context values from these Python objects.
    pub fn new(py: Python<'py>, sources: TemplateSources<'py>, label: &str) -> Self {
        Self {
            py,
            sources,
            label: label.to_owned(),
            effective: None,
            reads: RefCell::new(Vec::new()),
            raised: RefCell::new(None),
        }
    }

    /// Return the `(kind, name)` reads in order.
    pub fn into_reads(self) -> Vec<(&'static str, String)> {
        self.reads.into_inner()
    }

    /// Hold a Python error until the expansion returns, and stop evaluation.
    pub(crate) fn hold(&self, error: pyo3::PyErr) -> TemplateFailure {
        self.raised.borrow_mut().get_or_insert(error);
        TemplateFailure::Host
    }

    /// The stop for `failure`, taking the held Python error of a host failure.
    pub fn stop(&self, failure: TemplateFailure) -> Stop {
        match failure {
            TemplateFailure::Missing(error) | TemplateFailure::Invalid(error) => {
                Stop::Failure(error)
            }
            TemplateFailure::Host => {
                Stop::Python(self.raised.borrow_mut().take().unwrap_or_else(|| {
                    PyRuntimeError::new_err("template expansion stopped without an error")
                }))
            }
        }
    }

    pub(crate) fn text_of(&self, value: &Bound<'py, PyAny>) -> Result<String, TemplateFailure> {
        value
            .downcast::<PyString>()
            .map_err(|error| self.hold(error.into()))?
            .to_str()
            .map(str::to_owned)
            .map_err(|error| self.hold(error))
    }

    /// Resolve effective var `name`, keeping Python's resolving stack on errors.
    pub(crate) fn resolve(
        &self,
        effective: &EffectiveVars<'py>,
        name: &str,
    ) -> Result<Option<Bound<'py, PyAny>>, TemplateFailure> {
        let Some(raw) = self
            .sources
            .0
            .get_item(name)
            .map_err(|error| self.hold(error))?
        else {
            return Ok(None);
        };
        if let Some(value) = effective
            .resolved
            .get_item(name)
            .map_err(|error| self.hold(error))?
        {
            return Ok(Some(value));
        }
        if effective.resolving.borrow().iter().any(|key| key == name) {
            let mut cycle = effective.resolving.borrow().clone();
            cycle.push(name.to_owned());
            return Err(TemplateFailure::Invalid(TemplateError::Message(format!(
                "{EFFECTIVE_VARS_LABEL} contain a cyclic reference: {}",
                cycle.join(" -> ")
            ))));
        }
        effective.resolving.borrow_mut().push(name.to_owned());
        let value = if raw.is_instance_of::<PyString>() {
            let text = self.text_of(&raw)?;
            let options = TemplateOptions {
                allow_context: false,
                preserve_context_tokens: false,
                preserve_unknown_context: false,
            };
            PyString::new(self.py, &expand_template_text(self, &text, options)?).into_any()
        } else {
            raw
        };
        effective.resolving.borrow_mut().pop();
        effective
            .resolved
            .set_item(name, &value)
            .map_err(|error| self.hold(error))?;
        Ok(Some(value))
    }
}

/// The variables, the `os.environ` mapping and the context values one expansion reads.
#[derive(Debug, FromPyObject)]
pub struct TemplateSources<'py>(
    pub Bound<'py, PyDict>,
    pub Bound<'py, PyAny>,
    pub Bound<'py, PyDict>,
);

/// A template error, or a Python error raised while reading values or building containers.
#[derive(Debug)]
pub enum Stop {
    Failure(TemplateError),
    Python(pyo3::PyErr),
}

impl<'py> TemplateHost for PythonHost<'py> {
    type Value = Bound<'py, PyAny>;

    fn variable(&self, name: &str) -> Result<Option<Self::Value>, TemplateFailure> {
        if let Some(effective) = &self.effective {
            return self.resolve(effective, name);
        }
        self.sources
            .0
            .get_item(name)
            .map_err(|error| self.hold(error))
    }

    fn environment(&self, name: &str) -> Result<Option<Self::Value>, TemplateFailure> {
        self.reads
            .borrow_mut()
            .push((ENVIRONMENT_READ, name.to_owned()));
        let environment: &Bound<'py, PyAny> = &self.sources.1;
        if environment
            .contains(name)
            .map_err(|error| self.hold(error))?
        {
            environment
                .get_item(name)
                .map(Some)
                .map_err(|error| self.hold(error))
        } else {
            Ok(None)
        }
    }

    fn context(&self, name: &str) -> Result<ContextValue<Self::Value>, TemplateFailure> {
        self.reads
            .borrow_mut()
            .push((CONTEXT_READ, name.to_owned()));
        match self
            .sources
            .2
            .get_item(name)
            .map_err(|error| self.hold(error))?
        {
            None => Ok(ContextValue::Unknown),
            Some(value) if value.is_none() => Ok(ContextValue::Unavailable),
            Some(value) => Ok(ContextValue::Value(value)),
        }
    }

    fn text(&self, text: &str) -> Result<Self::Value, TemplateFailure> {
        Ok(PyString::new(self.py, text).into_any())
    }

    fn boolean(&self, flag: bool) -> Result<Self::Value, TemplateFailure> {
        Ok(PyBool::new(self.py, flag).to_owned().into_any())
    }

    fn null(&self) -> Self::Value {
        self.py.None().into_bound(self.py)
    }

    fn scalar(&self, value: &Self::Value) -> Result<Scalar, TemplateFailure> {
        if value.is_none() {
            return Ok(Scalar::Null);
        }
        if let Ok(flag) = value.downcast_exact::<PyBool>() {
            return Ok(Scalar::Bool(flag.is_true()));
        }
        let text = value.str().map_err(|error| self.hold(error))?;
        self.text_of(text.as_any()).map(Scalar::Text)
    }

    fn render(&self, value: &Self::Value, label: &str) -> Result<String, TemplateFailure> {
        if value.is_exact_instance_of::<PyString>() {
            return self.text_of(value);
        }
        match project_var_text(value, label) {
            Ok(text) => Ok(text),
            Err(error) if error.is_instance_of::<PyValueError>(self.py) => Err(
                TemplateFailure::Invalid(TemplateError::Message(error.value(self.py).to_string())),
            ),
            Err(error) => Err(self.hold(error)),
        }
    }

    fn label(&self) -> &str {
        &self.label
    }
}

/// Native-built specs never become Python audit records just to be read by attachment.
#[derive(Debug)]
pub enum AuditInstance<'py> {
    Native(Arc<ParsedAudit<PyConfigNode>>, Bound<'py, PyAny>),
    Python(Bound<'py, PyAny>),
}

impl<'py> AuditInstance<'py> {
    /// Equality with the builtin generated enum audit, without constructing argument containers.
    pub fn accepts_enum(&self, values: &[PyConfigNode]) -> PyResult<bool> {
        let Self::Native(spec, location) = self else {
            return Ok(false);
        };
        let py = location.py();
        if spec.definition_name.exact_text(py)? != ACCEPTED_VALUES_AUDIT
            || spec.thresholds.is_some()
        {
            return Ok(false);
        }
        for name in [
            "name",
            "description",
            "severity",
            "run_scope",
            "minimum_samples",
            "evidence_limit",
            "meta",
        ] {
            if !self.field(name)?.is_none() {
                return Ok(false);
            }
        }
        if !self.field("always_run")?.eq(false)? || spec.arguments.len() != 1 {
            return Ok(false);
        }
        let (key, authored) = &spec.arguments[0];
        let matches = match key {
            ConfigNode::Str(key) => key == ACCEPTED_VALUES_ARGUMENT,
            ConfigNode::Host(_) => {
                let key = key.object(py)?;
                key.is_instance_of::<PyString>() && key.eq(ACCEPTED_VALUES_ARGUMENT)?
            }
            _ => false,
        };
        if !matches {
            return Ok(false);
        }
        let ConfigNode::Tuple(authored) = authored else {
            return Ok(false);
        };
        if authored.len() != values.len() {
            return Ok(false);
        }
        for (left, right) in authored.iter().zip(values) {
            if !left.object(py)?.eq(right.object(py)?)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn meta_present(&self) -> PyResult<bool> {
        match self {
            Self::Native(spec, _) => Ok(spec
                .meta
                .as_ref()
                .is_some_and(|value| !matches!(value, ConfigNode::Null))),
            Self::Python(value) => Ok(!field(value, "meta")?.is_none()),
        }
    }

    pub fn threshold_policy(&self) -> PyResult<(bool, bool)> {
        match self {
            Self::Native(spec, _) => Ok((
                spec.thresholds.is_some(),
                spec.thresholds.is_some_and(|value| value.error.is_some()),
            )),
            Self::Python(value) => {
                let thresholds = field(value, "thresholds")?;
                Ok((
                    !thresholds.is_none(),
                    !thresholds.is_none() && !field(&thresholds, "error")?.is_none(),
                ))
            }
        }
    }

    pub fn field(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let (spec, location) = match self {
            Self::Native(spec, location) => (spec, location),
            Self::Python(value) => return field(value, name),
        };
        let py = location.py();
        let value = match name {
            "definition_name" => return spec.definition_name.object(py),
            "name" => &spec.name,
            "description" => &spec.description,
            "run_scope" => &spec.run_scope,
            "minimum_samples" => &spec.minimum_samples,
            "evidence_limit" => &spec.evidence_limit,
            "meta" => &spec.meta,
            "always_run" => {
                return match &spec.always_run {
                    Some(value) => value.object(py),
                    None => false.into_bound_py_any(py),
                };
            }
            "severity" => {
                let classes = contract_classes(py)?;
                let classes: ContractClasses<'_> = classes.extract()?;
                return match &spec.severity {
                    Some(value) => Ok(classes
                        .severities
                        .get_item(value.object(py)?)?
                        .unwrap_or_else(|| py.None().into_bound(py))),
                    None => Ok(py.None().into_bound(py)),
                };
            }
            "thresholds" => {
                let classes: ContractClasses<'_> = contract_classes(py)?.extract()?;
                return thresholds(py, &classes, spec.thresholds.as_ref());
            }
            "location" => return Ok(location.clone()),
            _ => return Err(pyo3::exceptions::PyAttributeError::new_err(name.to_owned())),
        };
        value
            .as_ref()
            .map(|value| value.object(py))
            .transpose()
            .map(|value| value.unwrap_or_else(|| py.None().into_bound(py)))
    }

    pub fn argument(&self, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self {
            Self::Native(spec, location) => {
                for (key, value) in &spec.arguments {
                    let matches = match key {
                        ConfigNode::Str(key) => key == name,
                        ConfigNode::Host(_) => {
                            let key = key.object(location.py())?;
                            key.is_instance_of::<PyString>() && key.eq(name)?
                        }
                        _ => false,
                    };
                    if matches {
                        return value.object(location.py()).map(Some);
                    }
                }
                Ok(None)
            }
            Self::Python(value) => field(value, "arguments")?.cast::<PyDict>()?.get_item(name),
        }
    }

    pub fn arguments(&self) -> PyResult<Vec<(String, ArgumentValue)>> {
        match self {
            Self::Native(spec, location) => {
                let mut arguments = Vec::with_capacity(spec.arguments.len());
                for (key, value) in &spec.arguments {
                    let key = match key {
                        ConfigNode::Str(key) => key.clone(),
                        ConfigNode::Host(_) => {
                            let key = key.object(location.py())?;
                            let Ok(key) = key.cast::<PyString>() else {
                                continue;
                            };
                            key.to_string_lossy().into_owned()
                        }
                        _ => continue,
                    };
                    arguments.push((key, config_argument_value(location.py(), value)?));
                }
                Ok(arguments)
            }
            Self::Python(value) => argument_pairs(field(value, "arguments")?.cast()?),
        }
    }

    pub fn meta(&self) -> PyResult<Option<JsonValue>> {
        match self {
            Self::Native(spec, location) => spec
                .meta
                .as_ref()
                .map(|value| config_json_value(location.py(), value))
                .transpose()
                .map(Option::flatten),
            Self::Python(value) => {
                let meta = field(value, "meta")?;
                if meta.is_none() {
                    Ok(None)
                } else {
                    json_value(&meta)
                }
            }
        }
    }
}

/// The parsed specs and genuine Python audit objects of one model or column.
#[derive(Debug)]
pub struct HeaderAudits {
    pub(crate) specs: Vec<ParsedAudit<PyConfigNode>>,
    pub(crate) native_generated: Vec<ParsedAudit<PyConfigNode>>,
    pub(crate) public: AuditProjection,
}

impl HeaderAudits {
    pub fn new<N: HeaderValue>(
        py: Python<'_>,
        classes: &ContractClasses<'_>,
        specs: &[ParsedAudit<N>],
        location: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let custom = if classes.uses_default_classes()? {
            None
        } else {
            Some(
                specs
                    .iter()
                    .map(|spec| audit_instance(py, classes, spec, location).map(Bound::unbind))
                    .collect::<PyResult<_>>()?,
            )
        };
        Ok(Self {
            specs: specs
                .iter()
                .map(|spec| retain_audit(py, spec))
                .collect::<PyResult<_>>()?,
            native_generated: Vec::new(),
            public: AuditProjection {
                custom,
                generated: Vec::new(),
                location: location.clone().unbind(),
                records: PyOnceLock::new(),
            },
        })
    }

    pub fn extend(&mut self, generated: Vec<Bound<'_, PyAny>>) {
        self.public
            .generated
            .extend(generated.into_iter().map(Bound::unbind));
    }

    pub fn push_native(&mut self, spec: ParsedAudit<PyConfigNode>) {
        self.native_generated.push(spec);
    }

    /// The number of audits the header declares, generated ones included.
    pub fn count(&self) -> usize {
        self.specs.len() + self.public.generated.len() + self.native_generated.len()
    }

    pub fn native_specs(&self) -> &[ParsedAudit<PyConfigNode>] {
        if self.public.custom.is_some() {
            &[]
        } else {
            &self.specs
        }
    }

    pub fn python_specs<'py>(&self, py: Python<'py>) -> Vec<Bound<'py, PyAny>> {
        self.public
            .custom
            .iter()
            .flatten()
            .chain(&self.public.generated)
            .map(|spec| spec.bind(py).clone())
            .collect()
    }

    pub fn instances<'py>(&self, py: Python<'py>) -> Vec<AuditInstance<'py>> {
        self.native_specs()
            .iter()
            .map(|spec| {
                AuditInstance::Native(
                    Arc::new(spec.clone()),
                    self.public.location.bind(py).clone(),
                )
            })
            .chain(self.python_specs(py).into_iter().map(AuditInstance::Python))
            .chain(self.native_generated.iter().map(|spec| {
                AuditInstance::Native(Arc::new(spec.clone()), py.None().into_bound(py))
            }))
            .collect()
    }

    pub fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.public
            .records
            .get_or_try_init(py, || {
                let classes: ContractClasses<'_> = contract_classes(py)?.extract()?;
                let mut audits = self
                    .native_specs()
                    .iter()
                    .map(|spec| audit_instance(py, &classes, spec, self.public.location.bind(py)))
                    .collect::<PyResult<Vec<_>>>()?;
                audits.extend(self.python_specs(py));
                for spec in &self.native_generated {
                    audits.push(audit_instance(
                        py,
                        &classes,
                        spec,
                        &py.None().into_bound(py),
                    )?);
                }
                PyTuple::new(py, audits).map(Bound::unbind)
            })
            .map(|value| value.bind(py).clone())
    }

    pub fn key_rows(&self, py: Python<'_>) -> PyResult<Vec<(String, Option<String>)>> {
        let mut rows = Vec::with_capacity(self.count());
        for spec in self.instances(py) {
            let column = spec.argument("column")?;
            rows.push((
                spec.field("definition_name")?.extract()?,
                column
                    .filter(|value| value.is_instance_of::<PyString>())
                    .map(|value| value.extract())
                    .transpose()?,
            ));
        }
        Ok(rows)
    }
}

/// Parsed family specs stay native until a public schema attribute needs records.
#[derive(Debug)]
pub struct HeaderFamilies {
    pub specs: Vec<DynamicFamily>,
    pub(crate) public: FamilyProjection,
}

impl HeaderFamilies {
    pub fn new(class: &Bound<'_, PyAny>, specs: Vec<DynamicFamily>) -> PyResult<Self> {
        let result = Self {
            specs,
            public: FamilyProjection::new(
                class,
                !class.is(&record_class(
                    class.py(),
                    RecordId::SchemaDynamicColumnFamily,
                )?),
            ),
        };
        if result.public.custom {
            let _ = result.project(class.py())?;
        }
        Ok(result)
    }

    pub fn read(&self, py: Python<'_>) -> PyResult<Vec<DynamicFamily>> {
        if self.public.custom {
            family_specs(self.project(py)?.as_any())
        } else {
            Ok(self.specs.clone())
        }
    }

    pub fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.public.project(py, &self.specs)
    }
}

/// Native column fields; a Python record is constructed only for public consumers.
pub struct HeaderColumn {
    pub(crate) fields: ColumnFields,
    pub(crate) class: Py<PyAny>,
    pub(crate) audits: Option<HeaderAudits>,
    pub(crate) projection: PyOnceLock<Py<PyAny>>,
}

impl std::fmt::Debug for HeaderColumn {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HeaderColumn")
            .finish_non_exhaustive()
    }
}

/// A natively parsed header column and what its Python record is built with.
#[derive(Debug)]
pub struct NativeColumnSpec {
    pub column: NativeColumn,
    /// The model file whose relative path the column's location names, and the location.
    pub location: Option<(Py<NativeModelFile>, LineColumnSpan)>,
}

impl HeaderColumn {
    /// Retype a builtin column without discarding its retained audit specs.
    pub fn enum_type(
        mut self,
        py: Python<'_>,
        scalar_type: Bound<'_, PyAny>,
        generated: Option<ParsedAudit<PyConfigNode>>,
    ) -> PyResult<Self> {
        if self.audits.is_none()
            && let ColumnFields::Native(spec) = &self.fields
        {
            let audits = spec
                .column
                .audits
                .iter()
                .map(|audit| authored_audit(py, audit))
                .collect::<PyResult<Vec<_>>>()?;
            let classes: ContractClasses<'_> = contract_classes(py)?.extract()?;
            self.audits = Some(HeaderAudits::new(
                py,
                &classes,
                &audits,
                &native_location(py, spec)?,
            )?);
        }
        if let Some(audits) = &mut self.audits {
            if let Some(generated) = generated {
                audits.push_native(generated);
            }
            match &mut self.fields {
                ColumnFields::Native(spec) => spec.column.column_type = scalar_type.extract()?,
                ColumnFields::Python(values) => values[1] = scalar_type.unbind(),
                ColumnFields::Projected => {
                    return Err(PyRuntimeError::new_err(
                        "enum column has no retained fields",
                    ));
                }
            }
            self.projection = PyOnceLock::new();
            return Ok(self);
        }
        Err(PyRuntimeError::new_err(
            "enum column has no retained audits",
        ))
    }

    pub fn retains_audits(&self) -> bool {
        self.audits.is_some() || matches!(self.fields, ColumnFields::Native(_))
    }

    pub fn audit_instances<'py>(&self, py: Python<'py>) -> PyResult<Vec<AuditInstance<'py>>> {
        if let Some(audits) = &self.audits {
            return Ok(audits.instances(py));
        }
        if let ColumnFields::Native(spec) = &self.fields {
            let audits = spec
                .column
                .audits
                .iter()
                .map(|audit| authored_audit(py, audit))
                .collect::<PyResult<Vec<_>>>()?;
            let classes: ContractClasses<'_> = contract_classes(py)?.extract()?;
            return Ok(
                HeaderAudits::new(py, &classes, &audits, &native_location(py, spec)?)?
                    .instances(py),
            );
        }
        self.field(py, "audits")?
            .try_iter()?
            .map(|value| value.map(AuditInstance::Python))
            .collect()
    }

    pub fn replaced<'py>(
        class: &Bound<'py, PyAny>,
        values: [Bound<'py, PyAny>; 8],
        keyword_order: [usize; 8],
    ) -> PyResult<Self> {
        if class.is(&record_class(class.py(), RecordId::SchemaColumn)?) {
            return Self::new(class, values);
        }
        let fields = PyDict::new(class.py());
        for index in keyword_order {
            fields.set_item(SCHEMA_COLUMN_FIELDS[index], &values[index])?;
        }
        Self::from_python(class.call((), Some(&fields))?)
    }

    pub fn field<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let index = || {
            SCHEMA_COLUMN_FIELDS
                .iter()
                .position(|field| *field == name)
                .ok_or_else(|| PyRuntimeError::new_err("unknown column field"))
        };
        if let Some(projected) = self.projection.get(py) {
            return field(projected.bind(py), name);
        }
        if name == AUDITS_KEY
            && let Some(audits) = &self.audits
        {
            return Ok(audits.project(py)?.into_any());
        }
        match &self.fields {
            ColumnFields::Python(values) => Ok(values[index()?].bind(py).clone()),
            ColumnFields::Native(spec) => native_value(py, spec, index()?),
            ColumnFields::Projected => field(&self.project(py)?, name),
        }
    }

    pub fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.projection
            .get_or_try_init(py, || {
                let mut values: [Bound<'py, PyAny>; 8] = match &self.fields {
                    ColumnFields::Python(values) => {
                        values.each_ref().map(|value| value.bind(py).clone())
                    }
                    ColumnFields::Native(spec) => native_values(py, spec)?,
                    ColumnFields::Projected => {
                        return Err(PyRuntimeError::new_err("missing column projection"));
                    }
                };
                if let Some(audits) = &self.audits {
                    values[5] = audits.project(py)?.into_any();
                }
                contract_object(self.class.bind(py), values).map(Bound::unbind)
            })
            .map(|value| value.bind(py).clone())
    }

    pub fn from_python(value: Bound<'_, PyAny>) -> PyResult<Self> {
        let py = value.py();
        let projection = PyOnceLock::new();
        let class = value.get_type().into_any().unbind();
        let _ = projection.set(py, value.unbind());
        Ok(Self {
            fields: ColumnFields::Projected,
            class,
            audits: None,
            projection,
        })
    }

    pub fn new<'py>(class: &Bound<'py, PyAny>, values: [Bound<'py, PyAny>; 8]) -> PyResult<Self> {
        if !class.is(&record_class(class.py(), RecordId::SchemaColumn)?) {
            return Self::from_python(contract_object(class, values)?);
        }
        checked_field_orders()?;
        Ok(Self {
            fields: ColumnFields::Python(values.map(Bound::unbind)),
            class: class.clone().unbind(),
            audits: None,
            projection: PyOnceLock::new(),
        })
    }

    /// A natively parsed column, whose Python record is built only when Python reads it.
    pub fn native(class: &Bound<'_, PyAny>, spec: NativeColumnSpec) -> PyResult<Self> {
        let py: Python<'_> = class.py();
        if !class.is(&record_class(py, RecordId::SchemaColumn)?) {
            return Self::from_python(contract_object(class, native_values(py, &spec)?)?);
        }
        checked_field_orders()?;
        Ok(Self {
            fields: ColumnFields::Native(Box::new(spec)),
            class: class.clone().unbind(),
            audits: None,
            projection: PyOnceLock::new(),
        })
    }
}

impl HeaderColumn {
    /// The original native column, if no retained audit edits changed its public contract.
    pub fn native_spec(&self) -> Option<&NativeColumnSpec> {
        if self.audits.is_some() {
            return None;
        }
        match &self.fields {
            ColumnFields::Native(spec) => Some(spec),
            ColumnFields::Python(_) | ColumnFields::Projected => None,
        }
    }
}

/// Retained native columns and their shared, lazy public tuple.
pub struct HeaderColumns {
    pub specs: Vec<HeaderColumn>,
    pub(crate) projection: PyOnceLock<Py<PyTuple>>,
}

impl std::fmt::Debug for HeaderColumns {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HeaderColumns")
            .finish_non_exhaustive()
    }
}

impl HeaderColumns {
    pub fn new(specs: Vec<HeaderColumn>) -> Self {
        Self {
            specs,
            projection: PyOnceLock::new(),
        }
    }

    pub fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.projection
            .get_or_try_init(py, || {
                let columns = self
                    .specs
                    .iter()
                    .map(|column| column.project(py))
                    .collect::<PyResult<Vec<_>>>()?;
                PyTuple::new(py, columns).map(Bound::unbind)
            })
            .map(|value| value.bind(py).clone())
    }
}

/// The Python classes the parsed metadata becomes, read from a Python mapping.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct ContractClasses<'py> {
    pub schema_column: Bound<'py, PyAny>,
    pub schema_audit_instance: Bound<'py, PyAny>,
    pub schema_dynamic_column_family: Bound<'py, PyAny>,
    pub source_location: Bound<'py, PyAny>,
    pub(crate) measurement_thresholds: Bound<'py, PyAny>,
    pub(crate) measurement_threshold_bound: Bound<'py, PyAny>,
    pub severities: Bound<'py, PyDict>,
    pub(crate) threshold_operators: Bound<'py, PyDict>,
}

impl ContractClasses<'_> {
    /// Whether these are the classes native header parsing builds by default.
    pub fn uses_default_classes(&self) -> PyResult<bool> {
        let py: Python<'_> = self.schema_column.py();
        let default: ContractClasses<'_> = contract_classes(py)?.extract()?;
        Ok(self.schema_column.is(&default.schema_column)
            && self
                .schema_audit_instance
                .is(&default.schema_audit_instance)
            && self.source_location.is(&default.source_location)
            && self
                .measurement_thresholds
                .is(&default.measurement_thresholds)
            && self
                .measurement_threshold_bound
                .is(&default.measurement_threshold_bound)
            && self.severities.is(&default.severities)
            && self.threshold_operators.is(&default.threshold_operators))
    }
}

/// The project-wide inputs every model's config build reads, converted once per compile.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeModelConfigBuilder {
    pub(crate) inputs: ConfigBuildInputs<HostHandle>,
    pub(crate) path_keys: Vec<String>,
    pub(crate) environment: Py<PyAny>,
}

/// One model's built config before Python wraps it in `CompileModelConfig`.
#[derive(Debug)]
pub struct BuiltConfig<'py> {
    pub values: Bound<'py, PyDict>,
    pub nodes: Option<BuiltConfigValues>,
    pub header_keys: Vec<String>,
    pub logical_schema: Option<Bound<'py, PyAny>>,
    pub layer_schema_configured: bool,
    pub logical_database: Option<Bound<'py, PyAny>>,
    pub retention: RetentionOverride,
    pub table_type: TableTypeOverride,
    pub reads: Vec<(&'static str, String)>,
}

/// A native build's outcome: the built config, or the config error it raises.
#[derive(Debug)]
pub enum ConfigBuild<'py> {
    Built(BuiltConfig<'py>),
    Rejected(Py<NativeConfigError>),
}

#[pymethods]
impl NativeModelConfigBuilder {
    #[new]
    pub fn new(
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
    pub(crate) fn model_config<'py>(
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

impl NativeModelConfigBuilder {
    pub(crate) fn host<'py>(&self, py: Python<'py>) -> PythonConfigHost<'py> {
        PythonConfigHost {
            environment: self.environment.bind(py).clone(),
        }
    }

    /// The selected path-default key, or the conflict error Python raises.
    pub fn selected_path_default(
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
    pub fn built<'py>(
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
    pub fn inherited_values<'py>(
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
    pub fn model_context_values<'py>(
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
    pub fn target_namespace<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        match &self.inputs.target_namespace {
            Some((database, schema)) => PyTuple::new(
                py,
                [config_object(py, database)?, config_object(py, schema)?],
            ),
            None => PyTuple::new(py, [py.None(), py.None()]),
        }
    }
}

/// The project facts every model's validation reads, captured once per compile.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeModelValidator {
    pub(crate) project: ProjectValidationFacts,
}

impl NativeModelValidator {
    /// The project's discovered resource names, which every reference resolves against.
    pub fn known_resources(&self) -> KnownResources<'_> {
        KnownResources {
            models: &self.project.models,
            seeds: &self.project.seeds,
            sources: &self.project.sources,
            functions: &self.project.functions,
            table_functions: &self.project.table_functions,
        }
    }
}

#[pymethods]
impl NativeModelValidator {
    /// The validator for the resources and materializations `inputs` discovered.
    #[staticmethod]
    pub fn from_inputs(inputs: Bound<'_, PyAny>, microbatch_concurrency: bool) -> PyResult<Self> {
        let names: ResourceNames = discovered_names(&inputs)?;
        let custom_materializations: HashSet<String> =
            each(&inputs, "materialization_files", |file| {
                native_field(file, "name")?.extract()
            })?
            .into_iter()
            .collect();
        let adapter: Option<String> = resolve_effective_adapter_name(
            &native_field(&inputs, "project_config")?,
            &native_field(&inputs, "local_config")?,
        )?
        .extract()?;
        Self::new(
            names,
            custom_materializations,
            microbatch_concurrency,
            adapter,
        )
    }

    /// The discovered model names references resolve against.
    pub(crate) fn model_names(&self) -> HashSet<String> {
        self.project.models.clone()
    }

    #[new]
    #[pyo3(signature = (names, custom_materializations, microbatch_concurrency, adapter=None))]
    pub(crate) fn new(
        names: ResourceNames,
        custom_materializations: HashSet<String>,
        microbatch_concurrency: bool,
        adapter: Option<String>,
    ) -> PyResult<Self> {
        let ResourceNames {
            models,
            seeds,
            sources,
            functions,
            table_functions,
        } = names;
        Ok(Self {
            project: ProjectValidationFacts {
                custom_materializations,
                microbatch_concurrency,
                models,
                seeds,
                sources,
                functions,
                table_functions,
                adapter,
            },
        })
    }

    /// `None` when accepted, the first error, or the first externally rejected reference.
    pub(crate) fn validate(
        &self,
        py: Python<'_>,
        values: Bound<'_, PyDict>,
        model: (String, String, String),
        facts: ModelFacts,
    ) -> PyResult<Py<PyAny>> {
        compiler_guard(|| {
            let (model_name, query_sql, relative_path) = model;
            let ModelFacts {
                references,
                declared_columns,
                retention_unmanaged,
                table_type_declared,
            } = facts;
            let references: Vec<ModelReference> = references
                .into_iter()
                .map(|(kind, name, externally_rejected)| ModelReference {
                    kind,
                    name,
                    externally_rejected,
                })
                .collect();
            let entries: Vec<(PyNode<'_>, PyNode<'_>)> = values
                .iter()
                .map(|(key, value)| (PyNode(key), PyNode(value)))
                .collect();
            let facts = ModelValidationFacts {
                model_name: &model_name,
                relative_path: &relative_path,
                references: &references,
                declared_columns: declared_columns.as_deref(),
                query_sql: &query_sql,
                retention_unmanaged,
                table_type_declared,
            };
            match validate_model_config(entries, &self.project, &facts) {
                Ok(()) => Ok(py.None()),
                Err(stop) => stop_object(py, stop),
            }
        })
    }

    /// Check references alone: `None`, the first error, or a rejected dbt reference's index.
    pub(crate) fn references(
        &self,
        py: Python<'_>,
        model: (String, String),
        references: Vec<(String, String, bool)>,
    ) -> PyResult<Py<PyAny>> {
        compiler_guard(|| {
            let (model_name, relative_path) = model;
            let references: Vec<ModelReference> = references
                .into_iter()
                .map(|(kind, name, externally_rejected)| ModelReference {
                    kind,
                    name,
                    externally_rejected,
                })
                .collect();
            let facts = ModelValidationFacts {
                model_name: &model_name,
                relative_path: &relative_path,
                references: &references,
                declared_columns: None,
                query_sql: "",
                retention_unmanaged: false,
                table_type_declared: false,
            };
            match validate_model_references(&self.project, &facts) {
                Ok(()) => Ok(py.None()),
                Err(stop) => stop_object(py, stop),
            }
        })
    }
}

/// The built tree and any Python-authored values opened by the record's readers.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct TypedConfigValues {
    pub built: Option<BuiltConfigValues>,
    pub(crate) read: Mutex<Vec<(String, Arc<PyConfigNode>)>>,
}
