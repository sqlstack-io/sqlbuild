use std::collections::BTreeMap;

use serde_json::Value as Json;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use sqlbuild_core::dbt_cli::models::DbtParsedFlags;

use crate::argv::models::{DbtExecutionFailure, TypeFailure, Value};
use crate::argv::types::ArgvHost;

/// Integer conversion and a fixed version, standing in for the interpreter.
pub(super) struct FakeHost;

impl ArgvHost for FakeHost {
    fn convert(&mut self, _converter: &str, text: &str) -> Result<Value, TypeFailure> {
        text.parse::<i64>()
            .map(|number| Value::Int(number.to_string()))
            .map_err(|_| TypeFailure::Invalid)
    }

    fn repr(&self, text: &str) -> String {
        python_str_repr(text)
    }

    fn program_version(&mut self) -> String {
        "1.2.3".to_owned()
    }
}

/// An accepted command line and the value one attribute ends with.
pub(super) struct ParsedValueTestCase {
    pub(super) description: &'static str,
    pub(super) argv: &'static [&'static str],
    pub(super) attribute: &'static str,
    pub(super) expected_value: Value,
}

/// A command line that ends the parse, its exit status and last write.
pub(super) struct ExitTestCase {
    pub(super) description: &'static str,
    pub(super) argv: &'static [&'static str],
    pub(super) expected_exit: i64,
    pub(super) expected_last_write: &'static str,
}

/// A help request and fragments its text must and must not contain.
pub(super) struct HelpTextTestCase {
    pub(super) description: &'static str,
    pub(super) argv: &'static [&'static str],
    pub(super) expected_present: &'static [&'static str],
    pub(super) expected_absent: &'static [&'static str],
}

/// Text, width and indent for `textwrap.wrap`, and the expected lines.
pub(super) struct WrapTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) width: usize,
    pub(super) indent: &'static str,
    pub(super) expected_lines: &'static [&'static str],
}

/// `sqb dbt` execution arguments and the parse result they produce.
pub(super) struct DbtExecutionTestCase {
    pub(super) description: &'static str,
    pub(super) args: &'static [&'static str],
    pub(super) expected_parsed: Result<DbtParsedFlags, DbtExecutionFailure>,
}

/// Authored settings and the build namespace or validation failure they produce.
pub(super) struct JobSettingsTestCase {
    pub(super) description: &'static str,
    pub(super) settings: BTreeMap<String, Json>,
    pub(super) expected_values: Vec<(&'static str, Value)>,
    pub(super) expected_empty: bool,
}

/// Mutually exclusive authored options rejected before a snapshot is published.
pub(super) struct JobSettingsRejectionTestCase {
    pub(super) description: &'static str,
    pub(super) settings: BTreeMap<String, Json>,
    pub(super) expected_error: &'static str,
}
