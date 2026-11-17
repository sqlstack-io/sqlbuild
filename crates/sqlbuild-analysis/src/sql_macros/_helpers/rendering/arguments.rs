//! A call's argument list: split, bound to parameters and checked against their types.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::sql_macros::_helpers::parsing::header::{split_top_level, unquoted};
use crate::sql_macros::_helpers::parsing::lexing::{Token, TokenKind, closing_token, tokens};
use crate::sql_macros::_helpers::rendering::slot_mappings::slot_mappings;
use crate::sql_macros::_helpers::typing::returns_check::adapter_dialect;
use crate::sql_macros::constants::{
    ARGUMENT_CODE, BOOLEAN_WORDS, NAMED_ARGUMENT_MIN_TOKENS, NULL_WORD, REFERENCE_FUNCTIONS,
};
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::{
    ArgumentShape, BoundArgument, CallArgument, ParameterKind, SlotMapping, SqlMacroDefinition,
    SqlMacroParameter, TypeFamily,
};
use crate::type_system::main::normalize_type::normalize_type;

const LITERAL_WORDS: [&str; 3] = ["TRUE", "FALSE", "NULL"];
const TYPED_LITERAL_WORDS: [&str; 5] = ["DATE", "TIMESTAMP", "TIME", "INTERVAL", "TIMESTAMPTZ"];

/// Each top-level argument of `arguments` with its `name :=` prefix, if any.
pub(crate) fn split_arguments(arguments: &str) -> Result<Vec<CallArgument>, SqlMacroError> {
    let scanned: Vec<Token> = tokens(arguments).map_err(|_| {
        SqlMacroError::new(
            ARGUMENT_CODE,
            "macro call arguments have an unclosed quote or comment",
        )
    })?;
    let mut found: Vec<CallArgument> = Vec::new();
    for part in split_top_level(arguments, &scanned) {
        let named: bool = part.len() >= NAMED_ARGUMENT_MIN_TOKENS
            && part[0].kind == TokenKind::Word
            && part[1].is(arguments, ":=");
        let slot: bool = part.len() >= NAMED_ARGUMENT_MIN_TOKENS + 2
            && part[0].kind == TokenKind::Word
            && part[1].is(arguments, ".")
            && matches!(part[2].kind, TokenKind::Word | TokenKind::Quoted)
            && part[3].is(arguments, ":=");
        let (name, slot, value): (Option<String>, Option<String>, &[Token]) = if named {
            (Some(part[0].text(arguments).to_owned()), None, &part[2..])
        } else if slot {
            (
                Some(part[0].text(arguments).to_owned()),
                Some(unquoted(part[2].text(arguments))),
                &part[4..],
            )
        } else {
            (None, None, part)
        };
        let (Some(first), Some(last)) = (value.first(), value.last()) else {
            return Err(SqlMacroError::new(
                ARGUMENT_CODE,
                "a named macro argument needs a value after :=",
            ));
        };
        found.push(CallArgument {
            name,
            slot,
            text: arguments[first.start..last.end].to_owned(),
            offset: first.start,
        });
    }
    Ok(found)
}

