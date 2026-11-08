//! An expression macro's body typed from its declared arguments and checked against `returns`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::analysis_route::types::NativeDialect;
use crate::sql_macros::_helpers::parsing::body::expand_body;
use crate::sql_macros::_helpers::rendering::arguments::type_family;
use crate::sql_macros::_helpers::typing::probes::{ProbeSchema, probe_outputs, probe_type_errors};
use crate::sql_macros::_helpers::typing::typed_calls::{
    markable_calls, shifted, typed_sql, without_marker_calls,
};
use crate::sql_macros::constants::{
    BODY_TYPE_CODE, INFERRED_TEXT_TYPE, RETURNS_MISMATCH_CODE, STAND_IN_RELATION, TYPE_CODE_PREFIX,
    TYPE_CODES, UNKNOWN_TYPE_HINT, VARCHAR_TYPE,
};
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::{
    ArgumentShape, BodyType, ExpandedText, ParameterKind, RenderedArgument, SqlMacroDefinition,
    SqlMacroForm, SqlMacroLocation, TypeFamily, TypedCall,
};
use crate::sql_macros::types::{BodyProbe, RelationShapes};
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::TypeNormalization;

/// Each body's type, by the probe that typed it, so a macro is typed once per process.
static BODY_TYPES: OnceLock<Mutex<HashMap<String, BodyType>>> = OnceLock::new();
/// Each body's first type error, by the probe that found it.
static TYPE_ERRORS: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

/// The dialect an adapter's SQL is analysed in.
pub(crate) fn adapter_dialect(adapter: &str) -> &str {
    match adapter {
        "motherduck" => "duckdb",
        "" => "generic",
        other => other,
    }
}

/// A parameter's stand-in column type, with `ANY` families widened only when `families` is set.
fn stand_in_type(kind: &ParameterKind, families: bool) -> Option<&str> {
    match kind {
        ParameterKind::Expression { data_type } | ParameterKind::Constant { data_type } => {
            Some(data_type)
        }
        ParameterKind::Collection { data_type, .. } => Some(data_type),
        ParameterKind::Any {
            family: Some(family),
        } if families => Some(family.widest()),
        ParameterKind::Any { .. } | ParameterKind::Enum { .. } | ParameterKind::Relation { .. } => {
            None
        }
    }
}

/// The name of parameter `index`'s stand-in column.
fn stand_in_column(index: usize) -> String {
    format!("p{index}")
}

/// The body selected over stand-in arguments, and their relation, unless it reads relations.
fn body_probe(
    (parser, definition): (&dyn NativeDialect, &SqlMacroDefinition),
    body: &ExpandedText,
    families: bool,
) -> Option<BodyProbe> {
    let mut arguments: Vec<RenderedArgument> = Vec::with_capacity(definition.parameters.len());
    let mut columns: Vec<(String, String)> = Vec::with_capacity(definition.parameters.len());
    for (index, parameter) in definition.parameters.iter().enumerate() {
        if matches!(parameter.kind, ParameterKind::Relation { .. }) {
            return None;
        }
        let column: String = stand_in_column(index);
        let sql: String = match parameter.kind {
            ParameterKind::Collection { .. } => format!("({STAND_IN_RELATION}.{column})"),
            _ => format!("{STAND_IN_RELATION}.{column}"),
        };
        columns.push((
            column.clone(),
            stand_in_type(&parameter.kind, families)
                .unwrap_or(UNKNOWN_TYPE_HINT)
                .to_owned(),
        ));
        arguments.push(RenderedArgument {
            parameter: index,
            sql,
            shape: ArgumentShape::Expression,
            calls: Vec::new(),
            slots: Vec::new(),
        });
    }
    let expansion = match expand_body(parser, (definition, None), body, &arguments) {
        Ok(expansion) => expansion,
        Err(_) => return None,
    };
    let prefix: &str = "SELECT (";
    let unmarked: String = format!("{prefix}{}) AS b FROM {STAND_IN_RELATION}", expansion.sql);
    let calls: Vec<TypedCall> = shifted(expansion.calls, prefix.len()).collect();
    let (probe, _) = typed_sql(&unmarked, &markable_calls(&unmarked, &calls, parser.name()));
    Some((probe, vec![(STAND_IN_RELATION.to_owned(), columns)]))
}

/// What inference knows of the body's type.
fn inferred_body_type(parser: &dyn NativeDialect, probe: &str, table: &RelationShapes) -> BodyType {
    let Some(outputs) = probe_outputs(parser, probe, &ProbeSchema::new(table)) else {
        return BodyType::Unknown;
    };
    match outputs.into_iter().next() {
        Some((_, Some(shown))) if !shown.to_ascii_uppercase().contains(UNKNOWN_TYPE_HINT) => {
            BodyType::Known(shown)
        }
        _ => BodyType::Unknown,
    }
}

