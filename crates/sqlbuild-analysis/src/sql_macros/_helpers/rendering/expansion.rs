//! One SQL macro call expanded: arguments bound and rendered, body expanded, result placed.

use std::sync::Arc;

use crate::sql_macros::_helpers::parsing::body::{expand_body, with_line_comment_closed};
use crate::sql_macros::_helpers::parsing::lexing::{AtConstruct, at_constructs, outer_sql_calls};
use crate::sql_macros::_helpers::parsing::lexing::{Token, line_column, tokens, tokens_with};
use crate::sql_macros::_helpers::parsing::literal_positions::{
    LiteralRules, literal_error, non_literal_use,
};
use crate::sql_macros::_helpers::parsing::parameters::{ParameterReference, parameter_references};
use crate::sql_macros::_helpers::rendering::arguments::{
    ShapeContext, argument_shape, bind_arguments, check_shape, is_atomic, split_arguments,
};
use crate::sql_macros::_helpers::rendering::cte_rename::{renamed_ctes, take_cte_argument};
use crate::sql_macros::_helpers::rendering::placement::top_level_parts;
use crate::sql_macros::_helpers::rendering::placement::{CallSite, cte_names, select_names};
use crate::sql_macros::_helpers::typing::return_types::returns_type_error;
use crate::sql_macros::_helpers::typing::returns_check::{check_body_returns, check_body_types};
use crate::sql_macros::_helpers::typing::typed_calls::{shifted, spliced};
use crate::sql_macros::constants::{
    ARGUMENT_CODE, CTE_ARGUMENT, CTE_NOUN, CYCLE_CODE, INVALID_DEFINITION_CODE,
    MISPLACED_CALL_CODE, NAME_CLASH_CODE,
};
use crate::sql_macros::errors::ExpansionFailure;
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::{
    ArgumentCheck, ArgumentExpectation, ArgumentShape, BodyExpansion, BoundArgument, CallArgument,
    CallPosition, CallScope, ExpandedText, InsertedNames, ParameterKind, RenderedArgument,
    RenderedDeclarations, SlotCapture, SqlMacroCall, SqlMacroCheck, SqlMacroDefinition,
    SqlMacroForm, SqlMacroLocation, TextContext, TypedCall,
};
use crate::sql_macros::types::{ConstantShape, SqlMacroHost, SqlMacroScope};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// The outer SQL macro calls of `text`, or why its `@` constructs could not be read.
pub(crate) fn sql_macro_calls(
    text: &str,
    syntax: Option<&LexicalSyntax>,
) -> Result<Vec<SqlMacroCall>, SqlMacroError> {
    let constructs: Vec<AtConstruct> = at_constructs(text, syntax).map_err(|_| {
        SqlMacroError::new(
            INVALID_DEFINITION_CODE,
            "SQL has an unclosed quote or comment",
        )
    })?;
    let calls: Vec<SqlMacroCall> = outer_sql_calls(&constructs)
        .into_iter()
        .filter_map(|(construct, in_python)| match construct {
            AtConstruct::Sql {
                name,
                start,
                end,
                arguments,
            } => Some(SqlMacroCall {
                name: name.clone(),
                start: *start,
                end: *end,
                replaced_end: *end,
                arguments: *arguments,
                in_python,
            }),
            _ => None,
        })
        .collect();
    if calls.iter().all(|call| call.in_python) {
        return Ok(calls);
    }
    let Ok(scanned) = tokens_with(text, syntax) else {
        return Ok(calls);
    };
    Ok(calls
        .into_iter()
        .map(|call| {
            if call.in_python {
                return call;
            }
            let site: CallSite<'_> =
                CallSite::new(text, &scanned, (call.start, call.end), call.end);
            SqlMacroCall {
                replaced_end: site.list_replaced_end(call.end),
                ..call
            }
        })
        .collect())
}

fn macro_location(definition: &SqlMacroDefinition, line: usize, column: usize) -> SqlMacroLocation {
    SqlMacroLocation {
        path: definition.path.clone(),
        line,
        column,
    }
}

