//! Parse `sqb dbt plan|run|build` arguments as their `argparse` parser did.

use std::collections::BTreeSet;

use sqlbuild_core::dbt_cli::models::DbtParsedFlags;

use crate::argv::_helpers::dbt_execution::{PlainHost, parsed_flags};
use crate::argv::_helpers::parsing::{Failure, KnownArgs, Run};
use crate::argv::_helpers::table::dbt_execution_parser;
use crate::argv::constants::DBT_EXECUTION_SEPARATOR;
use crate::argv::models::{
    ArgvSettings, DbtExecutionFailure, DbtExecutionOutcome, Namespace, ParserSpec,
};

/// Parse the declared flags before the first `--`; everything after it is passed to dbt.
pub fn parse_dbt_execution_arguments(
    command: &str,
    args: &[String],
    settings: ArgvSettings,
) -> DbtExecutionOutcome {
    let (head, tail): (&[String], &[String]) =
        match args.iter().position(|arg| arg == DBT_EXECUTION_SEPARATOR) {
            Some(separator) => (&args[..separator], &args[separator + 1..]),
            None => (args, &[]),
        };
    let parser: ParserSpec = dbt_execution_parser(command);
    let mut host: PlainHost = PlainHost;
    let mut run: Run<'_> = Run {
        settings: &settings,
        host: &mut host,
        writes: Vec::new(),
        command_prog: None,
        usage_failure: None,
        given: BTreeSet::new(),
    };
    let known: Result<KnownArgs, Failure> =
        run.known_args(&parser, head.to_vec(), Namespace::default());
    let parsed: Result<DbtParsedFlags, DbtExecutionFailure> = match known {
        Ok((namespace, unknown)) if unknown.is_empty() => {
            Ok(parsed_flags(&namespace, tail.to_vec()))
        }
        Ok((_, unknown)) => Err(DbtExecutionFailure::Unrecognized(unknown)),
        Err(Failure::Argument(message)) => Err(DbtExecutionFailure::Argument(format!(
            "{}: {message}",
            parser.prog
        ))),
        Err(Failure::Exit(exit)) => Err(DbtExecutionFailure::Exit(exit.0)),
    };
    DbtExecutionOutcome {
        writes: run.writes,
        parsed,
    }
}
