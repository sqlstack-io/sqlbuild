//! Parse `sqb`'s argv as its `argparse` parser and `parse_cli_invocation` did.

use std::collections::BTreeSet;

use crate::argv::_helpers::parsing::Run;
use crate::argv::_helpers::table::root_parser;
use crate::argv::models::{ArgvOutcome, ArgvSettings, Namespace, ParserSpec, Value};
use crate::argv::types::ArgvHost;

/// Parse `argv` onto a namespace holding `defaults`; the result, and everything to print.
pub fn parse_cli_arguments(
    argv: &[String],
    defaults: Vec<(String, Value)>,
    settings: ArgvSettings,
    host: &mut dyn ArgvHost,
) -> ArgvOutcome {
    let parser: &ParserSpec = root_parser();
    let mut run: Run<'_> = Run {
        settings: &settings,
        host,
        writes: Vec::new(),
        command_prog: None,
        usage_failure: None,
        given: BTreeSet::new(),
    };
    let namespace: Namespace = Namespace {
        entries: defaults,
        unrecognized: None,
    };
    let parsed: Result<Namespace, i64> = run
        .parse_known_args(parser, argv.to_vec(), namespace)
        .and_then(|known| run.normalized(parser, known))
        .map_err(|exit| exit.0);
    ArgvOutcome {
        writes: run.writes,
        parsed,
        usage_failure: run.usage_failure,
        given: run.given,
    }
}