fn body_location(definition: &SqlMacroDefinition) -> SqlMacroLocation {
    macro_location(definition, definition.body_line, definition.body_column)
}

/// The macro-file location of byte `offset` in its body.
fn body_offset_location(definition: &SqlMacroDefinition, offset: usize) -> SqlMacroLocation {
    let (line, column) = line_column(&definition.body, offset);
    if line == 1 {
        macro_location(
            definition,
            definition.body_line,
            definition.body_column + column - 1,
        )
    } else {
        macro_location(definition, definition.body_line + line - 1, column)
    }
}

/// The failure located in the macro body too, unless it already names a place in that file.
fn located<E>(failure: ExpansionFailure<E>, location: &SqlMacroLocation) -> ExpansionFailure<E> {
    match failure {
        ExpansionFailure::Macro(error)
            if error
                .locations
                .iter()
                .any(|known| known.path == location.path) =>
        {
            ExpansionFailure::Macro(error)
        }
        ExpansionFailure::Macro(error) => ExpansionFailure::Macro(error.at(location.clone())),
        host => host,
    }
}

/// `text` with every outer SQL macro call expanded, then its declarations rendered.
pub(crate) fn expand_text<H: SqlMacroHost>(
    host: &H,
    context: &TextContext<'_>,
) -> Result<ExpandedText, ExpansionFailure<H::Error>> {
    let syntax: Option<&LexicalSyntax> = host.lexical_syntax();
    let calls: Vec<SqlMacroCall> = sql_macro_calls(context.text, syntax)?;
    if calls.is_empty() {
        return Ok(ExpandedText {
            sql: host.render_declarations(context.scope, context.text)?.sql,
            calls: Vec::new(),
        });
    }
    let scanned: Vec<Token> = tokens_with(context.text, syntax).map_err(|_| {
        SqlMacroError::new(
            INVALID_DEFINITION_CODE,
            "SQL has an unclosed quote or comment",
        )
    })?;
    let mut inserted: InsertedNames = InsertedNames::default();
    let mut spliced_text: String = String::with_capacity(context.text.len());
    let mut typed: Vec<TypedCall> = Vec::new();
    let mut cursor: usize = 0;
    for call in &calls {
        let (expansion, updated) = expand_call(host, context, (&scanned, inserted), call)?;
        inserted = updated;
        spliced_text.push_str(&context.text[cursor..call.start]);
        typed.extend(shifted(expansion.calls, spliced_text.len()));
        spliced_text.push_str(&expansion.sql);
        cursor = call.replaced_end;
    }
    spliced_text.push_str(&context.text[cursor..]);
    let rendered: RenderedDeclarations = host.render_declarations(context.scope, &spliced_text)?;
    Ok(ExpandedText {
        calls: spliced(typed, &rendered.splices),
        sql: rendered.sql,
    })
}

