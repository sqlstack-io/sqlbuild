//! The whole-number thresholds a Python rule declares.

use crate::python_rules::constants::PYTHON_RULES;
use crate::rules::models::{BuiltinOption, BuiltinOptionKind};

/// The options of the Python rule `code`, or none for any other code.
#[must_use]
pub fn python_rule_options(code: &str) -> Vec<BuiltinOption> {
    PYTHON_RULES
        .iter()
        .filter(|rule| rule.code == code)
        .flat_map(|rule| rule.options.iter())
        .map(|(name, default, description)| BuiltinOption {
            kind: BuiltinOptionKind::Integer {
                default: *default,
                minimum: 0,
            },
            name,
            description,
        })
        .collect()
}
