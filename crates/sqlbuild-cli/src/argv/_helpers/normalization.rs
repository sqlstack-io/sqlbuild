//! `parse_cli_invocation`'s rules after `parse_known_args`: passthrough, selectors and validation.

use crate::argv::_helpers::parsing::{Exit, KnownArgs, Run};
use crate::argv::constants::{
    COMMAND_DEST, CONCURRENCY_COMMANDS, CONCURRENCY_ENV_VAR, CURSOR_COMMANDS, DBT_COMMAND,
    DBT_DEBUG_COMMAND, DBT_PASSTHROUGH_COMMANDS, DBT_PASSTHROUGH_SEPARATOR, DBT_VERBOSE_OPTION,
    DBT_VERBOSE_OPTIONS, DEBUG_OPTION, INT_CONVERTER, JANITOR_COMMAND, NO_COLOR_OPTION,
    POSITIONAL_SELECT_COMMANDS, RENAME_COMMAND, SCOPE_COMMAND, SCOPE_DEFAULT_PAGE_SIZE,
    SCOPE_GLOBAL_SUMMARY, SCOPE_KIND_DEST, TEST_COMMAND,
};
use crate::argv::models::{Namespace, ParserSpec, Value};

impl Run<'_> {
    /// Apply the post-parse rules to `namespace`, or exit through `parser.error`.
    pub(crate) fn normalized(
        &mut self,
        parser: &ParserSpec,
        (mut namespace, unknown): KnownArgs,
    ) -> Result<Namespace, Exit> {
        let command: Option<String> = text(&namespace.value(COMMAND_DEST));
        let command: &str = command.as_deref().unwrap_or_default();
        let dbt_command: Option<String> = text(&namespace.value("dbt_command"));
        if command == DBT_COMMAND
            && dbt_command
                .as_deref()
                .is_some_and(|name| DBT_PASSTHROUGH_COMMANDS.contains(&name))
        {
            let mut passthrough: Vec<Value> = Vec::new();
            let mut started: bool = false;
            for arg in unknown {
                if arg == DBT_PASSTHROUGH_SEPARATOR {
                    started = true;
                } else if arg == NO_COLOR_OPTION && !started {
                    namespace.set("no_color", Value::Bool(true));
                    continue;
                } else if arg == DEBUG_OPTION && !started {
                    namespace.set("debug", Value::Bool(true));
                    continue;
                }
                passthrough.push(Value::Str(arg));
            }
            namespace.set("dbt_args", Value::List(passthrough));
        } else {
            let mut unknown: Vec<String> = unknown;
            if POSITIONAL_SELECT_COMMANDS.contains(&command) {
                let mut select: Vec<Value> = list(&namespace.value("positional_select"));
                if !unknown.is_empty() && !unknown.iter().any(|arg| arg.starts_with('-')) {
                    select.extend(unknown.drain(..).map(Value::Str));
                }
                select.extend(list(&namespace.value("select")));
                namespace.set("select", Value::List(select));
            }
            if !unknown.is_empty() {
                return Err(self.error(
                    parser,
                    &format!("unrecognized arguments: {}", unknown.join(" ")),
                ));
            }
        }
        let verbose: bool =
            truthy(&namespace.value("verbose")) || truthy(&namespace.value("debug"));
        namespace.set("verbose", Value::Bool(verbose));
        let mut dbt_args: Vec<Value> = list(&namespace.value("dbt_args"));
        if truthy(&namespace.value("debug"))
            && command == DBT_COMMAND
            && dbt_command.as_deref() != Some(DBT_DEBUG_COMMAND)
            && !dbt_args.iter().any(
                |arg| matches!(arg, Value::Str(text) if DBT_VERBOSE_OPTIONS.contains(&text.as_str())),
            )
        {
            dbt_args.push(Value::Str(DBT_VERBOSE_OPTION.to_owned()));
            namespace.set("dbt_args", Value::List(dbt_args));
        }
        if CONCURRENCY_COMMANDS.contains(&command) {
            let concurrency: Value = self
                .concurrency(&namespace.value("concurrency"))
                .map_err(|message| self.error(parser, &message))?;
            namespace.set("concurrency", concurrency);
        }
        if CURSOR_COMMANDS.contains(&command)
            && let Some(message) = cursor_error(|name| namespace.value(name) != Value::None)
        {
            return Err(self.error(parser, message));
        }
        if command == RENAME_COMMAND
            && let Some(message) = rename_error(&namespace)
        {
            let rename: &ParserSpec = subcommand_parser(parser, RENAME_COMMAND).unwrap_or(parser);
            return Err(self.error(rename, &message));
        }
        if command == SCOPE_COMMAND
            && let Some(message) = scope_error(parser, &namespace)
        {
            return Err(self.error(parser, message));
        }
        if command == TEST_COMMAND
            && truthy(&namespace.value("inspect"))
            && (truthy(&namespace.value("json")) || namespace.value("json_output") != Value::None)
        {
            return Err(self.error(
                parser,
                "test --inspect cannot be combined with --json or --json-output",
            ));
        }
        if let Some(message) = since_error(&namespace) {
            return Err(self.error(parser, message));
        }
        if command == JANITOR_COMMAND
            && namespace.value("as_target") != Value::None
            && truthy(&namespace.value("auto_approve"))
        {
            return Err(self.error(
                parser,
                "janitor --as is inspection only and cannot be combined with --auto-approve",
            ));
        }
        Ok(namespace)
    }

    /// `resolve_env_default_concurrency`: the explicit value, else `SQLBUILD_CONCURRENCY`.
    fn concurrency(&mut self, explicit: &Value) -> Result<Value, String> {
        if *explicit != Value::None {
            if integer(explicit) < 1 {
                return Err("--concurrency must be >= 1".to_owned());
            }
            return Ok(explicit.clone());
        }
        let raw: String = match std::env::var_os(CONCURRENCY_ENV_VAR) {
            None => return Ok(Value::None),
            Some(raw) => raw.to_string_lossy().into_owned(),
        };
        if raw.is_empty() {
            return Ok(Value::None);
        }
        let Ok(value) = self.host.convert(INT_CONVERTER, &raw) else {
            return Err(format!("{CONCURRENCY_ENV_VAR} must be an integer"));
        };
        if integer(&value) < 1 {
            return Err(format!("{CONCURRENCY_ENV_VAR} must be >= 1"));
        }
        Ok(value)
    }
}

