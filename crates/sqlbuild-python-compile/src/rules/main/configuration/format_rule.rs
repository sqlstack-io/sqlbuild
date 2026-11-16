//! `sqb rules show` text for one rule, with effective thresholds when a config is given.

use crate::rules::_helpers::configuration::catalogue::native_rules_config;
use crate::rules::_helpers::configuration::guidance::rule_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_rules::models::RulesConfig;
use sqlbuild_rules::rule_catalogue::main::format_rule_text::format_rule_text;
use sqlbuild_rules::rule_catalogue::models::RuleText;

/// `sqb rules show` text for one rule, with effective thresholds when a config is given.
#[pyfunction]
#[pyo3(signature = (*, rule, config = None))]
pub fn format_rule(rule: &Bound<'_, PyAny>, config: Option<Bound<'_, PyAny>>) -> PyResult<String> {
    let config: Option<Bound<'_, PyAny>> = config.filter(|config| !config.is_none());
    let native: Option<RulesConfig> = config.as_ref().map(native_rules_config).transpose()?;
    let text: RuleText = rule_text(rule, config.as_ref())?;
    compiler_guard(|| Ok(format_rule_text(&text, native.as_ref())))
}