/// Each parameter's argument: positional first, then named, then its default.
pub(crate) fn bind_arguments(
    definition: &SqlMacroDefinition,
    arguments: &[CallArgument],
) -> Result<Vec<BoundArgument>, SqlMacroError> {
    let name: &str = &definition.name;
    let mut bound: Vec<Option<BoundArgument>> = vec![None; definition.parameters.len()];
    let mut seen_named: bool = false;
    let mut slot_arguments: Vec<&CallArgument> = Vec::new();
    for (position, argument) in arguments.iter().enumerate() {
        if argument.slot.is_some() {
            seen_named = true;
            slot_arguments.push(argument);
            continue;
        }
        let index: usize = match &argument.name {
            Some(argument_name) => {
                seen_named = true;
                definition
                    .parameters
                    .iter()
                    .position(|parameter| parameter.name.eq_ignore_ascii_case(argument_name))
                    .ok_or_else(|| {
                        SqlMacroError::new(
                            ARGUMENT_CODE,
                            format!("macro {name}() has no argument {argument_name}"),
                        )
                        .helped(format!("its arguments are {}", parameter_names(definition)))
                    })?
            }
            None => {
                if seen_named {
                    return Err(SqlMacroError::new(
                        ARGUMENT_CODE,
                        format!(
                            "positional argument after a named argument in a call to macro {name}()"
                        ),
                    ));
                }
                if position >= definition.parameters.len() {
                    return Err(SqlMacroError::new(
                        ARGUMENT_CODE,
                        format!(
                            "macro {name}() takes {} argument{}; the call passes {}",
                            definition.parameters.len(),
                            if definition.parameters.len() == 1 {
                                ""
                            } else {
                                "s"
                            },
                            arguments.len()
                        ),
                    ));
                }
                position
            }
        };
        if bound[index].is_some() {
            return Err(SqlMacroError::new(
                ARGUMENT_CODE,
                format!(
                    "argument {} of macro {name}() is passed twice",
                    definition.parameters[index].name
                ),
            ));
        }
        bound[index] = Some(BoundArgument {
            parameter: index,
            text: argument.text.clone(),
            from_default: false,
            offset: Some(argument.offset),
            slots: Vec::new(),
        });
    }
    let mut mappings: Vec<Vec<SlotMapping>> = slot_mappings(definition, &slot_arguments)?;
    let mut complete: Vec<BoundArgument> = Vec::with_capacity(bound.len());
    for (index, argument) in bound.into_iter().enumerate() {
        let parameter: &SqlMacroParameter = &definition.parameters[index];
        let slots: Vec<SlotMapping> = std::mem::take(&mut mappings[index]);
        match (argument, &parameter.default) {
            (Some(argument), _) => complete.push(BoundArgument { slots, ..argument }),
            (None, Some(default)) => complete.push(BoundArgument {
                parameter: index,
                text: default.clone(),
                from_default: true,
                offset: None,
                slots,
            }),
            (None, None) => {
                return Err(SqlMacroError::new(
                    ARGUMENT_CODE,
                    format!(
                        "call to macro {name}() is missing argument {} {}",
                        parameter.name,
                        parameter.kind.label()
                    ),
                ));
            }
        }
    }
    Ok(complete)
}

