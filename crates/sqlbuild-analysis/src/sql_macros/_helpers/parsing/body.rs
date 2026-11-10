//! A SQL macro body parsed as SQL, its parameter references located and replaced in place.

use sqlquack_syntax::expressions::names::models::{Column, Identifier};
use sqlquack_syntax::expressions::operators::models::DotAccess;
use sqlquack_syntax::expressions::queries::models::Select;
use sqlquack_syntax::{Expression, ExpressionWalk, Parsed};

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::analysis_route::types::NativeDialect;

use crate::sql_macros::_helpers::parsing::captures::{output_selects, slot_captures};
use crate::sql_macros::_helpers::parsing::header::unquoted;
use crate::sql_macros::_helpers::parsing::lexing::{Token, tokens};
use crate::sql_macros::_helpers::parsing::parameters::{
    ParameterReference, alias_follows, declarations_masked, in_lists_parenthesised,
    parameter_references, placeholder, placeholder_parameter, using_reads_joined,
    with_placeholders,
};
use crate::sql_macros::_helpers::parsing::slots::{SlotRead, check_group_positions, slot_reads};
use crate::sql_macros::_helpers::rendering::arguments::is_atomic;
use crate::sql_macros::_helpers::typing::typed_calls::{shifted, spliced};
use crate::sql_macros::constants::{
    ARGUMENT_CODE, INVALID_DEFINITION_CODE, NAME_CLASH_CODE, SLOT_PLACEHOLDER_PREFIX,
};
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::{
    ArgumentShape, BodyExpansion, ExpandedText, ParameterKind, RenderedArgument, SlotMapping,
    SqlMacroDefinition, SqlMacroForm, SqlMacroParameter, TextSplice, TypedCall,
};

const EXPRESSION_PREFIX: &str = "SELECT ";
const TYPE_PREFIX: &str = "SELECT CAST(NULL AS ";
const CTE_SUFFIX: &str = "\nSELECT 1";

/// The body parsed as SQL.
pub(crate) struct ParsedBody {
    pub(crate) select: Box<Select>,
}

/// The native parser of the project's SQL analysis `dialect`, or why SQL macros are unavailable.
pub(crate) fn macro_parser(dialect: Option<&str>) -> Result<&'static dyn NativeDialect, String> {
    match analysis_route(dialect) {
        AnalysisRoute::Native(parser) => Ok(parser),
        _ => Err(match dialect {
            Some(dialect) => format!(
                "SQL macros are unavailable for SQL analysis dialect '{dialect}', which SQLBuild has no native parser for"
            ),
            None => "SQL macros are unavailable without a SQL analysis dialect".to_owned(),
        }),
    }
}

/// The body parsed by the project's parser, an expression body reading as a type taken as a cast's.
pub(crate) fn parsed(
    parser: &dyn NativeDialect,
    form: SqlMacroForm,
    body: &str,
) -> Result<ParsedBody, String> {
    match form {
        SqlMacroForm::Expression => parsed_text(parser, &format!("{EXPRESSION_PREFIX}{body}"))
            .or_else(|reason| {
                parsed_text(parser, &format!("{TYPE_PREFIX}{body})")).map_err(|_| reason)
            }),
        SqlMacroForm::CteList => parsed_text(parser, &format!("{body}{CTE_SUFFIX}")),
        SqlMacroForm::ColumnList => parsed_text(parser, body),
    }
}

/// The single SELECT `text` holds, parsed by the project's parser, or why that parser cannot read it.
fn parsed_text(parser: &dyn NativeDialect, text: &str) -> Result<ParsedBody, String> {
    match parsed_statement(parser, text)? {
        Expression::Select(select) => Ok(ParsedBody { select }),
        _ => Err("it is not a SELECT fragment".to_owned()),
    }
}

/// The one statement `text` parses to in the project's parser, or why it cannot read it.
pub(crate) fn parsed_statement(
    parser: &dyn NativeDialect,
    text: &str,
) -> Result<Expression, String> {
    let Parsed {
        mut statements,
        gaps,
    } = parser
        .parse(&in_lists_parenthesised(&using_reads_joined(
            &declarations_masked(text),
        )))
        .map_err(|gap| gap.message)?;
    if let Some(gap) = gaps.first() {
        return Err(format!(
            "the {} parser cannot read {}",
            parser.name(),
            gap.construct
        ));
    }
    match (statements.pop(), statements.is_empty()) {
        (Some(statement), true) => Ok(statement),
        _ => Err("it holds more than one statement".to_owned()),
    }
}

