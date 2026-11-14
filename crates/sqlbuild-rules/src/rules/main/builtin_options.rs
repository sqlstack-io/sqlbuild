//! `builtin_options`: the options each built-in rule declares.

use crate::constants::{
    ALLOWED_QUALIFIERS_OPTION, DEFAULT_PUBLISHED_LAYERS, PUBLISHED_LAYERS_OPTION,
};
use crate::python_rules::main::python_rule_options::python_rule_options;
use crate::rules::models::{BuiltinOption, BuiltinOptionKind};

const fn boolean(name: &'static str, description: &'static str) -> BuiltinOption {
    BuiltinOption {
        kind: BuiltinOptionKind::Boolean,
        name,
        description,
    }
}

const fn published_layers() -> BuiltinOption {
    BuiltinOption {
        kind: BuiltinOptionKind::StringList(&DEFAULT_PUBLISHED_LAYERS),
        name: PUBLISHED_LAYERS_OPTION,
        description: "Model name layers that form each domain's published interface.",
    }
}

/// The declared options of the built-in rule `code`, in declaration order.
#[must_use]
pub fn builtin_options(code: &str) -> Vec<BuiltinOption> {
    match code {
        "SQBRCONTRACT102" => vec![boolean(
            "allow_numeric_indicators",
            "Accept numeric is_/has_/can_ indicator columns.",
        )],
        "SQBRCONTRACT103" => vec![
            boolean(
                "allow_date_for_at",
                "Accept DATE for columns ending in _at.",
            ),
            boolean(
                "allow_numeric_epoch",
                "Accept numeric epoch values for timestamp-named columns.",
            ),
            boolean(
                "allow_encoded_values",
                "Accept string or variant encoded timestamp values.",
            ),
        ],
        "SQBRCONTRACT104" => vec![
            boolean(
                "allow_timestamps",
                "Accept timestamp types for columns ending in _date.",
            ),
            boolean(
                "allow_encoded_values",
                "Accept string or variant encoded date values.",
            ),
        ],
        "SQBRGRAPH103" => vec![published_layers()],
        "SQBRPROJECT107" => vec![
            published_layers(),
            BuiltinOption {
                kind: BuiltinOptionKind::TableList,
                name: ALLOWED_QUALIFIERS_OPTION,
                description: "Qualifiers that do not count towards a domain's name shape, as \
                              { qualifier, reason } tables with an optional domain. qualifier \
                              is a case-sensitive glob such as \"[a-z][a-z][a-z]\"; \
                              { paths = [...], reason } instead leaves out published models \
                              whose file path matches a glob.",
            },
        ],
        "SQBRORCHESTRATION101" => vec![BuiltinOption {
            kind: BuiltinOptionKind::Integer {
                default: 3,
                minimum: 0,
            },
            name: "max_named_selectors",
            description: "Most nodes one job or sensor-condition selection may name one by one.",
        }],
        "SQBRTEST203" => vec![BuiltinOption {
            kind: BuiltinOptionKind::StringList(&[]),
            name: "allowed_tests",
            description: "Reviewed SQL test names that may assert only that empty inputs produce \
                          no rows; listed tests count toward min_tests_per_model.",
        }],
        _ => python_rule_options(code),
    }
}