pub(crate) fn parameter_names(definition: &SqlMacroDefinition) -> String {
    if definition.parameters.is_empty() {
        return "none".to_owned();
    }
    definition
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

/// How an argument is written, read from its tokens alone.
pub(crate) fn argument_shape(text: &str) -> ArgumentShape {
    let Ok(scanned) = tokens(text) else {
        return ArgumentShape::Expression;
    };
    match scanned.as_slice() {
        [] => ArgumentShape::Expression,
        [single] => match single.kind {
            TokenKind::Number | TokenKind::Text => ArgumentShape::Literal,
            TokenKind::Word
                if LITERAL_WORDS
                    .iter()
                    .any(|word| single.text(text).eq_ignore_ascii_case(word)) =>
            {
                ArgumentShape::Literal
            }
            TokenKind::Word | TokenKind::Quoted => ArgumentShape::Identifier {
                name: single.text(text).to_owned(),
            },
            _ => ArgumentShape::Expression,
        },
        [at, word] if at.is(text, "@") && word.kind == TokenKind::Word && word.start == at.end => {
            ArgumentShape::ParameterReference {
                name: word.text(text).to_owned(),
            }
        }
        [sign, number] if sign.is(text, "-") && number.kind == TokenKind::Number => {
            ArgumentShape::Literal
        }
        [word, value]
            if word.kind == TokenKind::Word
                && value.kind == TokenKind::Text
                && TYPED_LITERAL_WORDS
                    .iter()
                    .any(|typed| word.text(text).eq_ignore_ascii_case(typed)) =>
        {
            ArgumentShape::Literal
        }
        [at, keyword, open, name, close]
            if at.is(text, "@")
                && keyword.is_word(text, "const")
                && open.is(text, "(")
                && matches!(name.kind, TokenKind::Quoted | TokenKind::Text)
                && close.is(text, ")") =>
        {
            ArgumentShape::ConstantReference {
                name: unquote(name.text(text)),
            }
        }
        [at, keyword, open, name, close, dot, member]
            if at.is(text, "@")
                && keyword.is_word(text, "enum")
                && open.is(text, "(")
                && matches!(name.kind, TokenKind::Quoted | TokenKind::Text)
                && close.is(text, ")")
                && dot.is(text, ".")
                && member.kind == TokenKind::Word =>
        {
            ArgumentShape::EnumMember {
                name: unquote(name.text(text)),
                member: member.text(text).to_owned(),
            }
        }
        [first, ..]
            if (first.is(text, "[") || first.is(text, "{"))
                && closing_token(text, &scanned, 0) == Some(scanned.len() - 1) =>
        {
            ArgumentShape::InlineCollection
        }
        [function, open, name, close]
            if function.kind == TokenKind::Word
                && REFERENCE_FUNCTIONS.contains(&function.text(text))
                && open.is(text, "(")
                && matches!(name.kind, TokenKind::Quoted | TokenKind::Text)
                && close.is(text, ")") =>
        {
            ArgumentShape::Reference {
                call: text.trim().to_owned(),
            }
        }
        _ => ArgumentShape::Expression,
    }
}

fn unquote(text: &str) -> String {
    crate::sql_macros::_helpers::parsing::header::unquoted(text)
}

/// The type family of a SQL literal argument.
fn literal_family(text: &str) -> TypeFamily {
    let trimmed: &str = text.trim();
    let upper: String = trimmed.to_ascii_uppercase();
    if upper == NULL_WORD {
        return TypeFamily::Null;
    }
    if BOOLEAN_WORDS.contains(&upper.as_str()) {
        return TypeFamily::Boolean;
    }
    if trimmed.starts_with('\'') || trimmed.starts_with('$') {
        return TypeFamily::Text;
    }
    if let Some(word) = upper.split_whitespace().next()
        && TYPED_LITERAL_WORDS.contains(&word)
    {
        return match word {
            "DATE" => TypeFamily::Date,
            "TIME" => TypeFamily::Time,
            "INTERVAL" => TypeFamily::Unknown,
            _ => TypeFamily::Timestamp,
        };
    }
    let digits: &str = trimmed.trim_start_matches('-').trim();
    if digits.contains(['.', 'e', 'E']) {
        TypeFamily::Decimal
    } else {
        TypeFamily::Integer
    }
}

/// The adapter whose type rules apply and the calling macro an `@name` argument forwards from.
#[derive(Clone, Copy)]
pub(crate) struct ShapeContext<'a> {
    pub(crate) adapter: &'a str,
    pub(crate) caller: Option<&'a SqlMacroDefinition>,
}