/// The reason the parsed select is not exactly the form's fragment, if it is not.
fn shape_problem(form: SqlMacroForm, select: &Select) -> Option<&'static str> {
    let clauses: bool = select.from.is_some()
        || !select.joins.is_empty()
        || select.where_clause.is_some()
        || select.group_by.is_some()
        || select.having.is_some()
        || select.qualify.is_some()
        || select.order_by.is_some()
        || select.limit.is_some()
        || select.offset.is_some()
        || select.distinct;
    match form {
        SqlMacroForm::Expression => {
            if clauses || select.with.is_some() || select.expressions.len() != 1 {
                return Some("an expression macro body must be exactly one SQL expression");
            }
            if matches!(select.expressions[0], Expression::Alias(_)) {
                return Some(
                    "an expression macro body is an expression, not a named column; drop the alias",
                );
            }
            None
        }
        SqlMacroForm::CteList => {
            let with_list: bool = select.with.as_ref().is_some_and(|with| !with.recursive);
            if !with_list || clauses || select.expressions.len() != 1 {
                return Some(
                    "a CTE-list macro body must be only a WITH list of CTEs, with no final query",
                );
            }
            None
        }
        SqlMacroForm::ColumnList => {
            if clauses || select.with.is_some() || select.expressions.is_empty() {
                return Some(
                    "a column-list macro body must be only SELECT and its columns, with no FROM or other clauses",
                );
            }
            None
        }
    }
}

/// The parameter a parsed identifier stands for, when it is a placeholder of `@name`.
fn parameter_of<'a>(
    definition: &'a SqlMacroDefinition,
    identifier: &Identifier,
) -> Option<&'a SqlMacroParameter> {
    placeholder_parameter(&identifier.name).and_then(|name| definition.parameter(name))
}

/// The names a body defines: CTE names, or the output column names of a column list.
fn defined_names(
    definition: &SqlMacroDefinition,
    (text, select): (&str, &Select),
    arguments: &[RenderedArgument],
) -> Result<Vec<String>, String> {
    match definition.form {
        SqlMacroForm::Expression => Ok(Vec::new()),
        SqlMacroForm::CteList => Ok(select
            .with
            .iter()
            .flat_map(|with| with.ctes.iter())
            .map(|cte| cte.alias.name.clone())
            .collect()),
        SqlMacroForm::ColumnList => {
            let mut names: Vec<String> = Vec::new();
            for (index, projection) in select.expressions.iter().enumerate() {
                match projection {
                    Expression::Alias(alias) => names.push(alias.alias.name.clone()),
                    Expression::Column(column) => {
                        names.extend(column_output_names(definition, column, arguments)?);
                    }
                    Expression::Dot(dot) if aliased_slot(definition, dot).is_some() => {
                        let (relation, slot) = aliased_slot(definition, dot).unwrap_or_default();
                        names.extend(slot_output_names(definition, (relation, slot), arguments));
                    }
                    _ => {
                        return Err(format!(
                            "every column of a column-list macro needs a name; add AS <name> to {}",
                            shown_projection(text, index)
                        ));
                    }
                }
            }
            Ok(names)
        }
    }
}

/// The output names of a bare column projection: a `COLUMNS` slot's columns, or the name.
fn column_output_names(
    definition: &SqlMacroDefinition,
    column: &Column,
    arguments: &[RenderedArgument],
) -> Result<Vec<String>, String> {
    if let Some(parameter) = parameter_of(definition, &column.name) {
        return Err(format!(
            "every column of a column-list macro needs a name; add AS <name> to @{}",
            parameter.name
        ));
    }
    let slot: &str = &column.name.name;
    let relation: Option<&str> = column
        .table
        .as_ref()
        .and_then(|table| parameter_of(definition, table))
        .map(|parameter| parameter.name.as_str());
    Ok(match relation {
        Some(relation) => slot_output_names(definition, (relation, slot), arguments),
        None => vec![slot.to_owned()],
    })
}

