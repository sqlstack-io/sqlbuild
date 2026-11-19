use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as Json, json};
use sqlbuild_analysis::job_settings::constants::JOB_SETTINGS;
use sqlbuild_analysis::job_settings::models::{CommandLineValues, SettingScope};

use crate::argv::_helpers::table::command_parser;
use crate::argv::main::command_settings_arguments::command_settings_arguments;
use crate::argv::main::job_run_arguments::job_run_arguments;
use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::{ActionKind, ActionSpec, Nargs};
use crate::argv::models::{ArgvOutcome, ArgvSettings, Value};
use crate::argv::tests::test_types::FakeHost;

/// Parse `argv` at the recorded 100-column width without colour.
pub(super) fn parse(argv: &[&str]) -> ArgvOutcome {
    let argv: Vec<String> = argv.iter().map(|arg| (*arg).to_owned()).collect();
    let settings: ArgvSettings = ArgvSettings {
        columns: 100,
        use_color: false,
    };
    parse_cli_arguments(&argv, Vec::new(), settings, &mut FakeHost)
}

/// A list of string values.
pub(super) fn strings(items: &[&str]) -> Value {
    Value::List(
        items
            .iter()
            .map(|item| Value::Str((*item).to_owned()))
            .collect(),
    )
}

/// Owned copies of command-line strings.
pub(super) fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// Each destination of `command`'s parser with all its option strings, in the parser's order.
pub(super) fn parser_options(command: &str) -> Vec<(String, Vec<String>)> {
    let actions: Vec<&ActionSpec> = command_parser(command)
        .map(|parser| parser.actions.iter().collect())
        .unwrap_or_default();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let dests: Vec<&str> = actions
        .iter()
        .filter_map(|action| action.dest.as_deref())
        .filter(|dest| seen.insert(dest))
        .collect();
    dests
        .into_iter()
        .map(|dest| {
            let strings: Vec<String> = actions
                .iter()
                .filter(|action| action.dest.as_deref() == Some(dest))
                .flat_map(|action| action.option_strings.iter().cloned())
                .collect();
            (dest.to_owned(), strings)
        })
        .collect()
}

/// Each job setting with its listed options, in the table's order.
pub(super) fn table_options() -> Vec<(String, Vec<String>)> {
    JOB_SETTINGS
        .iter()
        .map(|spec| {
            let strings = spec
                .options
                .iter()
                .map(|option| (*option).to_owned())
                .collect();
            (spec.name.to_owned(), strings)
        })
        .collect()
}

/// A value the option of `action` accepts, as a job would write it.
fn sample(action: &ActionSpec) -> Json {
    let converter: Option<&str> = action.converter.as_deref();
    let candidates: [(bool, Json); 6] = [
        (
            action.kind == ActionKind::StoreTrue || action.nargs == Nargs::Attached,
            json!(true),
        ),
        (action.kind == ActionKind::StoreFalse, json!(false)),
        (
            action.choices.is_some(),
            json!(action.choices.as_ref().and_then(|choices| choices.first())),
        ),
        (
            matches!(converter, Some("int" | "_positive_integer")),
            json!(3),
        ),
        (converter == Some("parse_cli_vars"), json!({"region": "eu"})),
        (true, json!("2026-01-01T00:00:00+00:00")),
    ];
    candidates
        .into_iter()
        .find_map(|(applies, value)| applies.then_some(value))
        .unwrap_or_default()
}

/// The job settings of `scope` and `whole_run` that do not render as one of their listed options.
pub(super) fn unrendered_settings(scope: SettingScope, whole_run: bool) -> Vec<&'static str> {
    JOB_SETTINGS
        .iter()
        .filter(|spec| spec.scope == scope && spec.whole_run == whole_run)
        .filter(|spec| {
            !rendered_option(spec.name)
                .is_some_and(|option| spec.options.contains(&option.as_str()))
        })
        .map(|spec| spec.name)
        .collect()
}

/// The option a `build` job setting `name` renders as, from a sample value.
fn rendered_option(name: &str) -> Option<String> {
    let parser = command_parser("build").ok()?;
    let action = parser
        .actions
        .iter()
        .find(|action| action.dest.as_deref() == Some(name))?;
    let settings = BTreeMap::from([(name.to_owned(), sample(action))]);
    let rendered: Vec<String> = command_settings_arguments("build", &settings).ok()?;
    rendered
        .first()
        .and_then(|argument| argument.split('=').next())
        .map(str::to_owned)
}

/// The job settings whose `command_line` disagrees with what the build parser's options give.
pub(super) fn command_line_mismatches() -> Vec<&'static str> {
    let actions: Vec<&ActionSpec> = command_parser("build")
        .map(|parser| parser.actions.iter().collect())
        .unwrap_or_default();
    JOB_SETTINGS
        .iter()
        .filter(|spec| {
            let kinds: Vec<ActionKind> = actions
                .iter()
                .filter(|action| action.dest.as_deref() == Some(spec.name))
                .map(|action| action.kind)
                .collect();
            let only =
                |kind: ActionKind| !kinds.is_empty() && kinds.iter().all(|seen| *seen == kind);
            let expected: CommandLineValues = [
                (only(ActionKind::StoreTrue), CommandLineValues::OnlyTrue),
                (only(ActionKind::StoreFalse), CommandLineValues::OnlyFalse),
            ]
            .into_iter()
            .find_map(|(applies, values)| applies.then_some(values))
            .unwrap_or(CommandLineValues::Any);
            spec.command_line != expected
        })
        .map(|spec| spec.name)
        .collect()
}

/// The node settings that are not whole-run but still reach a job run's build arguments.
pub(super) fn job_run_leaks() -> Vec<&'static str> {
    let parser = command_parser("build").ok();
    JOB_SETTINGS
        .iter()
        .filter(|spec| spec.scope == SettingScope::Node && !spec.whole_run)
        .filter(|spec| {
            let value: Json = parser
                .and_then(|parser| {
                    parser
                        .actions
                        .iter()
                        .find(|action| action.dest.as_deref() == Some(spec.name))
                })
                .map(sample)
                .unwrap_or_default();
            let settings = BTreeMap::from([(spec.name.to_owned(), value)]);
            job_run_arguments("orders_job", &settings).map_or(true, |arguments| {
                arguments != ["--select", "job:orders_job"]
            })
        })
        .map(|spec| spec.name)
        .collect()
}