/// The command line and authored job settings share the same cursor-family restriction.
pub(crate) fn cursor_error(present: impl Fn(&str) -> bool) -> Option<&'static str> {
    let untyped = ["start_cursor", "end_cursor"]
        .iter()
        .any(|name| present(name));
    let typed = [
        "start_cursor_ts",
        "end_cursor_ts",
        "start_cursor_int",
        "end_cursor_int",
    ]
    .iter()
    .any(|name| present(name));
    (untyped && typed).then_some(
        "--start-cursor/--end-cursor cannot be combined with the -ts or -int cursor flags",
    )
}

/// The first rule `--since` and `--explain` break, on the commands that accept them.
fn since_error(namespace: &Namespace) -> Option<&'static str> {
    match namespace.value("since") {
        Value::None if truthy(&namespace.value("explain")) => Some("--explain requires --since"),
        Value::Str(reference) if reference.trim().is_empty() => {
            Some("--since requires a git ref, such as origin/main or @base")
        }
        _ => None,
    }
}

/// `_validate_scope_args`: the first rule the scope options break.
fn scope_error(parser: &ParserSpec, namespace: &Namespace) -> Option<&'static str> {
    let present = |name: &str| namespace.value(name) != Value::None;
    let has_target: bool = present("scope_target");
    if has_target == present("scope_at") {
        return Some("scope requires exactly one of TARGET or --at PATH");
    }
    if present("scope_as_path") && !has_target {
        return Some("scope --as-path requires TARGET");
    }
    let dependency_depth: i128 = integer(&namespace.value("scope_dependency_depth"));
    let page_size: i128 = integer(&namespace.value("scope_page_size"));
    let browse: bool = present("scope_browse");
    if (browse || present("scope_list"))
        && (present("scope_as_path")
            || present("scope_explain")
            || truthy(&namespace.value("scope_include_nearby"))
            || dependency_depth != 0)
    {
        return Some(
            "scope --browse/--list cannot be combined with --as-path, --explain, \
             --include-nearby, or --dependency-depth",
        );
    }
    if browse
        && (present("scope_defined_under")
            || truthy(&namespace.value("scope_kind"))
            || present("scope_match")
            || truthy(&namespace.value("scope_used_only"))
            || namespace.value("scope_globals") != Value::Str(SCOPE_GLOBAL_SUMMARY.to_owned())
            || page_size != i128::from(SCOPE_DEFAULT_PAGE_SIZE))
    {
        return Some(
            "scope --browse is folder-only; use --list for declaration filters and pagination",
        );
    }
    if page_size < 1 {
        return Some("scope --page-size must be positive");
    }
    if integer(&namespace.value("scope_nearby_depth")) < 0 || dependency_depth < 0 {
        return Some("scope depth values must be nonnegative");
    }
    if let Some(explain) = text(&namespace.value("scope_explain")) {
        let (prefix, qualified) = explain.split_once(':').unwrap_or((explain.as_str(), ""));
        if !declaration_kinds(parser).contains(&prefix) || qualified.is_empty() {
            return Some("scope --explain requires a declaration-qualified identity");
        }
    }
    None
}