/// The relation and slot an aliased slot read `alias.@relation.slot` names, if `dot` is one.
fn aliased_slot<'d>(
    definition: &'d SqlMacroDefinition,
    dot: &'d DotAccess,
) -> Option<(&'d str, &'d str)> {
    let Expression::Column(column) = &dot.this else {
        return None;
    };
    column.table.as_ref()?;
    let parameter: &SqlMacroParameter = parameter_of(definition, &column.name)?;
    parameter
        .kind
        .declares_slot(&dot.field.name)
        .then_some((parameter.name.as_str(), dot.field.name.as_str()))
}

/// The output names of a read of `relation.slot`: a group's mapped columns, or the slot's name.
fn slot_output_names(
    definition: &SqlMacroDefinition,
    (relation, slot): (&str, &str),
    arguments: &[RenderedArgument],
) -> Vec<String> {
    let mapped: Option<&SlotMapping> = arguments
        .iter()
        .find(|argument| definition.parameters[argument.parameter].name == relation)
        .and_then(|argument| {
            argument
                .slots
                .iter()
                .find(|mapping| mapping.group && mapping.slot.eq_ignore_ascii_case(slot))
        });
    match mapped {
        Some(mapping) => mapping
            .columns
            .iter()
            .map(|column| unquoted(column))
            .collect(),
        None => vec![slot.to_owned()],
    }
}

/// The `index`th item of the select list `text` as written, its placeholders shown as `@name`.
fn shown_projection(text: &str, index: usize) -> String {
    let scanned: Vec<Token> = tokens(text).unwrap_or_default();
    let mut items: Vec<Option<(usize, usize)>> = vec![None];
    let mut depth: usize = 0;
    for token in scanned.iter().skip(1) {
        if depth == 0 && token.is(text, ",") {
            items.push(None);
            continue;
        }
        if token.is(text, "(") {
            depth += 1;
        } else if token.is(text, ")") {
            depth = depth.saturating_sub(1);
        }
        if let Some(item) = items.last_mut() {
            let start: usize = item.map_or(token.start, |(start, _)| start);
            *item = Some((start, token.end));
        }
    }
    items
        .get(index)
        .copied()
        .flatten()
        .map_or("", |(start, end)| &text[start..end])
        .replace(&placeholder(""), "@")
}

/// Rejects a RELATION parameter read as a plain value, as in `SELECT @src`.
fn check_relation_values(definition: &SqlMacroDefinition, select: &Select) -> Result<(), String> {
    let root: Expression = Expression::Select(Box::new(select.clone()));
    for node in root.dfs() {
        if let Expression::Column(column) = node
            && column.table.is_none()
            && let Some(parameter) = parameter_of(definition, &column.name)
            && matches!(parameter.kind, ParameterKind::Relation { .. })
        {
            let name: &str = &parameter.name;
            return Err(format!(
                "RELATION argument {name} is used as a value; read its columns instead, such as @{name}.column"
            ));
        }
    }
    Ok(())
}

/// The text replacing one `@name` reference, from its argument and where it stands.
fn substituted(
    kind: &ParameterKind,
    parameter: &str,
    argument: &RenderedArgument,
    (sql, scanned, token): (&str, &[Token], usize),
) -> String {
    match kind {
        ParameterKind::Relation { .. } => {
            let qualifier: bool = scanned
                .get(token + 1)
                .is_some_and(|dot| dot.is(sql, ".") && dot.start == scanned[token].end);
            match &argument.shape {
                ArgumentShape::Reference { .. } if qualifier => parameter.to_owned(),
                ArgumentShape::Reference { call } if alias_follows(sql, scanned, token) => {
                    call.clone()
                }
                ArgumentShape::Reference { call } => format!("{call} AS {parameter}"),
                _ => argument.sql.clone(),
            }
        }
        ParameterKind::Expression { .. }
        | ParameterKind::Any { .. }
        | ParameterKind::Constant { .. }
            if !is_atomic(&argument.sql) =>
        {
            format!("({})", argument.sql)
        }
        _ => argument.sql.clone(),
    }
}

/// The mapping a call made for the slot a read names, if it made one.
fn read_mapping<'a>(
    definition: &SqlMacroDefinition,
    read: &SlotRead,
    arguments: &'a [RenderedArgument],
) -> Option<&'a SlotMapping> {
    arguments
        .iter()
        .find(|argument| {
            definition.parameters[argument.parameter]
                .name
                .eq_ignore_ascii_case(&read.relation)
        })?
        .slots
        .iter()
        .find(|mapping| mapping.slot.eq_ignore_ascii_case(&read.slot))
}