/// Whether `shape` is something `parameter` accepts; the caller checks references it resolves.
pub(crate) fn check_shape(
    definition: &SqlMacroDefinition,
    parameter: &SqlMacroParameter,
    (argument, shape): (&str, &ArgumentShape),
    context: ShapeContext<'_>,
) -> Result<(), SqlMacroError> {
    let macro_name: &str = &definition.name;
    let name: &str = &parameter.name;
    let shown: &str = argument.trim();
    let expects = |accepts: &str| {
        SqlMacroError::new(
            ARGUMENT_CODE,
            format!(
                "argument {name} of macro {macro_name}() expects {}: {accepts}; got {shown}",
                parameter.kind.label()
            ),
        )
    };
    match (&parameter.kind, shape) {
        (_, ArgumentShape::ParameterReference { name: forwarded }) => {
            let Some(outer) = context
                .caller
                .and_then(|caller| caller.parameter(forwarded).map(|found| (caller, found)))
            else {
                return Ok(());
            };
            if forwards(&outer.1.kind, &parameter.kind) {
                Ok(())
            } else {
                Err(expects(&format!(
                    "a value of that kind; @{forwarded} is {} argument {} of macro {}()",
                    article(&outer.1.kind.label()),
                    outer.1.name,
                    outer.0.name
                ))
                .helped(match &parameter.kind {
                    ParameterKind::Constant { .. } => format!(
                        "declare {} CONST in macro {}() so it is known at compile time",
                        outer.1.name, outer.0.name
                    ),
                    _ => "forward an argument of the same kind".to_owned(),
                }))
            }
        }
        (ParameterKind::Constant { data_type }, ArgumentShape::EnumMember { .. }) => {
            if fits(TypeFamily::Text, type_family(data_type, context.adapter)) {
                Ok(())
            } else {
                Err(expects("a literal, a scalar constant or an enum value of that type"))
            }
        }
        (ParameterKind::Expression { data_type }, ArgumentShape::Literal)
        | (ParameterKind::Constant { data_type }, ArgumentShape::Literal) => {
            let actual: TypeFamily = literal_family(argument);
            if fits(actual, type_family(data_type, context.adapter)) {
                Ok(())
            } else {
                Err(SqlMacroError::new(
                    ARGUMENT_CODE,
                    format!(
                        "argument {name} of macro {macro_name}() expects {data_type}; {shown} is {}",
                        family_label(actual)
                    ),
                ))
            }
        }
        (ParameterKind::Expression { .. }, ArgumentShape::InlineCollection) => {
            Err(expects("a SQL expression"))
        }
        (ParameterKind::Expression { .. }, _) => Ok(()),
        (
            ParameterKind::Any { .. },
            ArgumentShape::InlineCollection | ArgumentShape::Reference { .. },
        ) => {
            Err(expects("a scalar SQL expression")
                .helped("an ANY argument takes one value per row, such as a column, a literal or an expression; pass lists to LIST arguments and relations to RELATION arguments"))
        }
        (
            ParameterKind::Any {
                family: Some(family),
            },
            ArgumentShape::Literal,
        ) => {
            let actual: TypeFamily = literal_family(argument);
            if family.admits(actual) {
                Ok(())
            } else {
                Err(SqlMacroError::new(
                    ARGUMENT_CODE,
                    format!(
                        "argument {name} of macro {macro_name}() expects {}; {shown} is {}",
                        family.noun(),
                        family_label(actual)
                    ),
                ))
            }
        }
        (ParameterKind::Any { .. }, _) => Ok(()),
        (ParameterKind::Constant { .. }, ArgumentShape::ConstantReference { .. }) => Ok(()),
        (ParameterKind::Constant { .. }, _) => Err(expects("a literal, a scalar constant or an enum value")
            .helped(format!("pass a literal, such as 2, or a constant, such as @const(\"{name}\"); a CONST argument is known at compile time, so a column or expression cannot be passed"))),
        (
            ParameterKind::Collection { .. },
            ArgumentShape::InlineCollection | ArgumentShape::ConstantReference { .. },
        ) => Ok(()),
        (ParameterKind::Collection { .. }, _) => {
            Err(expects("an inline list or set, or a list or set constant")
                .helped("pass a list, such as [\"GB\", \"FR\"], or a constant, such as @const(\"supported_countries\")"))
        }
        (ParameterKind::Enum { name: enum_name }, ArgumentShape::EnumMember { name: given, .. })
            if given == enum_name =>
        {
            Ok(())
        }
        (ParameterKind::Enum { name: enum_name }, _) => Err(expects(&format!(
            "a member of enum {enum_name}, such as @enum(\"{enum_name}\").MEMBER"
        ))),
        (
            ParameterKind::Relation { .. },
            ArgumentShape::Identifier { .. } | ArgumentShape::Reference { .. },
        ) => Ok(()),
        (ParameterKind::Relation { .. }, _) => Err(expects(
            "a CTE name, a table alias, or __ref(), __source() or __seed()",
        )),
    }
}

/// Whether an argument of kind `outer`, forwarded as `@name`, fits a parameter of kind `inner`.
fn forwards(outer: &ParameterKind, inner: &ParameterKind) -> bool {
    match inner {
        ParameterKind::Expression { .. } | ParameterKind::Any { .. } => matches!(
            outer,
            ParameterKind::Expression { .. }
                | ParameterKind::Any { .. }
                | ParameterKind::Constant { .. }
                | ParameterKind::Enum { .. }
        ),
        ParameterKind::Constant { .. } => {
            matches!(
                outer,
                ParameterKind::Constant { .. } | ParameterKind::Enum { .. }
            )
        }
        ParameterKind::Collection { .. } => matches!(outer, ParameterKind::Collection { .. }),
        ParameterKind::Enum { name } => {
            matches!(outer, ParameterKind::Enum { name: outer_name } if outer_name == name)
        }
        ParameterKind::Relation { .. } => matches!(outer, ParameterKind::Relation { .. }),
    }
}