/// One call's expansion as it replaces the call text in `context.text`.
pub(crate) fn expand_call<H: SqlMacroHost>(
    host: &H,
    context: &TextContext<'_>,
    caller: (&[Token], InsertedNames),
    call: &SqlMacroCall,
) -> Result<(ExpandedText, InsertedNames), ExpansionFailure<H::Error>> {
    let (scanned, inserted) = caller;
    let definition: Arc<SqlMacroDefinition> = host.definition(context.scope, &call.name)?;
    if let Some(position) = context.stack.iter().position(|name| *name == call.name) {
        let mut chain: Vec<String> = context.stack[position..]
            .iter()
            .map(|name| format!("{name}()"))
            .collect();
        chain.push(format!("{}()", call.name));
        return Err(SqlMacroError::new(
            CYCLE_CODE,
            format!(
                "SQL macros call each other in a cycle: {}",
                chain.join(" -> ")
            ),
        )
        .at(body_location(&definition))
        .helped("SQL macros may call other SQL macros, but not back into themselves")
        .into());
    }
    check_adapter_rules(&definition, host.adapter())?;
    let arguments_text: &str = &context.text[call.arguments.0..call.arguments.1];
    let (cte_name, arguments): (Option<String>, Vec<CallArgument>) =
        split_arguments(arguments_text)
            .and_then(|arguments| take_cte_argument(&definition, arguments))
            .map_err(|error| error.at(body_location(&definition)))?;
    let bound: Vec<BoundArgument> = bind_arguments(&definition, &arguments)
        .map_err(|error| error.at(body_location(&definition)))?;
    let macro_scope: SqlMacroScope = SqlMacroScope::Macro(Arc::clone(&definition));
    let mut rendered: Vec<RenderedArgument> = Vec::with_capacity(bound.len());
    for argument in &bound {
        let scope: &SqlMacroScope = if argument.from_default {
            &macro_scope
        } else {
            context.scope
        };
        rendered.push(rendered_argument(
            host,
            (&definition, argument),
            scope,
            context,
        )?);
    }
    let mut stack: Vec<String> = context.stack.to_vec();
    stack.push(call.name.clone());
    let body: ExpandedText = expand_text(
        host,
        &TextContext {
            text: &definition.body,
            scope: &macro_scope,
            stack: &stack,
            owner: &format!("macro {}()", definition.name),
        },
    )
    .map_err(|failure| located(failure, &body_location(&definition)))?;
    let body: ExpandedText = match &cte_name {
        Some(name) => renamed_ctes(&body, name).map_err(|reason| {
            SqlMacroError::new(
                INVALID_DEFINITION_CODE,
                format!(
                    "body of macro {}() is not a valid CTE-list macro: {reason}",
                    definition.name
                ),
            )
            .at(body_location(&definition))
        })?,
        None => body,
    };
    check_body_returns(
        &definition,
        &body,
        (host.adapter(), body_location(&definition)),
    )?;
    check_body_types(
        &definition,
        &body,
        (host.adapter(), body_location(&definition)),
    )?;
    let outer: Option<&SqlMacroDefinition> = match context.scope {
        SqlMacroScope::Macro(outer) => Some(outer.as_ref()),
        SqlMacroScope::Caller => None,
    };
    let mut expansion: BodyExpansion = expand_body((&definition, outer), &body, &rendered)
        .map_err(|error| error.at(body_location(&definition)))?;
    expansion.like = definition.returns_like.as_deref().and_then(|name| {
        rendered
            .iter()
            .find(|argument| {
                definition.parameters[argument.parameter]
                    .name
                    .eq_ignore_ascii_case(name)
            })
            .map(|argument| argument.sql.clone())
    });
    if call.in_python {
        return Ok((
            ExpandedText {
                sql: python_string(&with_line_comment_closed(&expansion.sql, true)),
                calls: Vec::new(),
            },
            inserted,
        ));
    }
    let site: CallSite<'_> = CallSite::new(
        context.text,
        scanned,
        (call.start, call.end),
        call.replaced_end,
    );
    let checked: bool = matches!(context.scope, SqlMacroScope::Caller) && context.stack.is_empty();
    if let Some(capture) = expansion.captures.first()
        && !checked
    {
        return Err(SqlMacroError::new(
            ARGUMENT_CODE,
            format!(
                "slot {}.{} is read plainly in a subquery of macro {}(), called from another macro, where its bare columns cannot be checked; write alias.@{}.{}",
                capture.relation, capture.slot, definition.name, capture.relation, capture.slot
            ),
        )
        .at(body_location(&definition))
        .helped("qualify the read with the alias of the relation it reads")
        .into());
    }
    if checked {
        let mut arguments: Vec<ArgumentCheck> = argument_checks(&definition, &bound, &rendered);
        arguments.extend(capture_checks(&definition, &expansion.captures));
        let check: SqlMacroCheck = SqlMacroCheck {
            macro_name: definition.name.clone(),
            scope: call_scope(context.text, scanned, call.start),
            plain_select: plain_select(context.text, scanned, call.start),
            arguments,
            call: None,
        };
        if !check.arguments.is_empty() {
            host.record_check(check, (call.start, call.end));
        }
    }
    let line_continues: bool = context.text[call.replaced_end..]
        .split('\n')
        .next()
        .is_some_and(|rest| !rest.trim().is_empty());
    placed(
        &definition,
        (&site, inserted, line_continues),
        &expansion,
        context.owner,
    )
    .map_err(ExpansionFailure::Macro)
}