/// A column `select` outputs twice: bare or read through a relation, and in a group beside it.
fn select_clash(
    definition: &SqlMacroDefinition,
    select: &Select,
    groups: &[(&str, &SlotMapping)],
) -> Option<String> {
    let mut bare: Vec<&str> = Vec::new();
    let mut read: Vec<(&str, &str)> = Vec::new();
    for projection in &select.expressions {
        match projection {
            Expression::Column(column) => {
                match column
                    .table
                    .as_ref()
                    .and_then(|table| parameter_of(definition, table))
                {
                    Some(parameter) => {
                        read.push((parameter.name.as_str(), &column.name.name));
                        if !parameter.kind.declares_slot(&column.name.name) {
                            bare.push(&column.name.name);
                        }
                    }
                    None => bare.push(&column.name.name),
                }
            }
            Expression::Dot(dot) => read.extend(aliased_slot(definition, dot)),
            _ => {}
        }
    }
    for (relation, mapping) in groups {
        let in_select: bool = read.iter().any(|(read_relation, slot)| {
            read_relation.eq_ignore_ascii_case(relation) && slot.eq_ignore_ascii_case(&mapping.slot)
        });
        if !in_select {
            continue;
        }
        for column in &mapping.columns {
            let column: String = unquoted(column);
            if let Some(name) = bare.iter().find(|name| name.eq_ignore_ascii_case(&column)) {
                return Some((*name).to_owned());
            }
        }
    }
    None
}

/// Rejects a grouped column named like a column the macro's body defines.
fn check_group_clashes(
    definition: &SqlMacroDefinition,
    select: &Select,
    arguments: &[RenderedArgument],
) -> Result<(), SqlMacroError> {
    let mut groups: Vec<(&str, &SlotMapping)> = Vec::new();
    for argument in arguments {
        let relation: &str = &definition.parameters[argument.parameter].name;
        for mapping in &argument.slots {
            if mapping.group {
                groups.push((relation, mapping));
            }
        }
    }
    if groups.is_empty() {
        return Ok(());
    }
    let root: Expression = Expression::Select(Box::new(select.clone()));
    let mut defined: Vec<String> = Vec::new();
    for node in root.dfs() {
        if let Expression::Select(inner) = node {
            for projection in &inner.expressions {
                if let Expression::Alias(alias) = projection {
                    defined.push(alias.alias.name.clone());
                }
            }
        }
    }
    for select in output_selects(&root) {
        if let Some(clash) = select_clash(definition, select, &groups) {
            defined.push(clash);
        }
    }
    for (relation, mapping) in groups {
        for column in &mapping.columns {
            let bare: String = unquoted(column);
            if let Some(clash) = defined
                .iter()
                .find(|defined| defined.eq_ignore_ascii_case(&bare))
            {
                return Err(SqlMacroError::new(
                    NAME_CLASH_CODE,
                    format!(
                        "column {column} of slot {relation}.{} clashes with column {clash} that macro {}() defines",
                        mapping.slot, definition.name
                    ),
                )
                .helped(format!(
                    "map {relation}.{} to columns the macro does not define, or rename {clash} in {}",
                    mapping.slot, definition.path
                )));
            }
        }
    }
    Ok(())
}

/// `body` with each of `marked` as its slot placeholder, by index, and other references as theirs.
pub(crate) fn marked_text(
    (body, references): (&str, &[ParameterReference]),
    marked: &[&SlotRead],
) -> String {
    let mut text: String = String::with_capacity(body.len());
    let mut cursor: usize = 0;
    for (position, reference) in references.iter().enumerate() {
        if let Some(index) = marked.iter().position(|read| read.reference == position) {
            let read: &SlotRead = marked[index];
            text.push_str(&body[cursor..read.start]);
            text.push_str(&slot_placeholder(index));
            cursor = read.end;
            continue;
        }
        if reference.start < cursor {
            continue;
        }
        text.push_str(&body[cursor..reference.start]);
        text.push_str(&placeholder(&reference.name));
        cursor = reference.end;
    }
    text.push_str(&body[cursor..]);
    text
}

