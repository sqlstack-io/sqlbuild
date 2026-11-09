//! The embedded `sqb` command table, read once into parser specs.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value as Json;

use crate::argv::constants::{
    COMMAND_TABLE, DBT_COMMAND_PLACEHOLDER, DBT_EXECUTION_TABLE, SUPPRESS_DEST,
};
use crate::argv::models::{
    ActionKind, ActionSpec, ChoiceHelp, Default, GroupSpec, HelpText, Nargs, ParserSpec, Value,
};

static ROOT: OnceLock<ParserSpec> = OnceLock::new();
static DBT_EXECUTION: OnceLock<ParserSpec> = OnceLock::new();

/// The root `sqb` parser.
pub(crate) fn root_parser() -> &'static ParserSpec {
    ROOT.get_or_init(|| {
        let table: Json = serde_json::from_str(COMMAND_TABLE).unwrap_or(Json::Null);
        TableReader { next_id: 0 }.parser(&table)
    })
}

/// One `sqb` subcommand's parser, such as `build` or `plan`.
pub(crate) fn command_parser(command: &str) -> Result<&'static ParserSpec, String> {
    root_parser()
        .actions
        .iter()
        .flat_map(|action| &action.parsers)
        .find(|(name, _)| name == command)
        .map(|(_, parser)| parser)
        .ok_or_else(|| format!("{command} command is unavailable"))
}

/// The `sqb dbt <command>` execution parser with the command named in its texts.
pub(crate) fn dbt_execution_parser(command: &str) -> ParserSpec {
    let mut parser: ParserSpec = DBT_EXECUTION
        .get_or_init(|| {
            let table: Json = serde_json::from_str(DBT_EXECUTION_TABLE).unwrap_or(Json::Null);
            TableReader { next_id: 0 }.parser(&table)
        })
        .clone();
    parser.prog = parser.prog.replace(DBT_COMMAND_PLACEHOLDER, command);
    parser.description = parser
        .description
        .map(|description| description.replace(DBT_COMMAND_PLACEHOLDER, command));
    parser
}

/// Reads parser specs, numbering every action across the whole tree.
struct TableReader {
    next_id: usize,
}

impl TableReader {
    fn parser(&mut self, json: &Json) -> ParserSpec {
        let groups: Vec<GroupSpec> = items(json, "groups")
            .iter()
            .map(|group| GroupSpec {
                title: text(group, "title"),
                description: text(group, "description"),
            })
            .collect();
        let mutex_groups: Vec<bool> = items(json, "mutex_groups")
            .iter()
            .map(|required| required.as_bool().unwrap_or(false))
            .collect();
        let actions: Vec<ActionSpec> = items(json, "actions")
            .iter()
            .map(|item| self.action(item))
            .collect();
        let mut option_actions: Vec<(String, usize)> = Vec::new();
        let mut option_index: HashMap<String, usize> = HashMap::new();
        for (index, action) in actions.iter().enumerate() {
            for option in &action.option_strings {
                option_actions.push((option.clone(), index));
                option_index.insert(option.clone(), index);
            }
        }
        ParserSpec {
            prog: text(json, "prog").unwrap_or_default(),
            usage: text(json, "usage"),
            description: text(json, "description"),
            epilog: text(json, "epilog"),
            groups,
            mutex_groups,
            actions,
            option_actions,
            option_index,
        }
    }