fn article(label: &str) -> String {
    let vowel: bool = label.starts_with(['A', 'E', 'I', 'O', 'U']);
    format!("{} {label}", if vowel { "an" } else { "a" })
}

/// A type family as messages name it.
pub(crate) fn family_label(family: TypeFamily) -> &'static str {
    match family {
        TypeFamily::Integer => "an integer",
        TypeFamily::Decimal => "a decimal number",
        TypeFamily::Float => "a floating-point number",
        TypeFamily::Text => "a string",
        TypeFamily::Boolean => "a boolean",
        TypeFamily::Date => "a date",
        TypeFamily::Timestamp => "a timestamp",
        TypeFamily::Time => "a time",
        TypeFamily::Null => "NULL",
        TypeFamily::Unknown => "of an unknown type",
    }
}

/// Whether the SQL text is one self-delimited term that needs no parentheses around it.
pub(crate) fn is_atomic(sql: &str) -> bool {
    let Ok(scanned) = tokens(sql) else {
        return false;
    };
    let Some(first) = scanned.first() else {
        return false;
    };
    if matches!(
        argument_shape(sql),
        ArgumentShape::ParameterReference { .. }
    ) {
        return true;
    }
    if scanned.len() == 1 {
        return matches!(
            first.kind,
            TokenKind::Word | TokenKind::Quoted | TokenKind::Number | TokenKind::Text
        );
    }
    if first.is(sql, "(") {
        return closing_token(sql, &scanned, 0) == Some(scanned.len() - 1);
    }
    if first.is_word(sql, "CASE") {
        let mut depth: isize = 0;
        for (index, token) in scanned.iter().enumerate() {
            if token.is_word(sql, "CASE") {
                depth += 1;
            } else if token.is_word(sql, "END") {
                depth -= 1;
                if depth == 0 {
                    return index == scanned.len() - 1;
                }
            }
        }
        return false;
    }
    let mut index: usize = 0;
    while index < scanned.len() {
        let token: &Token = &scanned[index];
        if !matches!(token.kind, TokenKind::Word | TokenKind::Quoted) {
            return false;
        }
        match scanned.get(index + 1) {
            None => return true,
            Some(next) if next.is(sql, ".") => index += 2,
            Some(next) if next.is(sql, "(") => {
                return closing_token(sql, &scanned, index + 1) == Some(scanned.len() - 1);
            }
            Some(_) => return false,
        }
    }
    false
}

thread_local! {
    /// Each declared type's family by adapter, since normalizing a type parses it.
    static FAMILIES: RefCell<HashMap<String, HashMap<String, TypeFamily>>> =
        RefCell::new(HashMap::new());
}

/// The family of a declared type, normalized as schema declaration types are for the adapter.
pub(crate) fn type_family(data_type: &str, adapter: &str) -> TypeFamily {
    FAMILIES.with(|families| {
        if let Some(family) = families
            .borrow()
            .get(adapter)
            .and_then(|by_type| by_type.get(data_type))
        {
            return *family;
        }
        let family: TypeFamily = normalized_family(data_type, adapter);
        families
            .borrow_mut()
            .entry(adapter.to_owned())
            .or_default()
            .insert(data_type.to_owned(), family);
        family
    })
}

/// The family `normalize_type` gives a declared type for the adapter's dialect.
fn normalized_family(data_type: &str, adapter: &str) -> TypeFamily {
    use crate::type_system::models::TypeFamily as Normalized;
    let dialect: &str = adapter_dialect(adapter);
    let normalized = normalize_type(data_type, dialect)
        .or_else(|_| normalize_type(data_type, "generic"))
        .map(|normalization| normalization.normalized.family);
    match normalized {
        Ok(Normalized::Integer) => TypeFamily::Integer,
        Ok(Normalized::Decimal) => TypeFamily::Decimal,
        Ok(Normalized::Float) => TypeFamily::Float,
        Ok(Normalized::String) => TypeFamily::Text,
        Ok(Normalized::Boolean) => TypeFamily::Boolean,
        Ok(Normalized::Date) => TypeFamily::Date,
        Ok(Normalized::Timestamp | Normalized::Datetime) => TypeFamily::Timestamp,
        Ok(Normalized::Other) | Err(_) => TypeFamily::Unknown,
    }
}

