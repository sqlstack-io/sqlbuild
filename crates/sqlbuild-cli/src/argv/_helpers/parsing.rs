//! `ArgumentParser.parse_known_args`, reproduced over the command table.

use std::collections::{HashMap, HashSet};

use sqlbuild_core::text::main::is_decimal_digit::is_decimal_digit;

use crate::argv::_helpers::formatting::{format_help, format_usage, format_version};
use crate::argv::_helpers::patterns::match_groups;
use crate::argv::_helpers::values::{action_name, argument_error};
use crate::argv::constants::{
    ERROR_PREFIX, ERROR_STRONG_STYLE, HELP_EXIT, OPTIONS_TERMINATOR, PARSER_ERROR_EXIT,
    PREFIX_CHARS, STYLE_RESET, USAGE_ERROR_EXITS,
};
use crate::argv::models::{
    ActionKind, ActionSpec, ArgvSettings, Default, HelpText, Namespace, Nargs, ParserSpec, Stream,
    UsageFailure, Value,
};
use crate::argv::types::ArgvHost;

/// One parse: the caller's settings and host, and everything written so far.
pub(crate) struct Run<'a> {
    pub(crate) settings: &'a ArgvSettings,
    pub(crate) host: &'a mut dyn ArgvHost,
    pub(crate) writes: Vec<(Stream, String)>,
    /// The `prog` of the deepest sub-command parser entered so far.
    pub(crate) command_prog: Option<String>,
    pub(crate) usage_failure: Option<UsageFailure>,
}

/// `SystemExit` with this status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Exit(pub(crate) i64);

/// What stops `_parse_known_args`: an `ArgumentError`'s text, or `SystemExit`.
pub(crate) enum Failure {
    Argument(String),
    Exit(Exit),
}

impl From<Exit> for Failure {
    fn from(exit: Exit) -> Self {
        Self::Exit(exit)
    }
}

/// One interpretation of an option string: action, option, separator and explicit argument.
#[derive(Debug, Clone)]
struct OptionTuple {
    action: Option<usize>,
    option_string: String,
    separator: Option<String>,
    explicit: Option<String>,
}

/// A parsed namespace and the strings no action consumed.
pub(crate) type KnownArgs = (Namespace, Vec<String>);

impl Run<'_> {
    /// `_print_message`: write non-empty text to `stream`.
    pub(crate) fn write(&mut self, stream: Stream, text: String) {
        if !text.is_empty() {
            self.writes.push((stream, text));
        }
    }

    /// `SqlbuildArgumentParser.error`: usage, then the coded message, to stderr; the usage exit.
    pub(crate) fn error(&mut self, parser: &ParserSpec, message: &str) -> Exit {
        let usage: String = format_usage(self.settings, parser);
        self.write(Stream::Stderr, usage);
        let prefix: String = if self.settings.use_color {
            format!("{ERROR_STRONG_STYLE}{ERROR_PREFIX}{STYLE_RESET}")
        } else {
            ERROR_PREFIX.to_owned()
        };
        self.write(
            Stream::Stderr,
            format!("{}: {prefix} {message}\n\n", parser.prog),
        );
        let command: String = self
            .command_prog
            .clone()
            .unwrap_or_else(|| parser.prog.clone());
        let status: i64 = USAGE_ERROR_EXITS
            .iter()
            .find(|(prog, _)| *prog == command)
            .map_or(PARSER_ERROR_EXIT, |(_, status)| *status);
        self.usage_failure = Some(UsageFailure {
            prog: command,
            message: message.to_owned(),
        });
        Exit(status)
    }

    /// `parse_known_args(args, namespace)`: the namespace and unrecognised strings, or the exit.
    pub(crate) fn parse_known_args(
        &mut self,
        parser: &ParserSpec,
        args: Vec<String>,
        namespace: Namespace,
    ) -> Result<KnownArgs, Exit> {
        match self.known_args(parser, args, namespace) {
            Ok(known) => Ok(known),
            Err(Failure::Argument(message)) => Err(self.error(parser, &message)),
            Err(Failure::Exit(exit)) => Err(exit),
        }
    }

    /// `parse_known_args` for a parser whose `error` raises: the `ArgumentError` text, not an exit.
    pub(crate) fn known_args(
        &mut self,
        parser: &ParserSpec,
        args: Vec<String>,
        mut namespace: Namespace,
    ) -> Result<KnownArgs, Failure> {
        for action in &parser.actions {
            let (Some(dest), Default::Value(default)) = (&action.dest, &action.default) else {
                continue;
            };
            if !namespace.has(dest) {
                namespace.set(dest, default.clone());
            }
        }
        let level: Level<'_, '_, '_> = Level::new(self, parser, args, namespace);
        let (mut namespace, mut extras): KnownArgs = level.parse()?;
        if let Some(unrecognized) = namespace.unrecognized.take() {
            extras.extend(unrecognized);
        }
        Ok((namespace, extras))
    }
}