/// Whether body type `inferred` is outside the family of `returns`, or another non-family type.
fn disagrees(returns: &str, inferred: &str, adapter: &str) -> bool {
    let (declared, found) = (
        type_family(returns, adapter),
        type_family(inferred, adapter),
    );
    if declared != TypeFamily::Unknown && found != TypeFamily::Unknown {
        return declared != found;
    }
    let dialect: &str = adapter_dialect(adapter);
    match (
        normalize_type(returns, dialect),
        normalize_type(inferred, dialect),
    ) {
        (
            Ok(TypeNormalization {
                normalized: declared,
                ..
            }),
            Ok(TypeNormalization {
                normalized: found, ..
            }),
        ) => declared != found,
        _ => false,
    }
}

/// The verdict `cache` holds under `key`, or `compute`'s verdict, cached for the process.
fn remembered<T: Clone>(
    cache: &OnceLock<Mutex<HashMap<String, T>>>,
    key: String,
    compute: impl FnOnce() -> T,
) -> T {
    let verdicts = cache.get_or_init(|| Mutex::new(HashMap::new()));
    let mut verdicts = match verdicts.lock() {
        Ok(verdicts) => verdicts,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(known) = verdicts.get(&key) {
        return known.clone();
    }
    let verdict: T = compute();
    verdicts.insert(key, verdict.clone());
    verdict
}

/// Rejects an expression macro whose body's known type is not its declared `returns`.
pub(crate) fn check_body_returns(
    (parser, definition): (&dyn NativeDialect, &SqlMacroDefinition),
    body: &ExpandedText,
    (adapter, location): (&str, SqlMacroLocation),
) -> Result<(), SqlMacroError> {
    if definition.form != SqlMacroForm::Expression {
        return Ok(());
    }
    let Some(returns) = definition.returns.as_deref() else {
        return Ok(());
    };
    let Some((probe, table)) = body_probe((parser, definition), body, false) else {
        return Ok(());
    };
    let key: String = format!("returns\n{}\n{table:?}\n{probe}", parser.name());
    let body_type: BodyType = remembered(&BODY_TYPES, key, || {
        inferred_body_type(parser, &probe, &table)
    });
    let (shown, wrong): (String, bool) = match body_type {
        BodyType::Unknown => return Ok(()),
        BodyType::Known(inferred) if inferred.eq_ignore_ascii_case(INFERRED_TEXT_TYPE) => (
            VARCHAR_TYPE.to_owned(),
            disagrees(returns, VARCHAR_TYPE, adapter),
        ),
        BodyType::Known(inferred) => {
            let wrong: bool = disagrees(returns, &inferred, adapter);
            (inferred, wrong)
        }
    };
    if !wrong {
        return Ok(());
    }
    Err(SqlMacroError::new(
        RETURNS_MISMATCH_CODE,
        format!(
            "macro {}() declares returns {returns}, but its body is {shown}",
            definition.name
        ),
    )
    .at(location)
    .helped(format!(
        "declare returns {shown}, CAST the body to {returns}, or remove returns to use the inferred type"
    )))
}

/// The first type error the body's probe shows.
fn body_type_error(
    parser: &dyn NativeDialect,
    probe: &str,
    table: &RelationShapes,
) -> Option<String> {
    probe_type_errors(parser, probe, &ProbeSchema::new(table))
        .into_iter()
        .find(|(code, _)| is_type_code(code))
        .map(|(_, message)| without_marker_calls(&message).into_owned())
}

/// Whether an analysis code reports an operand or argument of the wrong type.
fn is_type_code(code: &str) -> bool {
    let number: usize = match code.get(1..).map(str::parse::<usize>) {
        Some(Ok(number)) => number,
        _ => return false,
    };
    code.starts_with(TYPE_CODE_PREFIX) && (TYPE_CODES.0..=TYPE_CODES.1).contains(&number)
}

/// Rejects a body that misuses an `ANY` family argument, typed as the family's widest member.
pub(crate) fn check_body_types(
    (parser, definition): (&dyn NativeDialect, &SqlMacroDefinition),
    body: &ExpandedText,
    location: SqlMacroLocation,
) -> Result<(), SqlMacroError> {
    let families: Vec<String> = definition
        .parameters
        .iter()
        .filter_map(|parameter| match parameter.kind {
            ParameterKind::Any {
                family: Some(family),
            } => Some(format!("{} ANY {}", parameter.name, family.keyword())),
            _ => None,
        })
        .collect();
    if families.is_empty() {
        return Ok(());
    }
    let Some((probe, table)) = body_probe((parser, definition), body, true) else {
        return Ok(());
    };
    let key: String = format!("types\n{}\n{table:?}\n{probe}", parser.name());
    let found: Option<String> = remembered(&TYPE_ERRORS, key, || {
        body_type_error(parser, &probe, &table)
    });
    let Some(found) = found else {
        return Ok(());
    };
    Err(SqlMacroError::new(
        BODY_TYPE_CODE,
        format!(
            "body of macro {}() does not accept every value of {}: {found}",
            definition.name,
            families.join(", ")
        ),
    )
    .at(location)
    .helped("an ANY family argument may be any type of its family; the body is checked with the family's widest type, so CAST the argument or narrow its declared type"))
}