/// Rejects a mapped single-column slot read in a select-list item without `AS`.
fn check_mapped_reads(
    (parser, definition): (&dyn NativeDialect, &SqlMacroDefinition),
    (body, references, reads): (&str, &[ParameterReference], &[SlotRead]),
    arguments: &[RenderedArgument],
) -> Result<(), SqlMacroError> {
    let mut mapped: Vec<(&SlotRead, &SlotMapping)> = Vec::new();
    for read in reads.iter().filter(|read| !read.group) {
        if let Some(mapping) = read_mapping(definition, read, arguments) {
            mapped.push((read, mapping));
        }
    }
    if mapped.is_empty() {
        return Ok(());
    }
    let marked: Vec<&SlotRead> = mapped.iter().map(|(read, _)| *read).collect();
    let text: String = marked_text((body, references), &marked);
    let Ok(parsed_text) = parsed(parser, definition.form, &text) else {
        return Ok(());
    };
    let root: Expression = Expression::Select(parsed_text.select);
    let wrapped: bool = definition.form == SqlMacroForm::Expression;
    for (position, select) in output_selects(&root).into_iter().enumerate() {
        if wrapped && position == 0 {
            continue;
        }
        for projection in &select.expressions {
            if matches!(projection, Expression::Alias(_)) {
                continue;
            }
            let found: Option<usize> = projection
                .dfs()
                .find_map(slot_placeholder_name)
                .and_then(|name| (0..mapped.len()).find(|index| name == slot_placeholder(*index)));
            if let Some(index) = found {
                let (read, mapping) = mapped[index];
                return Err(SqlMacroError::new(
                    ARGUMENT_CODE,
                    format!(
                        "slot {}.{} of macro {}() is mapped to {}, but the macro reads it in a select list without AS, so the output column's name would depend on the mapping",
                        read.relation, read.slot, definition.name, mapping.columns[0]
                    ),
                )
                .helped(format!(
                    "add AS <name> to {} in {}; a mapped single-column slot needs an explicit output name",
                    &body[read.start..read.end],
                    definition.path
                )));
            }
        }
    }
    Ok(())
}

/// The word the `index`th mapped single-column slot read stands as while the body is parsed.
pub(crate) fn slot_placeholder(index: usize) -> String {
    format!("{SLOT_PLACEHOLDER_PREFIX}{index}")
}

/// The placeholder column name a single-column slot read stands as, if `node` is one.
pub(crate) fn slot_placeholder_name(node: &Expression) -> Option<&str> {
    let Expression::Column(column) = node else {
        return None;
    };
    let name: &str = &column.name.name;
    (column.table.is_none() && name.starts_with(SLOT_PLACEHOLDER_PREFIX)).then_some(name)
}

/// A slot read's columns: aliased, bare for a group or in `USING`, else through the relation.
fn substituted_read(
    read: &SlotRead,
    argument: &RenderedArgument,
    (body, qualifier): (&str, &str),
    outer: Option<&ParameterKind>,
) -> String {
    let mapped: Option<&SlotMapping> = argument
        .slots
        .iter()
        .find(|mapping| mapping.slot.eq_ignore_ascii_case(&read.slot));
    let columns: Vec<&str> = match mapped {
        Some(mapping) => mapping.columns.iter().map(String::as_str).collect(),
        None => vec![read.written.as_str()],
    };
    let prefix: Option<&str> = read.prefix.map(|(start, end)| &body[start..end]);
    let forwarded: bool = matches!(argument.shape, ArgumentShape::ParameterReference { .. });
    let bare: bool = read.group || read.in_using;
    let mut texts: Vec<String> = Vec::with_capacity(columns.len());
    for column in columns {
        let outer_slot: bool =
            forwarded && outer.is_some_and(|kind| kind.declares_slot(&unquoted(column)));
        texts.push(match prefix {
            Some(prefix) if outer_slot => format!("{prefix}{}.{column}", argument.sql),
            Some(prefix) => format!("{prefix}{column}"),
            None if outer_slot => format!("{}.{column}", argument.sql),
            None if bare => column.to_owned(),
            None => format!("{qualifier}.{column}"),
        });
    }
    texts.join(", ")
}

/// The outer relation's kind for an argument that forwards an outer macro's relation.
fn forwarded_kind<'o>(
    argument: &RenderedArgument,
    outer: Option<&'o SqlMacroDefinition>,
) -> Option<&'o ParameterKind> {
    let ArgumentShape::ParameterReference { name } = &argument.shape else {
        return None;
    };
    outer?.parameter(name).map(|parameter| &parameter.kind)
}