/// The working state of one `_parse_known_args` call.
struct Level<'p, 'r, 'h> {
    run: &'r mut Run<'h>,
    parser: &'p ParserSpec,
    namespace: Namespace,
    arg_strings: Vec<String>,
    pattern: Vec<u8>,
    option_indices: HashMap<usize, Vec<OptionTuple>>,
    positionals: Vec<usize>,
    extras: Vec<String>,
    seen: HashSet<usize>,
    seen_non_default: HashSet<usize>,
}

impl<'p, 'r, 'h> Level<'p, 'r, 'h> {
    fn new(
        run: &'r mut Run<'h>,
        parser: &'p ParserSpec,
        arg_strings: Vec<String>,
        namespace: Namespace,
    ) -> Self {
        let mut pattern: Vec<u8> = Vec::with_capacity(arg_strings.len());
        let mut option_indices: HashMap<usize, Vec<OptionTuple>> = HashMap::new();
        let mut terminated: bool = false;
        for (index, arg) in arg_strings.iter().enumerate() {
            if terminated {
                pattern.push(b'A');
            } else if arg == OPTIONS_TERMINATOR {
                pattern.push(b'-');
                terminated = true;
            } else if let Some(tuples) = parse_optional(parser, arg) {
                option_indices.insert(index, tuples);
                pattern.push(b'O');
            } else {
                pattern.push(b'A');
            }
        }
        Self {
            run,
            parser,
            namespace,
            arg_strings,
            pattern,
            option_indices,
            positionals: parser
                .actions
                .iter()
                .enumerate()
                .filter(|(_, action)| action.option_strings.is_empty())
                .map(|(index, _)| index)
                .collect(),
            extras: Vec::new(),
            seen: HashSet::new(),
            seen_non_default: HashSet::new(),
        }
    }

    /// `_parse_known_args`: consume positionals and optionals alternately, then check requirements.
    fn parse(mut self) -> Result<KnownArgs, Failure> {
        let parser: &ParserSpec = self.parser;
        let max_option: Option<usize> = self.option_indices.keys().copied().max();
        let mut start: usize = 0;
        while max_option.is_some_and(|maximum| start <= maximum) {
            let next: usize = (start..)
                .find(|index| self.option_indices.contains_key(index))
                .unwrap_or(start);
            if start != next {
                let end: usize = self.consume_positionals(start)?;
                if end > start {
                    start = end;
                    continue;
                }
                start = end;
            }
            if !self.option_indices.contains_key(&start) {
                self.extras
                    .extend(self.arg_strings[start..next].iter().cloned());
                start = next;
            }
            start = self.consume_optional(start)?;
        }
        let stop: usize = self.consume_positionals(start)?;
        self.extras.extend(self.arg_strings[stop..].iter().cloned());
        let required: Vec<String> = parser
            .actions
            .iter()
            .filter(|action| action.required && !self.seen.contains(&action.id))
            .map(|action| action_name(action).unwrap_or_default())
            .collect();
        if !required.is_empty() {
            return Err(Failure::Argument(format!(
                "the following arguments are required: {}",
                required.join(", ")
            )));
        }
        for (group, required_group) in parser.mutex_groups.iter().enumerate() {
            let members: Vec<&ActionSpec> = parser
                .actions
                .iter()
                .filter(|action| action.mutex_group == Some(group))
                .collect();
            if *required_group
                && !members
                    .iter()
                    .any(|action| self.seen_non_default.contains(&action.id))
            {
                let names: Vec<String> = members
                    .iter()
                    .filter(|action| action.help != HelpText::Suppress)
                    .filter_map(|action| action_name(action))
                    .collect();
                return Err(Failure::Argument(format!(
                    "one of the arguments {} is required",
                    names.join(" ")
                )));
            }
        }
        Ok((self.namespace, self.extras))
    }

