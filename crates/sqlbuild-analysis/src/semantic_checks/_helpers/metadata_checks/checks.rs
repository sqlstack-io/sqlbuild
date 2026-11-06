//! Python's `get_semantic_metadata_diagnostics`, apart from the resource SQL checks.

use sqlbuild_sqltext::resolution::main::column_name_key::column_name_key;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::semantic_checks::_helpers::explanation::messages::{compiled, pattern};
use crate::semantic_checks::_helpers::metadata_checks::audits::{AuditContext, audit_errors};
use crate::semantic_checks::_helpers::metadata_checks::families::Families;
use crate::semantic_checks::_helpers::metadata_checks::function_calls::{
    FunctionContext, function_errors,
};
use crate::semantic_checks::_helpers::metadata_checks::positions::{
    find_code_point, text_position,
};
use crate::semantic_checks::_helpers::sql_text::text::casefold;
use crate::semantic_checks::constants::{
    CURSOR_INPUTS_KEY, SQL_TEST_COLUMN_CODE, SQL_TEST_CTE_PATTERN, TYPE_MISMATCH_CODE,
    UNKNOWN_COLUMN_REFERENCE_CODE, UNKNOWN_TYPE,
};
use crate::semantic_checks::models::{
    MetadataFinding, MetadataFunction, MetadataModel, MetadataOutcome, MetadataRequest,
    MetadataSource, MetadataSqlTest, ModelMetadataFindings, SemanticFailure,
};
use crate::semantic_validation::models::ProjectCatalog;
use crate::type_system::models::TypeFamily;

static SQL_TEST_CTE: LazyLock<Result<Regex, String>> =
    LazyLock::new(|| compiled(SQL_TEST_CTE_PATTERN));

const INTEGER_CURSOR_FAMILIES: &[TypeFamily] = &[TypeFamily::Integer, TypeFamily::Decimal];
/// The dialect that stores timestamps as ISO-8601 text, so a text column is a timestamp cursor.
const TEXT_TIMESTAMP_DIALECT: &str = "sqlite";
const TEMPORAL_CURSOR_FAMILIES: &[TypeFamily] = &[
    TypeFamily::Timestamp,
    TypeFamily::Datetime,
    TypeFamily::Date,
];

type Shapes<'a> = HashMap<&'a str, &'a [(String, String)]>;
/// One config reference: `(key, column name, the shape it must name a column of)`.
type Reference<'a> = (String, &'a str, &'a [(String, String)]);

/// The metadata errors of every model, source and SQL test, or the error Python raised.
pub(crate) fn check_metadata(
    request: &MetadataRequest,
    catalog: &ProjectCatalog,
) -> Result<MetadataOutcome, SemanticFailure> {
    let shapes: Shapes<'_> = request
        .shapes
        .iter()
        .map(|(name, columns)| (name.as_str(), columns.as_slice()))
        .collect();
    let functions: HashMap<&str, &MetadataFunction> = request
        .functions
        .iter()
        .map(|function| (function.key.as_str(), function))
        .collect();
    let return_types: HashMap<String, String> = request.return_types.iter().cloned().collect();
    let families = Families::new(request.dialect.as_deref());
    let context = FunctionContext {
        dialect: request.dialect.as_deref(),
        functions: &functions,
        shapes: &shapes,
        return_types: &return_types,
        families: &families,
    };
    let audits = AuditContext {
        dialect: request.dialect.as_deref(),
        quoted_ignore_case: request.quoted_ignore_case,
        shapes: &shapes,
        catalog,
    };
    let mut models: Vec<ModelMetadataFindings> = Vec::with_capacity(request.models.len());
    for model in &request.models {
        models.push(model_errors(model, &context, &audits)?);
    }
    let sources = source_errors(&request.sources, &shapes)?;
    let sql_tests = sql_test_errors(&request.sql_tests, &shapes)?;
    Ok(MetadataOutcome {
        models,
        sources,
        sql_tests,
        fallback_types: families.fallback_types(),
    })
}

fn model_errors(
    model: &MetadataModel,
    context: &FunctionContext<'_>,
    audits: &AuditContext<'_>,
) -> Result<ModelMetadataFindings, SemanticFailure> {
    if !model.checked {
        return Ok(ModelMetadataFindings::default());
    }
    let mut functions: Vec<MetadataFinding> = Vec::new();
    for (code, name, message) in function_errors(model, context)? {
        functions.push(model_error(model, code, &name, message)?);
    }
    let (audits, references) = match context.shapes.get(model.name.as_str()) {
        Some(shape) => (
            audit_errors(model, shape, audits)?,
            reference_errors(model, shape, context)?,
        ),
        None => (Vec::new(), Vec::new()),
    };
    Ok(ModelMetadataFindings {
        functions,
        audits,
        references,
    })
}

/// Python's `_model_error`, located at the name's first whole-word use in authored SQL.
fn model_error(
    model: &MetadataModel,
    code: &'static str,
    name: &str,
    message: String,
) -> Result<MetadataFinding, SemanticFailure> {
    let (line, column) = text_position(&model.authored_sql, name, 0)?;
    Ok(MetadataFinding {
        code,
        message,
        line,
        column,
    })
}

/// Each shape column's case-folded name, without the quotes of a quoted identifier.
fn folded_names(shape: &[(String, String)]) -> Result<HashSet<String>, SemanticFailure> {
    let mut names: HashSet<String> = HashSet::with_capacity(shape.len());
    for (name, _) in shape {
        names.insert(column_name_key(name));
    }
    Ok(names)
}