/// One bound argument checked against its parameter and rendered from `scope`.
fn rendered_argument<H: SqlMacroHost>(
    host: &H,
    (definition, argument): (&SqlMacroDefinition, &BoundArgument),
    scope: &SqlMacroScope,
    context: &TextContext<'_>,
) -> Result<RenderedArgument, ExpansionFailure<H::Error>> {
    let parameter = &definition.parameters[argument.parameter];
    let parameter_location: SqlMacroLocation =
        macro_location(definition, parameter.line, parameter.column);
    let text: &str = argument.text.trim();
    let shape: ArgumentShape = argument_shape(text);
    let caller: Option<&SqlMacroDefinition> = match context.scope {
        SqlMacroScope::Macro(calling) => Some(calling),
        SqlMacroScope::Caller => None,
    };
    check_shape(
        definition,
        parameter,
        (text, &shape),
        ShapeContext {
            adapter: host.adapter(),
            caller,
        },
    )
    .map_err(|error| error.at(parameter_location.clone()))?;
    let expects = |accepts: &str| -> ExpansionFailure<H::Error> {
        SqlMacroError::new(
            ARGUMENT_CODE,
            format!(
                "argument {} of macro {}() expects {}: {accepts}; got {text}",
                parameter.name,
                definition.name,
                parameter.kind.label()
            ),
        )
        .at(parameter_location.clone())
        .into()
    };
    let rendered: ExpandedText = match (&parameter.kind, &shape) {
        (ParameterKind::Collection { .. }, ArgumentShape::InlineCollection) => ExpandedText {
            sql: host.render_collection(text)?,
            calls: Vec::new(),
        },
        (ParameterKind::Collection { .. }, ArgumentShape::ConstantReference { name }) => {
            if host.constant_shape(scope, name)? != ConstantShape::Collection {
                return Err(expects("a list or set constant"));
            }
            ExpandedText {
                sql: host.render_declarations(scope, text)?.sql,
                calls: Vec::new(),
            }
        }
        (
            ParameterKind::Constant { .. } | ParameterKind::Any { .. },
            ArgumentShape::ConstantReference { name },
        ) => {
            if host.constant_shape(scope, name)? != ConstantShape::Scalar {
                return Err(expects("a scalar constant"));
            }
            ExpandedText {
                sql: host.render_declarations(scope, text)?.sql,
                calls: Vec::new(),
            }
        }
        (ParameterKind::Relation { .. }, _) => ExpandedText {
            sql: text.to_owned(),
            calls: Vec::new(),
        },
        _ => expand_text(
            host,
            &TextContext {
                text,
                scope,
                stack: context.stack,
                owner: context.owner,
            },
        )?,
    };
    Ok(RenderedArgument {
        parameter: argument.parameter,
        sql: rendered.sql,
        shape,
        calls: rendered.calls,
        slots: argument.slots.clone(),
    })
}

