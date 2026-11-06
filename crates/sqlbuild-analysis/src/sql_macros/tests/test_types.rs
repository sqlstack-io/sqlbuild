use std::collections::HashMap;
use std::sync::Arc;

use sqlbuild_render::macro_arguments::models::ArgumentValue;

use crate::sql_macros::models::SqlMacroDefinition;

pub(super) struct ExpansionTestCase {
    pub(super) description: &'static str,
    pub(super) adapter: &'static str,
    pub(super) macros: &'static [(&'static str, &'static str)],
    pub(super) sql: &'static str,
    pub(super) expected_sql: &'static str,
}

pub(super) struct ExpansionErrorTestCase {
    pub(super) description: &'static str,
    pub(super) adapter: &'static str,
    pub(super) macros: &'static [(&'static str, &'static str)],
    pub(super) sql: &'static str,
    pub(super) expected_code: &'static str,
    pub(super) expected_message: &'static str,
}

pub(super) struct DefinitionErrorTestCase {
    pub(super) description: &'static str,
    pub(super) source: &'static str,
    pub(super) expected_code: &'static str,
    pub(super) expected_message: &'static str,
}

pub(super) struct DefinitionTestCase {
    pub(super) description: &'static str,
    pub(super) source: &'static str,
    pub(super) expected_labels: &'static [&'static str],
    pub(super) expected_form: &'static str,
    pub(super) expected_body_position: (usize, usize),
}

pub(super) struct CycleTestCase {
    pub(super) description: &'static str,
    pub(super) calls: &'static [(&'static str, &'static [&'static str])],
    pub(super) expected_cycle: Option<&'static [&'static str]>,
}

/// A host serving parsed macros and fixed declaration values, for one adapter.
pub(super) struct FakeHost {
    pub(super) definitions: HashMap<String, Arc<SqlMacroDefinition>>,
    pub(super) adapter: &'static str,
}

/// A model calling a Python macro with a SQL macro call among its arguments.
pub(super) struct PythonArgumentTestCase {
    pub(super) description: &'static str,
    pub(super) body: &'static str,
    pub(super) sql: &'static str,
    /// What a Python macro receives after the direct expansion: a line break after a `--` comment.
    pub(super) expected_suffix: &'static str,
    /// The argument values, given the SQL macro call's own expansion.
    pub(super) expected_arguments: fn(String) -> Vec<ArgumentValue>,
}

/// Argument parsing without Unicode name lookups, which these arguments never use.
pub(super) struct AsciiArguments;

pub(super) struct MacroDescriptionTestCase {
    pub(super) description: &'static str,
    pub(super) source: &'static str,
    pub(super) expected_description: &'static str,
}

pub(super) struct MacroQuoteErrorTestCase {
    pub(super) description: &'static str,
    pub(super) source: &'static str,
    pub(super) expected_message: &'static str,
    pub(super) expected_location: (usize, usize),
}

/// A text expanded with its typed calls, each shown as `macro returns: text`.
pub(super) struct TypedCallTestCase {
    pub(super) description: &'static str,
    pub(super) macros: &'static [(&'static str, &'static str)],
    pub(super) sql: &'static str,
    pub(super) expected_calls: &'static [&'static str],
}

/// A compiled text and its calls, with the analysis text the markers make of it.
pub(super) struct TypedAnalysisTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) calls: &'static [(usize, usize, Option<&'static str>)],
    pub(super) expected_sql: &'static str,
}

/// Two names that must stay equal.
pub(super) struct ReservedNameTestCase {
    pub(super) description: &'static str,
    pub(super) authored_name: &'static str,
    pub(super) expected_parsed_name: &'static str,
}

/// A query and the texts of its untyped calls, with the outputs each whole call names.
pub(super) struct UntypedOutputsTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) call_texts: &'static [&'static str],
    pub(super) expected_outputs: &'static [(&'static str, &'static str)],
}

/// A query calling `macros`, with the analysis text its expansion marks.
pub(super) struct ExpandedAnalysisTestCase {
    pub(super) description: &'static str,
    pub(super) macros: &'static [(&'static str, &'static str)],
    pub(super) sql: &'static str,
    pub(super) expected_sql: &'static str,
}

/// A query read under one adapter's lexical rules, with the SQL macro calls it holds.
pub(super) struct LexicalRulesCallsTestCase {
    pub(super) description: &'static str,
    pub(super) syntax: sqlbuild_sqltext::sql_scan::models::LexicalSyntax,
    pub(super) sql: &'static str,
    pub(super) expected_calls: &'static [&'static str],
}