    fn ignored_explicit(&self, action: &ActionSpec, explicit: &str) -> Failure {
        Failure::Argument(argument_error(
            action,
            &format!("ignored explicit argument {}", self.run.host.repr(explicit)),
        ))
    }

    /// `consume_optional`: take the option at `start` and its arguments; the next index.
    fn consume_optional(&mut self, start: usize) -> Result<usize, Failure> {
        let parser: &ParserSpec = self.parser;
        let tuples: Vec<OptionTuple> = self.option_indices[&start].clone();
        if tuples.len() > 1 {
            let matches: Vec<&str> = tuples
                .iter()
                .map(|tuple| tuple.option_string.as_str())
                .collect();
            return Err(Failure::Argument(format!(
                "ambiguous option: {} could match {}",
                self.arg_strings[start],
                matches.join(", ")
            )));
        }
        let OptionTuple {
            mut action,
            mut option_string,
            mut separator,
            mut explicit,
        } = tuples[0].clone();
        let mut action_tuples: Vec<(usize, Vec<String>)> = Vec::new();
        let stop: usize;
        loop {
            let Some(index) = action else {
                self.extras.push(self.arg_strings[start].clone());
                return Ok(start + 1);
            };
            let spec: &ActionSpec = &parser.actions[index];
            let Some(explicit_arg) = explicit.clone() else {
                let first: usize = start + 1;
                let count: usize = match_argument(spec, &self.pattern[first..])?;
                stop = first + count;
                action_tuples.push((index, self.arg_strings[first..stop].to_vec()));
                break;
            };
            let count: usize = if spec.nargs == Nargs::Attached {
                1
            } else {
                match_argument(spec, b"A")?
            };
            let second: char = option_string.chars().nth(1).unwrap_or_default();
            if count == 1 {
                stop = start + 1;
                action_tuples.push((index, vec![explicit_arg]));
                break;
            }
            if count != 0 || second == PREFIX_CHARS || explicit_arg.is_empty() {
                return Err(self.ignored_explicit(spec, &explicit_arg));
            }
            if separator.as_deref().is_some_and(|sep| !sep.is_empty())
                || explicit_arg.starts_with(PREFIX_CHARS)
            {
                return Err(self.ignored_explicit(spec, &explicit_arg));
            }
            action_tuples.push((index, Vec::new()));
            let mut characters = explicit_arg.chars();
            let first: char = characters.next().unwrap_or_default();
            let rest: String = characters.collect();
            option_string = format!("{PREFIX_CHARS}{first}");
            let Some(next) = parser.option_index.get(&option_string) else {
                self.extras.push(format!("{PREFIX_CHARS}{explicit_arg}"));
                stop = start + 1;
                break;
            };
            action = Some(*next);
            (separator, explicit) = if rest.is_empty() {
                (None, None)
            } else if let Some(value) = rest.strip_prefix('=') {
                (Some("=".to_owned()), Some(value.to_owned()))
            } else {
                (Some(String::new()), Some(rest))
            };
        }
        for (index, args) in action_tuples {
            self.take_action(index, args)?;
        }
        Ok(stop)
    }

    /// `consume_positionals`: match as many positionals as possible from `start`.
    fn consume_positionals(&mut self, mut start: usize) -> Result<usize, Failure> {
        let parser: &ParserSpec = self.parser;
        let counts: Vec<usize> = match_partial(parser, &self.positionals, &self.pattern[start..]);
        let consumed: Vec<usize> = self.positionals.drain(..counts.len()).collect();
        for (index, count) in consumed.into_iter().zip(counts) {
            let action: &ActionSpec = &parser.actions[index];
            let mut args: Vec<String> = self.arg_strings[start..start + count].to_vec();
            let strip: bool = if action.nargs == Nargs::Parser {
                self.pattern.get(start) == Some(&b'-')
            } else {
                self.pattern[start..start + count].contains(&b'-')
            };
            if strip && let Some(position) = args.iter().position(|arg| arg == OPTIONS_TERMINATOR) {
                args.remove(position);
            }
            start += count;
            self.take_action(index, args)?;
        }
        Ok(start)
    }