/// Whether a value of type `actual` may be passed where `expected` is declared.
pub(crate) fn fits(actual: TypeFamily, expected: TypeFamily) -> bool {
    use TypeFamily::{
        Boolean, Date, Decimal, Float, Integer, Null, Text, Time, Timestamp, Unknown,
    };
    matches!(
        (actual, expected),
        (Unknown | Null, _)
            | (_, Unknown)
            | (Integer, Integer | Decimal | Float)
            | (Decimal | Float, Decimal | Float)
            | (Date, Date | Timestamp)
            | (Timestamp, Timestamp)
            | (Time, Time)
            | (Text, Text)
            | (Boolean, Boolean)
    )
}

/// The rendered literal and type family of each value of an inline collection's inner tokens.
pub(crate) fn collection_items(
    text: &str,
    inner: &[Token],
    invalid: &dyn Fn(&str) -> SqlMacroError,
) -> Result<Vec<(String, TypeFamily)>, SqlMacroError> {
    let mut items: Vec<(String, TypeFamily)> = Vec::new();
    let mut index: usize = 0;
    while index < inner.len() {
        let negative: bool = inner[index].is(text, "-");
        if negative {
            index += 1;
        }
        let value: &Token = inner
            .get(index)
            .ok_or_else(|| invalid("ends with a sign"))?;
        items.push(
            collection_item(text, value, negative)
                .ok_or_else(|| invalid("holds a value that is not a literal"))?,
        );
        index += 1;
        match inner.get(index) {
            None => {}
            Some(comma) if comma.is(text, ",") => index += 1,
            Some(_) => return Err(invalid("needs commas between its values")),
        }
    }
    Ok(items)
}

/// One collection value as a SQL literal with its type family, if it is a literal.
fn collection_item(text: &str, value: &Token, negative: bool) -> Option<(String, TypeFamily)> {
    match value.kind {
        TokenKind::Quoted | TokenKind::Text => Some((
            format!("'{}'", unquoted(value.text(text)).replace('\'', "''")),
            TypeFamily::Text,
        )),
        TokenKind::Number => {
            let digits: &str = value.text(text);
            let family: TypeFamily = if digits.contains(['.', 'e', 'E']) {
                TypeFamily::Decimal
            } else {
                TypeFamily::Integer
            };
            Some((
                format!("{}{digits}", if negative { "-" } else { "" }),
                family,
            ))
        }
        TokenKind::Word if !negative => match value.text(text).to_ascii_lowercase().as_str() {
            "true" => Some(("TRUE".to_owned(), TypeFamily::Boolean)),
            "false" => Some(("FALSE".to_owned(), TypeFamily::Boolean)),
            "null" => Some(("NULL".to_owned(), TypeFamily::Null)),
            _ => None,
        },
        _ => None,
    }
}

/// Why a collection's values do not form one list or set, if they do not.
pub(crate) fn collection_problem(items: &[(String, TypeFamily)], set: bool) -> Option<String> {
    let typed: Vec<TypeFamily> = items
        .iter()
        .map(|(_, family)| *family)
        .filter(|family| *family != TypeFamily::Null)
        .collect();
    let Some(first) = typed.first() else {
        return Some("needs at least one value that is not null".to_owned());
    };
    if typed
        .iter()
        .any(|family| family != first && !(is_numeric(*family) && is_numeric(*first)))
    {
        return Some("mixes values of different types".to_owned());
    }
    if !set {
        return None;
    }
    items.iter().enumerate().find_map(|(position, (value, _))| {
        repeats(&items[..position], value)
            .then(|| format!("repeats {value}; a set's values must be unique"))
    })
}

fn is_numeric(family: TypeFamily) -> bool {
    matches!(family, TypeFamily::Integer | TypeFamily::Decimal)
}

fn repeats(earlier: &[(String, TypeFamily)], value: &str) -> bool {
    earlier.iter().any(|(seen, _)| seen == value)
}
