//! A SQL macro file parsed into its definition, then checked on its own.

use crate::sql_macros::_helpers::parsing::header::{Header, HeaderParser};
use crate::sql_macros::_helpers::parsing::lexing::{AtConstruct, at_constructs};
use crate::sql_macros::_helpers::parsing::lexing::{Token, TokenKind, line_column, tokens};
use crate::sql_macros::_helpers::parsing::literal_positions::{
    LiteralRules, literal_error, non_literal_use,
};
use crate::sql_macros::_helpers::parsing::parameters::{ParameterReference, parameter_references};
use crate::sql_macros::_helpers::parsing::slots::{SlotRead, check_group_positions, slot_reads};
use crate::sql_macros::_helpers::rendering::arguments::{
    ShapeContext, argument_shape, check_shape,
};
use crate::sql_macros::constants::{
    BARE_MACRO_CALL_CODE, BODY_REFERENCE_CODE, INVALID_DEFINITION_CODE, MIXED_KIND_CODE,
    PARAMETER_REFERENCE_CODE, REFERENCE_FUNCTIONS, RESERVED_PARAMETER_NAMES, SELECT_KEYWORD,
    WITH_KEYWORD,
};
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::{
    ParameterKind, SqlMacroDefinition, SqlMacroForm, SqlMacroLocation, SqlMacroParameter,
};

/// Locates a byte offset of the macro file.
type Locate<'a> = &'a dyn Fn(usize) -> SqlMacroLocation;

/// The SQL macro `name` defined by the file at `path`, or why it is not a valid SQL macro.
pub(crate) fn parsed_definition(
    name: &str,
    path: &str,
    source: &str,
) -> Result<SqlMacroDefinition, SqlMacroError> {
    let header: Header = HeaderParser { name, path, source }.parse()?;
    let at = |offset: usize| {
        let (line, column) = line_column(source, offset);
        SqlMacroLocation {
            path: path.to_owned(),
            line,
            column,
        }
    };
    let (body_start, body) = body_text(name, (source, header.body_start), &at)?;
    let in_body = |offset: usize| at(body_start + offset);
    let scanned: Vec<Token> = tokens(body).map_err(|_| unclosed_body(name, at(body_start)))?;
    let form: SqlMacroForm = body_form(body, &scanned);
    check_references(name, body, &scanned, &in_body)?;
    let calls: Vec<String> = body_calls(name, body, &in_body)?;
    check_returns(
        name,
        (
            form,
            header.returns.is_some() || header.returns_like.is_some(),
        ),
        at(0),
    )?;
    if let Some((like, offset)) = &header.returns_like {
        check_returns_like(name, (like, &header.parameters), at(*offset))?;
    }
    let (body_line, body_column) = line_column(source, body_start);
    let definition: SqlMacroDefinition = SqlMacroDefinition {
        name: name.to_owned(),
        path: path.to_owned(),
        description: header.description,
        parameters: header.parameters,
        returns: header.returns,
        returns_like: header.returns_like.map(|(like, _)| like),
        form,
        body: body.to_owned(),
        body_line,
        body_column,
        calls,
    };
    checked_definition(definition, &scanned, &in_body)
}

/// The definition once its parameters and their uses in the body pass every check.
fn checked_definition(
    definition: SqlMacroDefinition,
    scanned: &[Token],
    in_body: Locate<'_>,
) -> Result<SqlMacroDefinition, SqlMacroError> {
    check_parameters(&definition)?;
    check_parameter_references(&definition, scanned, in_body)?;
    let references: Vec<ParameterReference> = parameter_references(&definition.body, scanned);
    let reads: Vec<SlotRead> = slot_reads(&definition, (&definition.body, scanned, &references))
        .map_err(|(offset, error)| error.at(in_body(offset)))?;
    check_group_positions(&definition, (&definition.body, scanned), &reads)
        .map_err(|(offset, error)| error.at(in_body(offset)))?;
    if let Some(found) = non_literal_use(&definition, scanned, &references, LiteralRules::Universal)
    {
        return Err(literal_error(&definition, &found).at(in_body(found.reference.start)));
    }
    Ok(definition)
}

/// The body's byte offset in the file and its text, without a trailing `;`.
fn body_text<'s>(
    name: &str,
    (source, header_end): (&'s str, usize),
    at: Locate<'_>,
) -> Result<(usize, &'s str), SqlMacroError> {
    let rest: &str = &source[header_end..];
    let body_start: usize = header_end + rest.len() - rest.trim_start().len();
    let mut body: &str = source[body_start..].trim_end();
    if let Some(stripped) = body.strip_suffix(';') {
        body = stripped.trim_end();
    }
    if body.is_empty() {
        return Err(SqlMacroError::new(
            INVALID_DEFINITION_CODE,
            format!("macro {name}() has no body after its MACRO header"),
        )
        .at(at(header_end)));
    }
    Ok((body_start, body))
}

fn unclosed_body(name: &str, location: SqlMacroLocation) -> SqlMacroError {
    SqlMacroError::new(
        INVALID_DEFINITION_CODE,
        format!("body of macro {name}() has an unclosed quote or comment"),
    )
    .at(location)
}

