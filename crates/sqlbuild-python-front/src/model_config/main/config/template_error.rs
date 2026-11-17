//! The `CompileInputError` Python raises for a template error in the context `label`.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::templates::errors::TemplateError;
use sqlbuild_model_config::templates::main::template_error_message::template_error_message;

/// The `CompileInputError` Python raises for a template error in the context `label`.
pub fn template_error(error: &TemplateError, label: &str) -> ConfigError {
    ConfigError::compile(template_error_message(error, label))
}
