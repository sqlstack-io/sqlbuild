//! Relation names a statement's code text creates, drops, alters or loads.

use std::collections::BTreeSet;

use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_core::text::main::python_strip::python_strip;

use crate::ddl_effects::_helpers::matching::{
    bounded_phrase, only_whitespace, phrase, phrase_then_whitespace, repeated_modifiers,
    skip_whitespace, whitespace, word, word_boundary,
};
use crate::ddl_effects::_helpers::names::{UnquotedPart, part_name_key, qualified_name};
use crate::ddl_effects::constants::{
    ALTER_FOLLOWING_WORDS, ALTER_SESSION_SET, ALTER_SESSION_UNSET, COMPUTE_ONLY_STATEMENTS,
    COPY_INTO, CREATE_FOLLOWING_WORDS, CREATE_SCHEMA_IF_NOT_EXISTS, DROP_FOLLOWING_WORDS,
    FROM_WORD, IF_EXISTS, IF_NOT_EXISTS, INTO, NAME_RESOLUTION_SETTINGS, NEUTRAL_OBJECT_KINDS,
    NEUTRAL_OBJECT_LEADS, NEUTRAL_OBJECT_MODIFIERS, NEUTRAL_SESSION_PARAMETERS, OR_REPLACE,
    QUERY_STATEMENTS, READ_OR_DATA_STATEMENTS, RELATION_DDL_STATEMENTS, RELATION_KIND_MODIFIERS,
    RELATION_KINDS, RENAMING_ALTERS, SET_SCOPE_MODIFIERS, SET_WORD, STATEMENT_SEPARATOR,
    TRANSACTION_END_STATEMENTS,
};
use crate::ddl_effects::models::StatementMetadataEffect;

/// Which statement form named the relation, deciding what may follow its name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    Create,
    Drop,
    Alter,
    CopyInto,
    SelectInto,
    SecondRelation,
}

/// A matched relation name's key and the text after the name.
struct NamedRelation<'a> {
    key: String,
    rest: &'a str,
}

/// The effect of every relation in the statement possibly changing.
pub(crate) fn all_relations() -> StatementMetadataEffect {
    StatementMetadataEffect {
        invalidates_all: true,
        ..StatementMetadataEffect::default()
    }
}

/// A qualified name at `start` followed by whitespace, `(` or the end.
fn named_relation(text: &str, start: usize) -> Option<NamedRelation<'_>> {
    let name = qualified_name(text, start, UnquotedPart::Statement)?;
    let rest: &str = &text[name.end..];
    let delimited: bool = rest
        .chars()
        .next()
        .is_none_or(|next| next == '(' || python_isspace(next));
    delimited.then(|| NamedRelation {
        key: part_name_key(name.last_part),
        rest,
    })
}

/// `LEAD\s++` modifiers `(TABLE|VIEW)\s++` and an optional `guard\s++` before the name.
fn guarded_relation<'a>(text: &'a str, lead: &str, guard: &[&str]) -> Option<NamedRelation<'a>> {
    let after_lead: usize = whitespace(text, word(text, 0, lead)?)?;
    let after_modifiers: usize = repeated_modifiers(text, after_lead, RELATION_KIND_MODIFIERS);
    let name_start: usize = phrase_then_whitespace(text, after_modifiers, RELATION_KINDS)?;
    phrase(text, name_start, guard)
        .and_then(|end| whitespace(text, end))
        .and_then(|guarded| named_relation(text, guarded))
        .or_else(|| named_relation(text, name_start))
}

fn copy_into_relation(text: &str) -> Option<NamedRelation<'_>> {
    named_relation(text, whitespace(text, phrase(text, 0, COPY_INTO)?)?)
}

fn second_relation(rest: &str) -> Option<NamedRelation<'_>> {
    let clause: usize = whitespace(rest, 0)?;
    named_relation(rest, phrase_then_whitespace(rest, clause, RENAMING_ALTERS)?)
}

/// The first `\bINTO\s++name` in the statement.
fn select_into_relation(text: &str) -> Option<NamedRelation<'_>> {
    text.char_indices()
        .filter(|(index, _)| word(text, *index, INTO).is_some() && word_boundary(text, *index))
        .find_map(|(index, _)| named_relation(text, whitespace(text, index + INTO.len())?))
}