/// `sql` as a double-quoted Python string literal holding no `"` and no line break.
fn python_string(sql: &str) -> String {
    let mut quoted: String = String::with_capacity(sql.len() + 2);
    quoted.push('"');
    for character in sql.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\x22"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

/// The expansion placed at the call with the calls it holds, or why it cannot stand there.
fn placed(
    definition: &SqlMacroDefinition,
    (site, inserted, line_continues): (&CallSite<'_>, InsertedNames, bool),
    expansion: &BodyExpansion,
    owner: &str,
) -> Result<(ExpandedText, InsertedNames), SqlMacroError> {
    let (text, inserted, prefix) = placed_text(
        definition,
        (site, inserted, line_continues),
        expansion,
        owner,
    )?;
    let mut calls: Vec<TypedCall> = shifted(expansion.calls.clone(), prefix).collect();
    if definition.form == SqlMacroForm::Expression {
        let comma: usize = site.expression_separator().len();
        calls.push(TypedCall {
            start: 0,
            end: text.len() - comma,
            macro_name: definition.name.clone(),
            returns: definition.returns.clone(),
            returns_like: expansion.like.clone(),
        });
    }
    Ok((ExpandedText { sql: text, calls }, inserted))
}

/// The expansion's text placed at the call, and where the expansion starts in it.
fn placed_text(
    definition: &SqlMacroDefinition,
    (site, inserted, line_continues): (&CallSite<'_>, InsertedNames, bool),
    expansion: &BodyExpansion,
    owner: &str,
) -> Result<(String, InsertedNames, usize), SqlMacroError> {
    let closed = |after: &str| {
        let followed: bool = match after.chars().next() {
            None => line_continues,
            Some(next) => next != '\n' && next != '\r',
        };
        with_line_comment_closed(&expansion.sql, followed)
    };
    let position: CallPosition = site.position();
    let name: &str = &definition.name;
    match definition.form {
        SqlMacroForm::Expression => {
            if position == CallPosition::CteList {
                return Err(SqlMacroError::new(MISPLACED_CALL_CODE, format!("expression macro {name}() is called where a CTE belongs"))
                    .helped("call an expression macro where an expression belongs, such as in a select list or WHERE clause"));
            }
            let comma: &str = site.expression_separator();
            if site.expression_needs_parentheses() && !is_atomic(&expansion.sql) {
                Ok((format!("({}){comma}", closed(")")), inserted, 1))
            } else {
                Ok((format!("{}{comma}", closed(comma)), inserted, 0))
            }
        }
        SqlMacroForm::CteList => {
            if position != CallPosition::CteList {
                return Err(SqlMacroError::new(MISPLACED_CALL_CODE, format!("CTE-list macro {name}() must be called inside a WITH list"))
                    .helped(format!("call it between CTEs, such as WITH a AS (...), @sql.{name}(a) b AS (...) SELECT ...")));
            }
            let Some((list, _)) = site.clause() else {
                return Err(SqlMacroError::new(
                    MISPLACED_CALL_CODE,
                    format!("CTE-list macro {name}() must be called inside a WITH list"),
                ));
            };
            let defined: Vec<(String, usize)> = cte_names(site.sql, site.tokens, list);
            let inserted: InsertedNames = check_clashes(
                definition,
                (&defined, inserted, site.tokens[list].start),
                expansion,
                ("CTE", owner),
            )?;
            let (before, after) = site.cte_separators();
            Ok((
                format!("{before}{}{after}", closed(after)),
                inserted,
                before.len(),
            ))
        }
        SqlMacroForm::ColumnList => {
            if position != CallPosition::SelectList {
                return Err(SqlMacroError::new(MISPLACED_CALL_CODE, format!("column-list macro {name}() must be called as an item of a select list"))
                    .helped(format!("call it where a column belongs, such as SELECT o.id, @sql.{name}(o) FROM orders AS o")));
            }
            let Some((list, _)) = site.clause() else {
                return Err(SqlMacroError::new(
                    MISPLACED_CALL_CODE,
                    format!(
                        "column-list macro {name}() must be called as an item of a select list"
                    ),
                ));
            };
            if site.has_alias() {
                return Err(SqlMacroError::new(
                    MISPLACED_CALL_CODE,
                    format!("column-list macro {name}() cannot take an alias; it inserts several columns, each named in the macro"),
                )
                .helped(format!("remove the alias after @sql.{name}(...), or rename the columns in {}", definition.path)));
            }
            let defined: Vec<(String, usize)> = select_names(site.sql, site.tokens, list);
            let inserted: InsertedNames = check_clashes(
                definition,
                (&defined, inserted, site.tokens[list].start),
                expansion,
                ("column", owner),
            )?;
            let (before, after) = site.column_separators();
            Ok((
                format!("{before}{}{after}", closed(after)),
                inserted,
                before.len(),
            ))
        }
    }
}

/// Rejects a name the expansion defines that the caller, or an earlier call, already defines.
fn check_clashes(
    definition: &SqlMacroDefinition,
    (defined, inserted, list): (&[(String, usize)], InsertedNames, usize),
    expansion: &BodyExpansion,
    (noun, owner): (&str, &str),
) -> Result<InsertedNames, SqlMacroError> {
    for name in &expansion.names {
        let clash_location: SqlMacroLocation =
            body_offset_location(definition, definition.body.find(name.as_str()).unwrap_or(0));
        let lists_with = |plural: &str| {
            if definition.form == SqlMacroForm::CteList {
                return format!(
                    "pass {CTE_ARGUMENT} := <name> to give the CTEs one call inserts their own names, as in @sql.{}(..., {CTE_ARGUMENT} := first_orders), or rename one of the {plural}",
                    definition.name
                );
            }
            format!(
                "rename one of the {plural}; a model can't call a {} macro whose {noun} names it already uses",
                definition.form.label()
            )
        };
        if defined
            .iter()
            .any(|(existing, _)| existing.eq_ignore_ascii_case(name))
        {
            return Err(SqlMacroError::new(
                NAME_CLASH_CODE,
                format!("{noun} \"{name}\" from macro {}() clashes with a {noun} already defined in {owner}", definition.name),
            )
            .at(clash_location)
            .helped(lists_with(if noun == CTE_NOUN { "CTEs" } else { "columns" })));
        }
        if let Some((_, _, other)) = inserted
            .lists
            .iter()
            .find(|(start, existing, _)| *start == list && existing.eq_ignore_ascii_case(name))
        {
            return Err(SqlMacroError::new(
                NAME_CLASH_CODE,
                format!("{noun} \"{name}\" from macro {}() clashes with a {noun} from macro {other}() in {owner}", definition.name),
            )
            .at(clash_location)
            .helped(lists_with(if noun == CTE_NOUN { "CTEs" } else { "columns" })));
        }
    }
    let mut updated: InsertedNames = inserted;
    for name in &expansion.names {
        updated
            .lists
            .push((list, name.clone(), definition.name.clone()));
    }
    Ok(updated)
}

/// Rejects types the adapter cannot parse and runtime parameters where it needs a literal.
fn check_adapter_rules(
    definition: &SqlMacroDefinition,
    adapter: &str,
) -> Result<(), SqlMacroError> {
    check_adapter_types(definition, adapter)?;
    let upper: String = definition.body.to_ascii_uppercase();
    if !upper.contains("ROUND") && !upper.contains("_TRUNC") {
        return Ok(());
    }
    let Ok(scanned) = tokens(&definition.body) else {
        return Ok(());
    };
    let references: Vec<ParameterReference> = parameter_references(&definition.body, &scanned);
    match non_literal_use(
        definition,
        &scanned,
        &references,
        LiteralRules::Adapter(adapter),
    ) {
        Some(found) => Err(literal_error(definition, &found)
            .at(body_offset_location(definition, found.reference.start))),
        None => Ok(()),
    }
}

fn check_adapter_types(
    definition: &SqlMacroDefinition,
    adapter: &str,
) -> Result<(), SqlMacroError> {
    let invalid = |data_type: &str, context: String| -> Option<SqlMacroError> {
        if adapter.is_empty() {
            return None;
        }
        returns_type_error(data_type, adapter, &context).map(|message| {
            SqlMacroError::new(INVALID_DEFINITION_CODE, message)
                .helped("use a SQL type the warehouse accepts, as in schema declarations")
        })
    };
    for parameter in &definition.parameters {
        let types: Vec<&str> = match &parameter.kind {
            ParameterKind::Expression { data_type }
            | ParameterKind::Constant { data_type }
            | ParameterKind::Collection { data_type, .. } => vec![data_type.as_str()],
            ParameterKind::Relation { columns, .. } => columns
                .iter()
                .map(|(_, data_type)| data_type.as_str())
                .collect(),
            ParameterKind::Any { .. } | ParameterKind::Enum { .. } => Vec::new(),
        };
        for data_type in types {
            if let Some(error) = invalid(
                data_type,
                format!("argument {} of macro {}()", parameter.name, definition.name),
            ) {
                return Err(error.at(macro_location(definition, parameter.line, parameter.column)));
            }
        }
    }
    if let Some(returns) = &definition.returns
        && let Some(message) = returns_type_error(
            returns,
            adapter,
            &format!("returns of macro {}()", definition.name),
        )
    {
        return Err(SqlMacroError::new(INVALID_DEFINITION_CODE, message)
            .helped("use a SQL type the warehouse accepts, as in schema declarations")
            .at(macro_location(definition, 1, 1)));
    }
    Ok(())
}

/// The checks the caller's column types decide, for arguments the call itself passed.
fn argument_checks(
    definition: &SqlMacroDefinition,
    bound: &[BoundArgument],
    rendered: &[RenderedArgument],
) -> Vec<ArgumentCheck> {
    let mut checks: Vec<ArgumentCheck> = Vec::new();
    for (argument, rendered) in bound.iter().zip(rendered) {
        if argument.from_default {
            continue;
        }
        let parameter = &definition.parameters[argument.parameter];
        let expected: ArgumentExpectation = match (&parameter.kind, &rendered.shape) {
            (ParameterKind::Relation { columns, .. }, ArgumentShape::Identifier { .. }) => {
                ArgumentExpectation::Relation {
                    columns: columns.clone(),
                    slots: rendered.slots.clone(),
                }
            }
            (
                ParameterKind::Expression { data_type },
                ArgumentShape::Expression | ArgumentShape::Identifier { .. },
            ) if !rendered.sql.contains('@') && !rendered.sql.contains("__") => {
                ArgumentExpectation::Typed {
                    data_type: data_type.clone(),
                }
            }
            (
                ParameterKind::Any {
                    family: Some(family),
                },
                ArgumentShape::Expression | ArgumentShape::Identifier { .. },
            ) if !rendered.sql.contains('@') && !rendered.sql.contains("__") => {
                ArgumentExpectation::Family { family: *family }
            }
            _ => continue,
        };
        checks.push(ArgumentCheck {
            parameter: parameter.name.clone(),
            text: rendered.sql.trim().to_owned(),
            expected,
            declared_at: macro_location(definition, parameter.line, parameter.column),
        });
    }
    checks
}

/// One check per relation a plain group read in a subquery meets, that it holds none of its columns.
fn capture_checks(definition: &SqlMacroDefinition, captures: &[SlotCapture]) -> Vec<ArgumentCheck> {
    let mut checks: Vec<ArgumentCheck> = Vec::new();
    for capture in captures {
        let Some(parameter) = definition.parameter(&capture.relation) else {
            continue;
        };
        for source in &capture.sources {
            checks.push(ArgumentCheck {
                parameter: parameter.name.clone(),
                text: source.clone().unwrap_or_default(),
                expected: ArgumentExpectation::Capture {
                    slot: capture.slot.clone(),
                    columns: capture.columns.clone(),
                    source: source.clone(),
                },
                declared_at: macro_location(definition, parameter.line, parameter.column),
            });
        }
    }
    checks
}

/// The top-level query part holding the byte `offset` of `text`.
fn call_scope(text: &str, scanned: &[Token], offset: usize) -> CallScope {
    match top_level_parts(text, scanned) {
        Some(parts) => parts.scope_of(offset),
        None => CallScope::Main,
    }
}

/// Whether the call at byte `offset` of `text` stands in its scope's one select.
fn plain_select(text: &str, scanned: &[Token], offset: usize) -> bool {
    top_level_parts(text, scanned).is_some_and(|parts| parts.plain_select(text, scanned, offset))
}