/// Rejects a forwarded slot mapped to a slot of the outer relation of the other kind.
fn check_forwarded_kinds(
    definition: &SqlMacroDefinition,
    arguments: &[RenderedArgument],
    outer: Option<&SqlMacroDefinition>,
) -> Result<(), SqlMacroError> {
    for argument in arguments {
        let ArgumentShape::ParameterReference { name } = &argument.shape else {
            continue;
        };
        let Some(kind) = outer
            .and_then(|outer| outer.parameter(name))
            .map(|outer| &outer.kind)
        else {
            continue;
        };
        let relation: &str = &definition.parameters[argument.parameter].name;
        for mapping in &argument.slots {
            for column in &mapping.columns {
                let bare: String = unquoted(column);
                if !kind.declares_slot(&bare) || kind.declares_group(&bare) == mapping.group {
                    continue;
                }
                let (inner, outer_kind) = if mapping.group {
                    ("a COLUMNS slot", "a single-column slot")
                } else {
                    ("a single-column slot", "a COLUMNS slot")
                };
                return Err(SqlMacroError::new(
                    ARGUMENT_CODE,
                    format!(
                        "slot {relation}.{} of macro {}() is {inner}, but the call maps it to {name}.{bare}, {outer_kind} of the forwarded relation {name}",
                        mapping.slot, definition.name
                    ),
                )
                .helped(format!(
                    "map {relation}.{} to slots of {name} of the same kind, or to columns of {name}",
                    mapping.slot
                )));
            }
        }
    }
    Ok(())
}

/// `body` with each `@name` replaced by its argument, token by token, and the calls it holds.
fn substituted_body(
    (definition, outer): (&SqlMacroDefinition, Option<&SqlMacroDefinition>),
    (body, scanned): (&str, &[Token]),
    (references, reads): (&[ParameterReference], &[SlotRead]),
    (arguments, body_calls): (&[RenderedArgument], &[TypedCall]),
) -> (String, Vec<TypedCall>) {
    let mut expanded: String = String::with_capacity(body.len());
    let mut splices: Vec<TextSplice> = Vec::new();
    let mut argument_calls: Vec<TypedCall> = Vec::new();
    let mut cursor: usize = 0;
    for (position, reference) in references.iter().enumerate() {
        let Some(argument) = arguments.iter().find(|argument| {
            definition.parameters[argument.parameter]
                .name
                .eq_ignore_ascii_case(&reference.name)
        }) else {
            continue;
        };
        let parameter = &definition.parameters[argument.parameter];
        let read: Option<&SlotRead> = reads.iter().find(|read| read.reference == position);
        let source: (usize, usize) = read.map_or((reference.start, reference.end), |read| {
            (read.start, read.end)
        });
        expanded.push_str(&body[cursor..source.0]);
        let text: String = substituted(
            &parameter.kind,
            &parameter.name,
            argument,
            (body, scanned, reference.token),
        );
        let text: String = match read {
            Some(read) => substituted_read(
                read,
                argument,
                (body, &text),
                forwarded_kind(argument, outer),
            ),
            None => text,
        };
        let start: usize = expanded.len();
        if !argument.calls.is_empty() {
            let inner: Option<usize> = if text == argument.sql {
                Some(0)
            } else if text.len() == argument.sql.len() + 2 && text[1..].starts_with(&argument.sql) {
                Some(1)
            } else {
                None
            };
            if let Some(inner) = inner {
                argument_calls.extend(shifted(argument.calls.clone(), start + inner));
            }
        }
        expanded.push_str(&text);
        splices.push(TextSplice {
            source,
            output: (start, expanded.len()),
        });
        cursor = source.1;
    }
    expanded.push_str(&body[cursor..]);
    let mut calls: Vec<TypedCall> = argument_calls;
    calls.extend(spliced(body_calls.to_vec(), &splices));
    (expanded, calls)
}

/// `calls` in `text` kept within its `range`, relative to the range's start.
fn within(calls: Vec<TypedCall>, (start, end): (usize, usize)) -> Vec<TypedCall> {
    calls
        .into_iter()
        .filter(|call| start <= call.start && call.start < end)
        .map(|call| TypedCall {
            start: call.start - start,
            end: call.end.min(end) - start,
            ..call
        })
        .collect()
}