/// The form a body's first token decides.
pub(crate) fn body_form(body: &str, scanned: &[Token]) -> SqlMacroForm {
    match scanned.first() {
        Some(first) if first.is_word(body, WITH_KEYWORD) => SqlMacroForm::CteList,
        Some(first) if first.is_word(body, SELECT_KEYWORD) => SqlMacroForm::ColumnList,
        _ => SqlMacroForm::Expression,
    }
}

/// The SQL macros the body calls, rejecting Python and bare macro calls.
fn body_calls(name: &str, body: &str, in_body: Locate<'_>) -> Result<Vec<String>, SqlMacroError> {
    let constructs: Vec<AtConstruct> =
        at_constructs(body, None).map_err(|_| unclosed_body(name, in_body(0)))?;
    let mut calls: Vec<String> = Vec::new();
    for construct in constructs {
        match construct {
            AtConstruct::Sql { name: called, .. } => {
                if !calls.contains(&called) {
                    calls.push(called);
                }
            }
            AtConstruct::Python {
                name: called,
                start,
                ..
            } => {
                return Err(SqlMacroError::new(
                    MIXED_KIND_CODE,
                    format!("SQL macro {name}() calls Python macro {called}(); SQL and Python macros never call each other"),
                )
                .at(in_body(start))
                .helped(format!("compose them in the model instead, such as @py.{called}(@sql.{name}(...))")));
            }
            AtConstruct::Bare {
                name: called,
                start,
            } => {
                return Err(SqlMacroError::new(
                    BARE_MACRO_CALL_CODE,
                    format!("macro call @{called}() in SQL macro {name}() does not say which kind of macro it calls"),
                )
                .at(in_body(start))
                .helped(format!("write @sql.{called}(...) to call a SQL macro")));
            }
        }
    }
    Ok(calls)
}

/// Rejects `returns` on a CTE-list or column-list macro.
fn check_returns(
    name: &str,
    (form, returns): (SqlMacroForm, bool),
    location: SqlMacroLocation,
) -> Result<(), SqlMacroError> {
    match (form, returns) {
        (SqlMacroForm::CteList | SqlMacroForm::ColumnList, true) => Err(SqlMacroError::new(
            INVALID_DEFINITION_CODE,
            format!(
                "{} macro {name}() cannot declare returns; only expression macros return a value",
                form.label()
            ),
        )
        .at(location)),
        _ => Ok(()),
    }
}

/// Rejects `returns TYPE OF` naming no argument, or an argument with no scalar type.
fn check_returns_like(
    name: &str,
    (like, parameters): (&str, &[SqlMacroParameter]),
    location: SqlMacroLocation,
) -> Result<(), SqlMacroError> {
    let Some(parameter) = parameters
        .iter()
        .find(|parameter| parameter.name.eq_ignore_ascii_case(like))
    else {
        let names: Vec<&str> = parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        return Err(SqlMacroError::new(
            INVALID_DEFINITION_CODE,
            format!("returns TYPE OF {like} names no argument of macro {name}()"),
        )
        .at(location)
        .helped(if names.is_empty() {
            format!("macro {name}() has no arguments; declare returns <type> instead")
        } else {
            format!("name one of its arguments: {}", names.join(", "))
        }));
    };
    match parameter.kind {
        ParameterKind::Expression { .. }
        | ParameterKind::Any { .. }
        | ParameterKind::Constant { .. }
        | ParameterKind::Enum { .. } => Ok(()),
        ParameterKind::Collection { .. }
        | ParameterKind::Relation { .. } => Err(SqlMacroError::new(
            INVALID_DEFINITION_CODE,
            format!(
                "returns TYPE OF {} names a {} argument of macro {name}(); only a scalar argument has a type to return",
                parameter.name,
                parameter.kind.label()
            ),
        )
        .at(location)),
    }
}

