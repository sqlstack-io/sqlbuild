//! The `CompileInputError` Python raises for a template error in the context `label`.

use crate::model_config::_helpers::config::config_templates;
use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::templates::errors::TemplateError;

/// The `CompileInputError` Python raises for a template error in the context `label`.
#[inline]
pub fn template_error(error: &TemplateError, label: &str) -> ConfigError {
    config_templates::template_error(error, label)
}