    /// `take_action`: convert the strings, check mutual exclusion and call the action.
    fn take_action(&mut self, index: usize, args: Vec<String>) -> Result<(), Failure> {
        let parser: &ParserSpec = self.parser;
        let action: &ActionSpec = &parser.actions[index];
        self.seen.insert(action.id);
        let values: Option<Value> = self
            .run
            .action_values(action, &args)
            .map_err(Failure::Argument)?;
        if !action.option_strings.is_empty() || !args.is_empty() {
            self.seen_non_default.insert(action.id);
            let conflict: Option<&ActionSpec> = action.mutex_group.and_then(|group| {
                parser.actions.iter().find(|other| {
                    other.mutex_group == Some(group)
                        && other.id != action.id
                        && self.seen_non_default.contains(&other.id)
                })
            });
            if let Some(other) = conflict {
                return Err(Failure::Argument(argument_error(
                    action,
                    &format!(
                        "not allowed with argument {}",
                        action_name(other).unwrap_or_default()
                    ),
                )));
            }
        }
        match values {
            Some(values) => self.call_action(action, values),
            None => Ok(()),
        }
    }

    /// `Action.__call__` for each action class the command table uses.
    fn call_action(&mut self, action: &ActionSpec, values: Value) -> Result<(), Failure> {
        let dest: &str = action.dest.as_deref().unwrap_or_default();
        match action.kind {
            ActionKind::Store => self.namespace.set(dest, values),
            ActionKind::StoreConst | ActionKind::StoreTrue | ActionKind::StoreFalse => {
                self.namespace.set(dest, action.constant.clone());
            }
            ActionKind::Append | ActionKind::Extend => {
                let mut items: Vec<Value> = match self.namespace.get(dest) {
                    Some(Value::List(items)) => items.clone(),
                    _ => Vec::new(),
                };
                match (action.kind, values) {
                    (ActionKind::Extend, Value::List(values)) => items.extend(values),
                    (_, value) => items.push(value),
                }
                self.namespace.set(dest, Value::List(items));
            }
            ActionKind::Help => {
                let help: String = format_help(self.run.settings, self.parser);
                self.run.write(Stream::Stdout, help);
                return Err(Failure::Exit(Exit(HELP_EXIT)));
            }
            ActionKind::Version => {
                let version: String = self.run.host.program_version();
                let text: String = format_version(self.run.settings, action, &version);
                self.run.write(Stream::Stdout, text);
                return Err(Failure::Exit(Exit(HELP_EXIT)));
            }
            ActionKind::Parsers => self.call_parsers(action, values)?,
        }
        Ok(())
    }

    /// `_SubParsersAction.__call__`: parse the rest into a fresh namespace and copy it over.
    fn call_parsers(&mut self, action: &ActionSpec, values: Value) -> Result<(), Failure> {
        let Value::List(values) = values else {
            return Ok(());
        };
        let strings: Vec<String> = values
            .into_iter()
            .map(|value| match value {
                Value::Str(text) => text,
                _ => String::new(),
            })
            .collect();
        let Some((name, rest)) = strings.split_first() else {
            return Ok(());
        };
        if let Some(dest) = &action.dest {
            self.namespace.set(dest, Value::Str(name.clone()));
        }
        let Some((_, subparser)) = action.parsers.iter().find(|(choice, _)| choice == name) else {
            return Ok(());
        };
        self.run.command_prog = Some(subparser.prog.clone());
        let (subnamespace, leftover) =
            self.run
                .parse_known_args(subparser, rest.to_vec(), Namespace::default())?;
        for (name, value) in subnamespace.entries {
            self.namespace.set(&name, value);
        }
        if !leftover.is_empty() {
            self.namespace
                .unrecognized
                .get_or_insert_with(Vec::new)
                .extend(leftover);
        }
        Ok(())
    }
}

/// `_match_argument`: how many strings an optional consumes, or its arity error.
fn match_argument(action: &ActionSpec, pattern: &[u8]) -> Result<usize, Failure> {
    if let Some(counts) = match_groups(&[action], pattern) {
        return Ok(counts[0]);
    }
    let message: &str = match action.nargs {
        Nargs::Single => "expected one argument",
        Nargs::Optional => "expected at most one argument",
        Nargs::OneOrMore => "expected at least one argument",
        _ => "expected 0 arguments",
    };
    Err(Failure::Argument(argument_error(action, message)))
}

