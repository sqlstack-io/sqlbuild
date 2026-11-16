use std::cell::RefCell;

use crate::templates::errors::TemplateError;
use crate::templates::models::{ContextValue, Scalar, StringExpansion, TemplateFailure};
use crate::templates::types::TemplateHost;

/// A template value standing in for the Python objects the bindings evaluate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Value {
    Null,
    Bool(bool),
    Text(String),
    /// A mapping, which compares by its `str()` but cannot be interpolated.
    Opaque,
}

/// Fixed variables, environment and context, recording reads like the Python resolver.
pub(super) struct TestHost {
    pub(super) variables: Vec<(&'static str, Value)>,
    pub(super) environment: Vec<(&'static str, &'static str)>,
    pub(super) context: Vec<(&'static str, Option<&'static str>)>,
    pub(super) reads: RefCell<Vec<String>>,
}

impl TemplateHost for TestHost {
    type Value = Value;

    fn variable(&self, name: &str) -> Result<Option<Value>, TemplateFailure> {
        Ok(self
            .variables
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone()))
    }

    fn environment(&self, name: &str) -> Result<Option<Value>, TemplateFailure> {
        self.reads.borrow_mut().push(format!("ENV:{name}"));
        Ok(self
            .environment
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| Value::Text((*value).to_owned())))
    }

    fn context(&self, name: &str) -> Result<ContextValue<Value>, TemplateFailure> {
        self.reads.borrow_mut().push(format!("CTX:{name}"));
        Ok(match self.context.iter().find(|(key, _)| *key == name) {
            None => ContextValue::Unknown,
            Some((_, None)) => ContextValue::Unavailable,
            Some((_, Some(value))) => ContextValue::Value(Value::Text((*value).to_owned())),
        })
    }

    fn text(&self, text: &str) -> Result<Value, TemplateFailure> {
        Ok(Value::Text(text.to_owned()))
    }

    fn boolean(&self, flag: bool) -> Result<Value, TemplateFailure> {
        Ok(Value::Bool(flag))
    }

    fn null(&self) -> Value {
        Value::Null
    }

    fn scalar(&self, value: &Value) -> Result<Scalar, TemplateFailure> {
        Ok(match value {
            Value::Null => Scalar::Null,
            Value::Bool(flag) => Scalar::Bool(*flag),
            Value::Text(text) => Scalar::Text(text.clone()),
            Value::Opaque => Scalar::Text("{'a': 1}".to_owned()),
        })
    }

    fn render(&self, value: &Value, label: &str) -> Result<String, TemplateFailure> {
        match self.scalar(value)? {
            Scalar::Null => Ok(String::new()),
            Scalar::Bool(flag) => Ok(flag.to_string()),
            Scalar::Text(_) if *value == Value::Opaque => Err(TemplateFailure::Invalid(
                TemplateError::Message(format!("{label} is an object")),
            )),
            Scalar::Text(text) => Ok(text),
        }
    }

    fn label(&self) -> &str {
        "model config"
    }
}

/// One template string, its expansion and the reads it records.
pub(super) struct TemplateTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) expected: Result<StringExpansion<Value>, TemplateFailure>,
    pub(super) expected_reads: &'static [&'static str],
}

/// A malformed template string and the most characters per input character its scan may visit.
pub(super) struct TemplateScanTestCase {
    pub(super) description: &'static str,
    pub(super) text: String,
    pub(super) expected_max_steps_per_character: usize,
}