/// Whether `keyword` appears in `text` as a whole word.
fn has_keyword(text: &str, keyword: &str) -> bool {
    text.char_indices().any(|(index, _)| {
        word(text, index, keyword)
            .is_some_and(|end| word_boundary(text, index) && word_boundary(text, end))
    })
}

/// Whether `rest` is text the form allows after the relation name.
fn allowed_following(form: Form, rest: &str) -> bool {
    match form {
        Form::Create => {
            let start: usize = skip_whitespace(rest, 0);
            start == rest.len()
                || rest[start..].starts_with('(')
                || bounded_phrase(rest, start, CREATE_FOLLOWING_WORDS).is_some()
        }
        Form::Drop => {
            let start: usize = skip_whitespace(rest, 0);
            let end: usize = DROP_FOLLOWING_WORDS
                .iter()
                .find_map(|words| phrase(rest, start, words))
                .unwrap_or(start);
            only_whitespace(rest, end)
        }
        Form::Alter => whitespace(rest, 0)
            .is_some_and(|start| bounded_phrase(rest, start, ALTER_FOLLOWING_WORDS).is_some()),
        Form::CopyInto | Form::SelectInto => whitespace(rest, 0)
            .is_some_and(|start| bounded_phrase(rest, start, &[FROM_WORD]).is_some()),
        Form::SecondRelation => only_whitespace(rest, 0),
    }
}

/// The relation keys a matched name names, or `None` when what follows is ambiguous.
fn unambiguous_names(form: Form, relation: Option<NamedRelation<'_>>) -> Option<BTreeSet<String>> {
    let relation: NamedRelation<'_> = relation?;
    if !allowed_following(form, relation.rest) {
        return None;
    }
    let renaming: bool = form == Form::Alter
        && whitespace(relation.rest, 0)
            .is_some_and(|start| bounded_phrase(relation.rest, start, RENAMING_ALTERS).is_some());
    let mut names: BTreeSet<String> = BTreeSet::from([relation.key]);
    if renaming {
        names.extend(unambiguous_names(
            Form::SecondRelation,
            second_relation(relation.rest),
        )?);
    }
    Some(names)
}

fn named_effect(names: BTreeSet<String>) -> StatementMetadataEffect {
    StatementMetadataEffect {
        relation_names: names,
        ..StatementMetadataEffect::default()
    }
}

fn select_into_effect(statement: &str) -> StatementMetadataEffect {
    if !has_keyword(statement, INTO) {
        return StatementMetadataEffect::default();
    }
    unambiguous_names(Form::SelectInto, select_into_relation(statement))
        .map_or_else(all_relations, named_effect)
}

fn relation_ddl_effect(statement: &str) -> StatementMetadataEffect {
    [
        (
            Form::Create,
            guarded_relation(statement, "CREATE", IF_NOT_EXISTS),
        ),
        (Form::Drop, guarded_relation(statement, "DROP", IF_EXISTS)),
        (Form::Alter, guarded_relation(statement, "ALTER", IF_EXISTS)),
        (Form::CopyInto, copy_into_relation(statement)),
    ]
    .into_iter()
    .find_map(|(form, relation)| unambiguous_names(form, relation))
    .map_or_else(all_relations, named_effect)
}

fn creates_schema_if_missing(statement: &str) -> bool {
    let Some(name_start) = phrase(statement, 0, CREATE_SCHEMA_IF_NOT_EXISTS)
        .and_then(|end| whitespace(statement, end))
    else {
        return false;
    };
    let name_length: usize = statement[name_start..]
        .chars()
        .take_while(|character| *character != STATEMENT_SEPARATOR && !python_isspace(*character))
        .map(char::len_utf8)
        .sum();
    name_length > 0 && only_whitespace(statement, name_start + name_length)
}

fn is_neutral_object(statement: &str) -> bool {
    let Some(after_lead) = phrase_then_whitespace(statement, 0, NEUTRAL_OBJECT_LEADS) else {
        return false;
    };
    let after_replace: usize =
        phrase_then_whitespace(statement, after_lead, &[OR_REPLACE]).unwrap_or(after_lead);
    let kind_start: usize = repeated_modifiers(statement, after_replace, NEUTRAL_OBJECT_MODIFIERS);
    bounded_phrase(statement, kind_start, NEUTRAL_OBJECT_KINDS).is_some()
}