/// `_match_arguments_partial`: argument counts for as many leading positionals as match.
fn match_partial(parser: &ParserSpec, positionals: &[usize], pattern: &[u8]) -> Vec<usize> {
    for length in (1..=positionals.len()).rev() {
        let actions: Vec<&ActionSpec> = positionals[..length]
            .iter()
            .map(|index| &parser.actions[*index])
            .collect();
        if let Some(mut counts) = match_groups(&actions, pattern) {
            let end: usize = counts.iter().sum();
            if pattern.get(end) == Some(&b'O') {
                while counts.last() == Some(&0) {
                    counts.pop();
                }
            }
            return counts;
        }
    }
    Vec::new()
}

/// `_parse_optional`: how a string starting with `-` reads as options, or `None` for an argument.
fn parse_optional(parser: &ParserSpec, arg: &str) -> Option<Vec<OptionTuple>> {
    if !arg.starts_with(PREFIX_CHARS) {
        return None;
    }
    if let Some(index) = parser.option_index.get(arg) {
        return Some(vec![OptionTuple {
            action: Some(*index),
            option_string: arg.to_owned(),
            separator: None,
            explicit: None,
        }]);
    }
    if arg.chars().count() == 1 {
        return None;
    }
    if let Some((option, explicit)) = arg.split_once('=')
        && let Some(index) = parser.option_index.get(option)
    {
        return Some(vec![OptionTuple {
            action: Some(*index),
            option_string: option.to_owned(),
            separator: Some("=".to_owned()),
            explicit: Some(explicit.to_owned()),
        }]);
    }
    let tuples: Vec<OptionTuple> = option_tuples(parser, arg);
    if !tuples.is_empty() {
        return Some(tuples);
    }
    if looks_negative(arg) || arg.contains(' ') {
        return None;
    }
    Some(vec![OptionTuple {
        action: None,
        option_string: arg.to_owned(),
        separator: None,
        explicit: None,
    }])
}

/// `_get_option_tuples` with `allow_abbrev`: every option the string abbreviates.
fn option_tuples(parser: &ParserSpec, arg: &str) -> Vec<OptionTuple> {
    let (prefix, separator, explicit) = match arg.split_once('=') {
        Some((prefix, explicit)) => (prefix, Some("=".to_owned()), Some(explicit.to_owned())),
        None => (arg, None, None),
    };
    let second: char = arg.chars().nth(1).unwrap_or_default();
    let mut tuples: Vec<OptionTuple> = Vec::new();
    if second == PREFIX_CHARS {
        for (option, index) in &parser.option_actions {
            if option.starts_with(prefix) {
                tuples.push(OptionTuple {
                    action: Some(*index),
                    option_string: option.clone(),
                    separator: separator.clone(),
                    explicit: explicit.clone(),
                });
            }
        }
        return tuples;
    }
    let short_end: usize = arg.char_indices().nth(2).map_or(arg.len(), |(end, _)| end);
    let (short_prefix, short_explicit) = arg.split_at(short_end);
    for (option, index) in &parser.option_actions {
        if option == short_prefix {
            tuples.push(OptionTuple {
                action: Some(*index),
                option_string: option.clone(),
                separator: Some(String::new()),
                explicit: Some(short_explicit.to_owned()),
            });
        } else if option.starts_with(prefix) {
            tuples.push(OptionTuple {
                action: Some(*index),
                option_string: option.clone(),
                separator: separator.clone(),
                explicit: explicit.clone(),
            });
        }
    }
    tuples
}

/// `_negative_number_matcher.match` (`^-\d+$|^-\d*\.\d+$`): a negative-number-like string.
fn looks_negative(arg: &str) -> bool {
    let rest: &str = &arg[1..];
    let body: &str = rest.strip_suffix('\n').unwrap_or(rest);
    let integer: bool = !body.is_empty() && body.chars().all(is_decimal_digit);
    let decimal: bool = body.split_once('.').is_some_and(|(whole, fraction)| {
        whole.chars().all(is_decimal_digit)
            && !fraction.is_empty()
            && fraction.chars().all(is_decimal_digit)
    });
    integer || decimal
}