/// Rejects an `@name` naming no parameter and a RELATION parameter written bare.
fn check_parameter_references(
    definition: &SqlMacroDefinition,
    scanned: &[Token],
    at: Locate<'_>,
) -> Result<(), SqlMacroError> {
    let body: &str = &definition.body;
    let name: &str = &definition.name;
    let listed = || -> String {
        if definition.parameters.is_empty() {
            "it has no parameters".to_owned()
        } else {
            format!(
                "its parameters are {}",
                definition
                    .parameters
                    .iter()
                    .map(|parameter| format!("@{}", parameter.name))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        }
    };
    let references: Vec<ParameterReference> = parameter_references(body, scanned);
    for reference in &references {
        if definition.parameter(&reference.name).is_none() {
            return Err(SqlMacroError::new(
                PARAMETER_REFERENCE_CODE,
                format!(
                    "@{} in the body of macro {name}() names no parameter of the macro",
                    reference.name
                ),
            )
            .at(at(reference.start))
            .helped(format!(
                "{}; declare it in arguments (...) or fix the name",
                listed()
            )));
        }
    }
    let referenced: Vec<usize> = references.iter().map(|reference| reference.token).collect();
    let ctes: Vec<String> = body_cte_names(body, scanned);
    for (index, token) in scanned.iter().enumerate() {
        if token.kind != TokenKind::Word || referenced.contains(&index) {
            continue;
        }
        let word: &str = token.text(body);
        let Some(parameter) = definition.parameter(word) else {
            continue;
        };
        if !matches!(parameter.kind, ParameterKind::Relation { .. })
            || ctes.iter().any(|cte| cte.eq_ignore_ascii_case(word))
            || aliased(body, scanned, word)
        {
            continue;
        }
        let slot_mapping: bool = scanned
            .get(index + 3)
            .is_some_and(|assign| assign.is(body, ":="));
        let qualifier: bool = scanned
            .get(index + 1)
            .is_some_and(|dot| dot.is(body, ".") && dot.start == token.end)
            && !(index > 0 && scanned[index - 1].is(body, "."))
            && !slot_mapping;
        let table: bool = index > 0
            && (scanned[index - 1].is_word(body, "FROM")
                || scanned[index - 1].is_word(body, "JOIN"));
        if qualifier || table {
            return Err(SqlMacroError::new(
                PARAMETER_REFERENCE_CODE,
                format!("{word} is a parameter of macro {name}(); write @{word}"),
            )
            .at(at(token.start))
            .helped("a bare name in a SQL macro body is plain SQL; write parameters as @name, as generic audits do"));
        }
    }
    Ok(())
}

/// Whether the body names a table or column alias `word`, as in `FROM @src AS src`.
fn aliased(body: &str, scanned: &[Token], word: &str) -> bool {
    scanned.windows(2).any(|pair| {
        pair[0].is_word(body, "AS")
            && pair[1].kind == TokenKind::Word
            && pair[1].text(body).eq_ignore_ascii_case(word)
    })
}

/// The names of the CTEs a body's `WITH` lists define, at any depth.
fn body_cte_names(body: &str, scanned: &[Token]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (index, token) in scanned.iter().enumerate() {
        let defines: bool = token.kind == TokenKind::Word
            && scanned
                .get(index + 1)
                .is_some_and(|keyword| keyword.is_word(body, "AS"))
            && scanned
                .get(index + 2)
                .is_some_and(|open| open.is(body, "("))
            && index > 0
            && (scanned[index - 1].is_word(body, "WITH")
                || scanned[index - 1].is(body, ",")
                || scanned[index - 1].is_word(body, "RECURSIVE"));
        if defines {
            names.push(token.text(body).to_owned());
        }
    }
    names
}

/// Rejects `__ref`, `__source` and `__seed` in the body: relations are passed in.
fn check_references(
    name: &str,
    body: &str,
    scanned: &[Token],
    at: Locate<'_>,
) -> Result<(), SqlMacroError> {
    for (index, token) in scanned.iter().enumerate() {
        if token.kind != TokenKind::Word || !REFERENCE_FUNCTIONS.contains(&token.text(body)) {
            continue;
        }
        if !scanned
            .get(index + 1)
            .is_some_and(|open| open.is(body, "("))
        {
            continue;
        }
        let function: &str = token.text(body);
        let referenced: String = scanned
            .get(index + 2)
            .filter(|argument| matches!(argument.kind, TokenKind::Quoted | TokenKind::Text))
            .map(|argument| {
                crate::sql_macros::_helpers::parsing::header::unquoted(argument.text(body))
            })
            .unwrap_or_default();
        let kind: &str = match function {
            "__source" => "source",
            "__seed" => "seed",
            _ => "model",
        };
        return Err(SqlMacroError::new(
            BODY_REFERENCE_CODE,
            format!("macro {name}() reads {kind}:{referenced} through {function}() in its body"),
        )
        .at(at(token.start))
        .helped(format!(
            "pass the relation in as a RELATION argument and write {function}(\"{referenced}\") in the model"
        )));
    }
    Ok(())
}

/// Rejects reserved names and defaults the parameter does not accept.
fn check_parameters(definition: &SqlMacroDefinition) -> Result<(), SqlMacroError> {
    for parameter in &definition.parameters {
        let location = SqlMacroLocation {
            path: definition.path.clone(),
            line: parameter.line,
            column: parameter.column,
        };
        if RESERVED_PARAMETER_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(&parameter.name))
        {
            return Err(SqlMacroError::new(
                INVALID_DEFINITION_CODE,
                format!(
                    "argument {} of macro {}() uses a built-in name; @{} in a body would not read it",
                    parameter.name, definition.name, parameter.name
                ),
            )
            .at(location)
            .helped(format!("rename it; {} are built-in names", RESERVED_PARAMETER_NAMES.join(", "))));
        }
        if let Some(default) = &parameter.default {
            check_shape(
                definition,
                parameter,
                (default, &argument_shape(default)),
                ShapeContext {
                    adapter: "",
                    caller: None,
                },
            )
            .map_err(|error| {
                SqlMacroError {
                    code: INVALID_DEFINITION_CODE,
                    message: format!("DEFAULT of {}", error.message),
                    ..error
                }
                .at(location.clone())
            })?;
        }
    }
    Ok(())
}