/// The `DeclarationKind` values: the choices of `sqb scope --kind`.
fn declaration_kinds(parser: &ParserSpec) -> Vec<&str> {
    parser
        .actions
        .iter()
        .flat_map(|action| &action.parsers)
        .filter(|(name, _)| name == SCOPE_COMMAND)
        .flat_map(|(_, scope)| &scope.actions)
        .filter(|action| action.dest.as_deref() == Some(SCOPE_KIND_DEST))
        .flat_map(|action| action.choices.iter().flatten())
        .map(String::as_str)
        .collect()
}

fn text(value: &Value) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.clone()),
        _ => None,
    }
}

fn list(value: &Value) -> Vec<Value> {
    match value {
        Value::List(items) => items.clone(),
        _ => Vec::new(),
    }
}

/// Python truthiness of a parsed value.
fn truthy(value: &Value) -> bool {
    match value {
        Value::None | Value::EmptyDict => false,
        Value::Bool(flag) => *flag,
        Value::Int(text) => integer(&Value::Int(text.clone())) != 0,
        Value::Str(text) => !text.is_empty(),
        Value::List(items) => !items.is_empty(),
        Value::Path(_) | Value::Host(_) => true,
    }
}

/// An integer value, saturated beyond `i128`.
fn integer(value: &Value) -> i128 {
    let Value::Int(text) = value else {
        return 0;
    };
    text.parse::<i128>().unwrap_or(if text.starts_with('-') {
        i128::MIN
    } else {
        i128::MAX
    })
}

/// The usage error of a `sqb rename` without `--from-file` and without one full pair.
fn rename_error(namespace: &Namespace) -> Option<String> {
    if namespace.value("refactor_from_file") != Value::None
        || namespace.value("refactor_new_name") != Value::None
    {
        return None;
    }
    let missing: &str = if namespace.value("refactor_target") == Value::None {
        "TARGET, NEW_NAME"
    } else {
        "NEW_NAME"
    };
    Some(format!("the following arguments are required: {missing}"))
}

/// The parser of sub-command `name` below `parser`.
fn subcommand_parser<'a>(parser: &'a ParserSpec, name: &str) -> Option<&'a ParserSpec> {
    parser
        .actions
        .iter()
        .flat_map(|action| &action.parsers)
        .find(|(command, _)| command == name)
        .map(|(_, spec)| spec)
}