    fn action(&mut self, json: &Json) -> ActionSpec {
        let id: usize = self.next_id;
        self.next_id += 1;
        let dest: String = text(json, "dest").unwrap_or_default();
        let parsers: Vec<(String, ParserSpec)> = items(json, "parsers")
            .iter()
            .map(|item| {
                (
                    text(item, "name").unwrap_or_default(),
                    self.parser(&item["parser"]),
                )
            })
            .collect();
        let choices: Option<Vec<String>> = if parsers.is_empty() {
            json.get("choices")
                .and_then(Json::as_array)
                .map(|choices| choices.iter().map(plain_text).collect())
        } else {
            Some(parsers.iter().map(|(name, _)| name.clone()).collect())
        };
        ActionSpec {
            id,
            kind: kind(json),
            option_strings: items(json, "options").iter().map(plain_text).collect(),
            dest: (dest != SUPPRESS_DEST).then_some(dest),
            nargs: nargs(json.get("nargs").unwrap_or(&Json::Null)),
            constant: value(json.get("const").unwrap_or(&Json::Null)),
            default: default(json.get("default").unwrap_or(&Json::Null)),
            converter: text(json, "type"),
            choices,
            required: json
                .get("required")
                .and_then(Json::as_bool)
                .unwrap_or(false),
            help: help(json.get("help").unwrap_or(&Json::Null)),
            metavar: metavar(json.get("metavar").unwrap_or(&Json::Null)),
            group: json.get("group").and_then(Json::as_u64).unwrap_or(0) as usize,
            mutex_group: json
                .get("mutex_group")
                .and_then(Json::as_u64)
                .map(|group| group as usize),
            version: text(json, "version"),
            parsers,
            choices_help: items(json, "choices_help")
                .iter()
                .map(|choice| ChoiceHelp {
                    metavar: text(choice, "metavar").unwrap_or_default(),
                    help: help(choice.get("help").unwrap_or(&Json::Null)),
                })
                .collect(),
            removed_choices: items(json, "removed_choices")
                .iter()
                .map(|removed| {
                    (
                        text(removed, "value").unwrap_or_default(),
                        text(removed, "instead").unwrap_or_default(),
                    )
                })
                .collect(),
        }
    }
}

fn kind(json: &Json) -> ActionKind {
    match json.get("kind").and_then(Json::as_str).unwrap_or_default() {
        "store_true" => ActionKind::StoreTrue,
        "store_false" => ActionKind::StoreFalse,
        "store_const" => ActionKind::StoreConst,
        "append" => ActionKind::Append,
        "extend" => ActionKind::Extend,
        "help" => ActionKind::Help,
        "version" => ActionKind::Version,
        "parsers" => ActionKind::Parsers,
        _ => ActionKind::Store,
    }
}

fn nargs(json: &Json) -> Nargs {
    match json {
        Json::Number(_) => Nargs::Zero,
        Json::String(text) => match text.as_str() {
            "?" => Nargs::Optional,
            "*" => Nargs::ZeroOrMore,
            "+" => Nargs::OneOrMore,
            _ => Nargs::Parser,
        },
        _ => Nargs::Single,
    }
}

fn suppressed(json: &Json) -> bool {
    json.get("suppress")
        .and_then(Json::as_bool)
        .unwrap_or(false)
}

fn default(json: &Json) -> Default {
    if suppressed(json) {
        return Default::Suppress;
    }
    Default::Value(value(json))
}

fn help(json: &Json) -> HelpText {
    match json {
        Json::String(text) => HelpText::Text(text.clone()),
        Json::Null => HelpText::None,
        _ => HelpText::Suppress,
    }
}

fn metavar(json: &Json) -> Option<Option<String>> {
    match json {
        Json::String(text) => Some(Some(text.clone())),
        Json::Null => None,
        _ => Some(None),
    }
}

fn value(json: &Json) -> Value {
    match json {
        Json::Bool(flag) => Value::Bool(*flag),
        Json::Number(number) => Value::Int(number.to_string()),
        Json::String(text) => Value::Str(text.clone()),
        Json::Array(items) => Value::List(items.iter().map(value).collect()),
        Json::Object(_) => Value::EmptyDict,
        Json::Null => Value::None,
    }
}

fn items<'a>(json: &'a Json, key: &str) -> &'a [Json] {
    json.get(key)
        .and_then(Json::as_array)
        .map_or(&[], Vec::as_slice)
}

fn text(json: &Json, key: &str) -> Option<String> {
    json.get(key).and_then(Json::as_str).map(str::to_owned)
}

fn plain_text(json: &Json) -> String {
    json.as_str().unwrap_or_default().to_owned()
}