/// The end of the session parameter value at `start`: a backslash-free literal or one bare word.
fn session_value_end(text: &str, start: usize) -> Option<usize> {
    let rest: &str = &text[start..];
    let Some(quoted) = rest.strip_prefix('\'') else {
        let length: usize = rest
            .chars()
            .take_while(|character| !python_isspace(*character))
            .map(char::len_utf8)
            .sum();
        let value: &str = &rest[..length];
        return (length > 0 && !value.contains(['\'', ',', '='])).then_some(start + length);
    };
    let mut position: usize = 0;
    loop {
        let close: usize = quoted[position..].find('\'')? + position;
        if quoted[position..close].contains('\\') {
            return None;
        }
        if !quoted[close + 1..].starts_with('\'') {
            return Some(start + 1 + close + 1);
        }
        position = close + 2;
    }
}

/// Whether the statement only sets or resets one session parameter that name resolution ignores.
fn sets_neutral_session_parameter(statement: &str) -> bool {
    if let Some(end) = phrase(statement, 0, ALTER_SESSION_UNSET) {
        return whitespace(statement, end)
            .and_then(|start| bounded_phrase(statement, start, NEUTRAL_SESSION_PARAMETERS))
            .is_some_and(|end| only_whitespace(statement, end));
    }
    let Some(start) =
        phrase(statement, 0, ALTER_SESSION_SET).and_then(|end| whitespace(statement, end))
    else {
        return false;
    };
    let Some(after_name) = bounded_phrase(statement, start, NEUTRAL_SESSION_PARAMETERS) else {
        return false;
    };
    let equals: usize = skip_whitespace(statement, after_name);
    if !statement[equals..].starts_with('=') {
        return false;
    }
    session_value_end(statement, skip_whitespace(statement, equals + 1))
        .is_some_and(|end| only_whitespace(statement, end))
}

/// Whether the statement is `SET [SESSION|LOCAL|GLOBAL] search_path|schema ...`.
fn sets_name_resolution(statement: &str) -> bool {
    let Some(after_set) = phrase_then_whitespace(statement, 0, &[SET_WORD]) else {
        return false;
    };
    let start: usize = repeated_modifiers(statement, after_set, SET_SCOPE_MODIFIERS);
    bounded_phrase(statement, start, NAME_RESOLUTION_SETTINGS).is_some()
}

/// Whether the statement changes only session state that relations and name resolution ignore.
fn is_session_only(statement: &str) -> bool {
    bounded_phrase(statement, 0, COMPUTE_ONLY_STATEMENTS)
        .is_some_and(|end| whitespace(statement, end).is_some())
        || sets_neutral_session_parameter(statement)
}

/// The effect of one reading of the statement's code text; `None` code changes everything.
pub(crate) fn code_effect(code: Option<&str>) -> StatementMetadataEffect {
    let Some(code) = code else {
        return all_relations();
    };
    let statement: &str = python_strip(python_strip(code).trim_end_matches(STATEMENT_SEPARATOR))
        .trim_start_matches('(');
    let first_length: usize = statement
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .count();
    if first_length == 0 || statement.contains(STATEMENT_SEPARATOR) {
        return all_relations();
    }
    let keyword: String = statement[..first_length].to_ascii_uppercase();
    if QUERY_STATEMENTS.contains(&keyword.as_str()) {
        return select_into_effect(statement);
    }
    if sets_name_resolution(statement) {
        return all_relations();
    }
    if READ_OR_DATA_STATEMENTS.contains(&keyword.as_str()) {
        return StatementMetadataEffect::default();
    }
    if TRANSACTION_END_STATEMENTS.contains(&keyword.as_str()) {
        return StatementMetadataEffect {
            ends_transaction: true,
            ..StatementMetadataEffect::default()
        };
    }
    if is_session_only(statement) {
        return StatementMetadataEffect::default();
    }
    if !RELATION_DDL_STATEMENTS.contains(&keyword.as_str()) {
        return all_relations();
    }
    if creates_schema_if_missing(statement) || is_neutral_object(statement) {
        return StatementMetadataEffect::default();
    }
    relation_ddl_effect(statement)
}
