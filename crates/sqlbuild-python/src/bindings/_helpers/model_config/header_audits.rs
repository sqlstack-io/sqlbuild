//! Retained audit contracts; Python-authored factory results remain opaque values.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::types::{PyString, PyStringMethods, PyTuple};
use sqlbuild_attachments::audits::models::ArgumentValue;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};
use sqlbuild_discovery::model_files::main::config_value::config_value;
use sqlbuild_discovery::model_files::models::NativeAudit;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::header_metadata::constants::{
    ACCEPTED_VALUES_ARGUMENT, ACCEPTED_VALUES_AUDIT,
};
use sqlbuild_model_config::header_metadata::models::ParsedAudit;
use sqlbuild_sqltext::compiler::models::AuthoredValue;
use std::sync::Arc;

use crate::bindings::_helpers::attachments::argument_values::{
    argument_pairs, config_argument_value,
};
use crate::bindings::_helpers::model_config::config_nodes::{
    PyConfigNode, config_node, config_object,
};
use crate::bindings::_helpers::model_config::contract_classes::contract_classes;
use crate::bindings::_helpers::model_config::header_metadata::{
    ContractClasses, HeaderValue, audit_instance, thresholds,
};
use crate::bindings::_helpers::native_host::header_projection::AuditProjection;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::result_output::integration_values::json_value;
use crate::bindings::_helpers::sqltext::authored_values::authored_value_to_python;