/// The B300 config references and the B301 cursor type of one model with a closed shape.
fn reference_errors(
    model: &MetadataModel,
    shape: &[(String, String)],
    context: &FunctionContext<'_>,
) -> Result<Vec<MetadataFinding>, SemanticFailure> {
    let mut references: Vec<Reference<'_>> = Vec::new();
    for (key, names) in &model.references {
        for name in names {
            references.push((key.clone(), name, shape));
        }
    }
    for (upstream, names) in &model.cursor_inputs {
        if let Some(upstream_shape) = context.shapes.get(upstream.as_str()) {
            for name in names {
                references.push((
                    format!("{CURSOR_INPUTS_KEY} {upstream}"),
                    name,
                    upstream_shape,
                ));
            }
        }
    }
    let mut errors: Vec<MetadataFinding> = Vec::new();
    for (key, name, reference_shape) in references {
        if !folded_names(reference_shape)?.contains(&column_name_key(name)) {
            let message = format!("{key} references unknown column '{name}'");
            errors.push(model_error(
                model,
                UNKNOWN_COLUMN_REFERENCE_CODE,
                name,
                message,
            )?);
        }
    }
    if let Some(error) = cursor_error(model, shape, (context.families, context.dialect))? {
        errors.push(error);
    }
    Ok(errors)
}

/// Python's cursor-type check: the cursor column's family must suit `cursor_type`.
fn cursor_error(
    model: &MetadataModel,
    shape: &[(String, String)],
    (families, dialect): (&Families, Option<&str>),
) -> Result<Option<MetadataFinding>, SemanticFailure> {
    let (Some(cursor), Some(cursor_type)) = (model.cursor.as_deref(), model.cursor_type.as_deref())
    else {
        return Ok(None);
    };
    let folded_cursor: String = casefold(cursor);
    let mut actual: &str = UNKNOWN_TYPE;
    for (name, column_type) in shape {
        if casefold(name) == folded_cursor {
            actual = column_type;
            break;
        }
    }
    let allowed: &[TypeFamily] = match cursor_type.to_ascii_lowercase().as_str() {
        "integer" => INTEGER_CURSOR_FAMILIES,
        "timestamp" | "date" => TEMPORAL_CURSOR_FAMILIES,
        _ => return Ok(None),
    };
    let family: TypeFamily = families.family(actual)?;
    if family == TypeFamily::Other || allowed.contains(&family) {
        return Ok(None);
    }
    if family == TypeFamily::String
        && allowed == TEMPORAL_CURSOR_FAMILIES
        && dialect == Some(TEXT_TIMESTAMP_DIALECT)
    {
        return Ok(None);
    }
    let message =
        format!("cursor_type {cursor_type} does not match column '{cursor}' type {actual}");
    Ok(Some(model_error(
        model,
        TYPE_MISMATCH_CODE,
        cursor,
        message,
    )?))
}

/// Python's source `cursor_column` check against each closed source shape.
fn source_errors(
    sources: &[MetadataSource],
    shapes: &Shapes<'_>,
) -> Result<Vec<(usize, MetadataFinding)>, SemanticFailure> {
    let mut errors: Vec<(usize, MetadataFinding)> = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let (Some(cursor), Some(shape)) = (&source.cursor_column, shapes.get(source.name.as_str()))
        else {
            continue;
        };
        if folded_names(shape)?.contains(&column_name_key(cursor)) {
            continue;
        }
        let (line, column) = text_position(&source.contents, cursor, 0)?;
        errors.push((
            index,
            MetadataFinding {
                code: UNKNOWN_COLUMN_REFERENCE_CODE,
                message: format!("cursor_column references unknown column '{cursor}'"),
                line,
                column,
            },
        ));
    }
    Ok(errors)
}

/// Python's `_sql_test_errors`: B302 for fixture or expected columns a tested shape lacks.
fn sql_test_errors(
    tests: &[MetadataSqlTest],
    shapes: &Shapes<'_>,
) -> Result<Vec<(usize, MetadataFinding, i64)>, SemanticFailure> {
    let cte_pattern: &Regex = pattern(&SQL_TEST_CTE)?;
    let mut errors: Vec<(usize, MetadataFinding, i64)> = Vec::new();
    for (index, test) in tests.iter().enumerate() {
        for (cte_name, columns) in &test.ctes {
            let matched_name: &str = cte_name.strip_suffix('\n').unwrap_or(cte_name);
            let Some(shape) = cte_pattern
                .captures(matched_name)
                .and_then(|captures| shapes.get(&captures[1]))
            else {
                continue;
            };
            let available: HashSet<String> = folded_names(shape)?;
            for column_name in columns {
                if available.contains(&column_name_key(column_name)) {
                    continue;
                }
                let offset: usize = find_code_point(&test.contents, cte_name).unwrap_or(0);
                let (line, column) = text_position(&test.contents, column_name, offset)?;
                let width: i64 = i64::try_from(column_name.chars().count()).unwrap_or(i64::MAX);
                let message = format!("SQL test '{cte_name}' names unknown column '{column_name}'");
                errors.push((
                    index,
                    MetadataFinding {
                        code: SQL_TEST_COLUMN_CODE,
                        message,
                        line,
                        column,
                    },
                    column + width,
                ));
            }
        }
    }
    Ok(errors)
}