/// `body` with each parameter replaced by its argument, as the form inserts it at a call.
pub(crate) fn expand_body(
    parser: &dyn NativeDialect,
    (definition, outer): (&SqlMacroDefinition, Option<&SqlMacroDefinition>),
    body: &ExpandedText,
    arguments: &[RenderedArgument],
) -> Result<BodyExpansion, SqlMacroError> {
    let (body, body_calls): (&str, &[TypedCall]) = (&body.sql, &body.calls);
    let invalid = |reason: String| {
        SqlMacroError::new(
            INVALID_DEFINITION_CODE,
            format!(
                "body of macro {}() is not a valid {} macro: {reason}",
                definition.name,
                definition.form.label()
            ),
        )
    };
    let scanned: Vec<Token> =
        tokens(body).map_err(|_| invalid("it has an unclosed quote or comment".to_owned()))?;
    let references: Vec<ParameterReference> = parameter_references(body, &scanned);
    let reads: Vec<SlotRead> =
        slot_reads(definition, (body, &scanned, &references)).map_err(|(_, error)| error)?;
    check_group_positions(definition, (body, &scanned), &reads).map_err(|(_, error)| error)?;
    let parsed_text: String = with_placeholders(body, &references);
    let parsed_body: ParsedBody = parsed(parser, definition.form, &parsed_text).map_err(invalid)?;
    if let Some(problem) = shape_problem(definition.form, &parsed_body.select) {
        return Err(invalid(problem.to_owned()));
    }
    let names: Vec<String> =
        defined_names(definition, (&parsed_text, &parsed_body.select), arguments)
            .map_err(invalid)?;
    check_relation_values(definition, &parsed_body.select).map_err(|reason| {
        SqlMacroError::new(
            ARGUMENT_CODE,
            format!("macro {}(): {reason}", definition.name),
        )
    })?;
    check_group_clashes(definition, &parsed_body.select, arguments)?;
    check_mapped_reads((parser, definition), (body, &references, &reads), arguments)?;
    check_forwarded_kinds(definition, arguments, outer)?;
    let (expanded, calls) = substituted_body(
        (definition, outer),
        (body, &scanned),
        (&references, &reads),
        (arguments, body_calls),
    );
    let kept: (usize, usize) = match definition.form {
        SqlMacroForm::Expression => trimmed_range(&expanded, 0),
        SqlMacroForm::CteList | SqlMacroForm::ColumnList => without_leading_keyword(&expanded),
    };
    Ok(BodyExpansion {
        sql: expanded[kept.0..kept.1].to_owned(),
        names,
        calls: within(calls, kept),
        like: None,
        captures: slot_captures((parser, definition), (body, &references, &reads), arguments),
    })
}

/// The byte range of `text[from..]` trimmed of surrounding whitespace.
fn trimmed_range(text: &str, from: usize) -> (usize, usize) {
    let rest: &str = &text[from..];
    let start: usize = from + rest.len() - rest.trim_start().len();
    (start, start + rest.trim().len())
}

/// `sql`, with a line break when it ends in a `--` comment and `followed` text would join that line.
pub(crate) fn with_line_comment_closed(sql: &str, followed: bool) -> String {
    let mut sql: String = sql.to_owned();
    if !followed {
        return sql;
    }
    let Ok(scanned) = tokens(&sql) else {
        return sql;
    };
    let mut rest: &str = &sql[scanned.last().map_or(0, |last| last.end)..];
    let mut open_line_comment: bool = false;
    loop {
        rest = rest.trim_start();
        if let Some(comment) = rest.strip_prefix("--") {
            match comment.split_once('\n') {
                Some((_, after)) => rest = after,
                None => {
                    open_line_comment = true;
                    break;
                }
            }
        } else if let Some(comment) = rest.strip_prefix("/*") {
            rest = comment.split_once("*/").map_or("", |(_, after)| after);
        } else {
            break;
        }
    }
    if open_line_comment {
        sql.push('\n');
    }
    sql
}

/// The byte range of the text after its first token, the body's `WITH` or `SELECT`, trimmed.
fn without_leading_keyword(text: &str) -> (usize, usize) {
    match tokens(text) {
        Ok(scanned) => match scanned.first() {
            Some(first) => trimmed_range(text, first.end),
            None => trimmed_range(text, 0),
        },
        Err(_) => trimmed_range(text, 0),
    }
}