/// Native-built specs never become Python audit records just to be read by attachment.
pub(crate) enum AuditInstance<'py> {
    Native(Arc<ParsedAudit<PyConfigNode>>, Bound<'py, PyAny>),
    Python(Bound<'py, PyAny>),
}

impl<'py> AuditInstance<'py> {
    /// Equality with the builtin generated enum audit, without constructing argument containers.
    pub(crate) fn accepts_enum(&self, values: &[PyConfigNode]) -> PyResult<bool> {
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

    pub(crate) fn meta_present(&self) -> PyResult<bool> {
        match self {
            Self::Native(spec, _) => Ok(spec
                .meta
                .as_ref()
                .is_some_and(|value| !matches!(value, ConfigNode::Null))),
            Self::Python(value) => Ok(!field(value, "meta")?.is_none()),
        }
    }

    pub(crate) fn threshold_policy(&self) -> PyResult<(bool, bool)> {
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

    pub(crate) fn field(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
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

    pub(crate) fn argument(&self, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
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

    pub(crate) fn arguments(&self) -> PyResult<Vec<(String, ArgumentValue)>> {
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

    pub(crate) fn meta(&self) -> PyResult<Option<JsonValue>> {
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

/// Native discovery facts keep their authored values rather than building audit records.
pub(crate) fn authored_audit(
    py: Python<'_>,
    spec: &NativeAudit,
) -> PyResult<ParsedAudit<PyConfigNode>> {
    let value = |value: &AuthoredValue| match config_value(value) {
        Some(value) => Ok(value),
        None => config_node(authored_value_to_python(py, value)?.bind(py)),
    };
    Ok(ParsedAudit {
        definition_name: ConfigNode::Str(spec.definition_name.clone()),
        arguments: spec
            .arguments
            .iter()
            .map(|(key, item)| Ok((ConfigNode::Str(key.clone()), value(item)?)))
            .collect::<PyResult<_>>()?,
        name: spec.name.clone().map(ConfigNode::Str),
        description: spec.description.clone().map(ConfigNode::Str),
        severity: spec.severity.clone().map(ConfigNode::Str),
        run_scope: spec.run_scope.clone().map(ConfigNode::Str),
        always_run: spec.always_run.map(ConfigNode::Bool),
        thresholds: spec.thresholds,
        minimum_samples: spec.minimum_samples.as_ref().map(value).transpose()?,
        evidence_limit: spec.evidence_limit.as_ref().map(value).transpose()?,
        meta: spec.meta.as_ref().map(value).transpose()?,
    })
}

/// The parsed specs and genuine Python audit objects of one model or column.
pub(crate) struct HeaderAudits {
    specs: Vec<ParsedAudit<PyConfigNode>>,
    native_generated: Vec<ParsedAudit<PyConfigNode>>,
    public: AuditProjection,
}

impl HeaderAudits {
    pub(crate) fn new<N: HeaderValue>(
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

    pub(crate) fn extend(&mut self, generated: Vec<Bound<'_, PyAny>>) {
        self.public
            .generated
            .extend(generated.into_iter().map(Bound::unbind));
    }

    pub(crate) fn push_native(&mut self, spec: ParsedAudit<PyConfigNode>) {
        self.native_generated.push(spec);
    }

    pub(crate) fn len(&self) -> usize {
        self.specs.len() + self.public.generated.len() + self.native_generated.len()
    }

    pub(crate) fn native_specs(&self) -> &[ParsedAudit<PyConfigNode>] {
        if self.public.custom.is_some() {
            &[]
        } else {
            &self.specs
        }
    }

    pub(crate) fn python_specs<'py>(&self, py: Python<'py>) -> Vec<Bound<'py, PyAny>> {
        self.public
            .custom
            .iter()
            .flatten()
            .chain(&self.public.generated)
            .map(|spec| spec.bind(py).clone())
            .collect()
    }

    pub(crate) fn instances<'py>(&self, py: Python<'py>) -> Vec<AuditInstance<'py>> {
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

    pub(crate) fn project<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
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

    pub(crate) fn key_rows(&self, py: Python<'_>) -> PyResult<Vec<(String, Option<String>)>> {
        let mut rows = Vec::with_capacity(self.len());
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

/// Preserve parsed leaves without rebuilding their public argument dictionaries.
pub(crate) fn retain_audit<N: HeaderValue>(
    py: Python<'_>,
    spec: &ParsedAudit<N>,
) -> PyResult<ParsedAudit<PyConfigNode>> {
    let optional = |node: &Option<N>| node.as_ref().map(|node| node.retained(py)).transpose();
    Ok(ParsedAudit {
        definition_name: spec.definition_name.retained(py)?,
        arguments: spec
            .arguments
            .iter()
            .map(|(key, value)| Ok((key.retained(py)?, value.retained(py)?)))
            .collect::<PyResult<_>>()?,
        name: optional(&spec.name)?,
        description: optional(&spec.description)?,
        severity: optional(&spec.severity)?,
        run_scope: optional(&spec.run_scope)?,
        always_run: optional(&spec.always_run)?,
        thresholds: spec.thresholds,
        minimum_samples: optional(&spec.minimum_samples)?,
        evidence_limit: optional(&spec.evidence_limit)?,
        meta: optional(&spec.meta)?,
    })
}

/// Read metadata without projecting retained containers; opaque user values keep Python semantics.
fn config_json_value(py: Python<'_>, value: &PyConfigNode) -> PyResult<Option<JsonValue>> {
    Ok(Some(match value {
        ConfigNode::Null => JsonValue::Null,
        ConfigNode::Bool(value) => JsonValue::Bool(*value),
        ConfigNode::Int(value) => JsonValue::Integer(JsonInteger::from(*value)),
        ConfigNode::Float(value) => JsonValue::Float(*value),
        ConfigNode::Str(value) => JsonValue::String(value.clone()),
        ConfigNode::List(items) | ConfigNode::Tuple(items) => {
            let mut result: Vec<JsonValue> = Vec::with_capacity(items.len());
            for item in items {
                let Some(value) = config_json_value(py, item)? else {
                    return Ok(None);
                };
                result.push(value);
            }
            JsonValue::Array(result)
        }
        ConfigNode::Map(entries) => {
            let mut result: Vec<(String, JsonValue)> = Vec::with_capacity(entries.len());
            for (key, item) in entries {
                let ConfigNode::Str(key) = key else {
                    return json_value(&config_object(py, value)?);
                };
                let Some(value) = config_json_value(py, item)? else {
                    return Ok(None);
                };
                result.push((key.clone(), value));
            }
            JsonValue::Object(result)
        }
        ConfigNode::Host(_) => return json_value(&config_object(py, value)?),
    }))
}
