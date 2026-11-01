//! Python's `_audit_errors`: schema audits checked against the model's closed output shape.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::semantic_checks::_helpers::metadata_checks::positions::text_position;
use crate::semantic_checks::_helpers::resource_sql::columns::{
    binding_dialect, resolve_binding_column,
};
use crate::semantic_checks::models::{
    AuditFinding, AuditValue, MetadataAudit, MetadataModel, SemanticFailure,
};
use crate::semantic_validation::main::diagnostics::binding_diagnostics;
use crate::semantic_validation::main::validations_json;
use crate::semantic_validation::models::{CatalogInput, Columns, DiagnosticRow, ProjectCatalog};
use crate::semantic_validation::types::Relations;

const EXPRESSION_AUDIT: &str = "expression_is_true";
const RELATIONSHIPS_AUDIT: &str = "relationships";
const ACCEPTED_VALUES_AUDIT: &str = "accepted_values";
const OUTPUT_RELATION: &str = "output";
const UNKNOWN_TABLE_CODE: &str = "B000";
const MISSING_TARGET_COLUMN_CODE: &str = "B300";
const AUDIT_TYPE_CODE: &str = "B301";
const ERROR_SEVERITY: &str = "error";
const BINDING_SEVERITIES: [&str; 2] = ["error", "warning"];
const DEFAULT_BINDING_MESSAGE: &str = "SQL binding failed";
const GENERIC_DIALECT: &str = "generic";
const REFERENCE_KINDS: [&str; 3] = ["ref", "source", "seed"];
const QUOTES: [char; 2] = ['\'', '"'];

/// What the audits of one model are checked against.
pub(crate) struct AuditContext<'a> {
    pub(crate) dialect: Option<&'a str>,
    pub(crate) quoted_ignore_case: bool,
    pub(crate) shapes: &'a std::collections::HashMap<&'a str, &'a [(String, String)]>,
    pub(crate) catalog: &'a ProjectCatalog,
}

fn internal(error: String) -> SemanticFailure {
    SemanticFailure::Internal(error)
}

fn columns(shape: &[(String, String)]) -> Columns {
    Columns(
        shape
            .iter()
            .map(|(column, data_type)| (column.clone(), Some(data_type.clone())))
            .collect(),
    )
}

/// The referenced name of a `__ref("name")`-style call starting at `start`, if one does.
fn reference_call_at(text: &[char], start: usize) -> Option<String> {
    let rest: String = text[start..].iter().collect();
    let after = rest.strip_prefix("__")?;
    let mut kinds = REFERENCE_KINDS.iter();
    let after = loop {
        let kind = kinds.next()?;
        if let Some(after) = after.strip_prefix(kind) {
            break after;
        }
    };
    let after = after.strip_prefix('(')?;
    let mut characters = after.chars();
    if !QUOTES.contains(&characters.next()?) {
        return None;
    }
    let body: &str = characters.as_str();
    let name_length = body.find(QUOTES).unwrap_or(body.len());
    let close = &body[name_length..];
    (name_length > 0 && close.len() > 1 && close[1..].starts_with(')'))
        .then(|| body[..name_length].to_owned())
}

/// `__(?:ref|source|seed)\(['"]([^'"]+)['"]\)` searched in `target`: the referenced name.
fn referenced_name(target: &str) -> Option<String> {
    let text: Vec<char> = target.chars().collect();
    (0..text.len()).find_map(|start| reference_call_at(&text, start))
}

fn optional_index(value: Option<&Value>) -> Option<usize> {
    value.and_then(Value::as_u64).map(|number| number as usize)
}

/// Python's `_binding_result` over one native schema validation response.
fn response_rows(response: &Value) -> Result<Vec<DiagnosticRow>, SemanticFailure> {
    let errors = response
        .get("errors")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            internal("native SQL schema validation returned an invalid response".to_owned())
        })?;
    let mut rows: Vec<DiagnosticRow> = Vec::new();
    for error in errors {
        let (Some(severity), Some(code)) = (
            error.get("severity").and_then(Value::as_str),
            error.get("code").and_then(Value::as_str),
        ) else {
            continue;
        };
        if !BINDING_SEVERITIES.contains(&severity) {
            continue;
        }
        let message = match error.get("message") {
            Some(Value::String(text)) if !text.is_empty() => text.clone(),
            _ => DEFAULT_BINDING_MESSAGE.to_owned(),
        };
        rows.push(DiagnosticRow {
            code: code.to_owned(),
            message,
            line: optional_index(error.get("line")),
            column: optional_index(error.get("column")),
            start: optional_index(error.get("start")),
            end: optional_index(error.get("end")),
            severity: severity.to_owned(),
        });
    }
    Ok(rows)
}

/// Python's `get_schema_validations` for one request without a catalog.
fn standalone_validation(
    sql: &str,
    relations: Vec<(&str, Columns)>,
    dialect: Option<&str>,
) -> Result<Vec<DiagnosticRow>, SemanticFailure> {
    let dialect = binding_dialect(Some(dialect.unwrap_or(GENERIC_DIALECT)))
        .unwrap_or(GENERIC_DIALECT)
        .to_owned();
    let references: BTreeMap<&str, bool> = relations
        .iter()
        .map(|(name, columns)| (*name, !columns.0.is_empty()))
        .collect();
    let catalog = ProjectCatalog::new(CatalogInput {
        dialect: dialect.clone(),
        quoted_ignore_case: false,
        known_functions: Vec::new(),
        known_types: Vec::new(),
        relations: relations
            .into_iter()
            .map(|(name, columns)| (name.to_owned(), columns))
            .collect(),
    })
    .map_err(internal)?;
    let references: Vec<(String, bool)> = references
        .into_iter()
        .map(|(name, closed)| (name.to_owned(), closed))
        .collect();
    let payload = catalog
        .validation_payload(vec![(sql.to_owned(), references, Relations::new())])
        .map_err(internal)?;
    let responses: Value = serde_json::from_str(&validations_json(&payload).map_err(internal)?)
        .map_err(|error| internal(error.to_string()))?;
    let response = responses
        .as_array()
        .and_then(|responses| responses.first())
        .ok_or_else(|| {
            internal("native SQL schema validation returned an invalid batch response".to_owned())
        })?;
    binding_diagnostics(sql, &dialect, response_rows(response)?).map_err(internal)
}

/// One audit finding before it is located at the named column or field.
struct Unlocated {
    code: String,
    message: String,
    severity: String,
    name: String,
}

fn expression_findings(
    expression: &str,
    shape: &[(String, String)],
    context: &AuditContext<'_>,
) -> Result<Vec<Unlocated>, SemanticFailure> {
    let mut overrides: Relations = Relations::new();
    if !shape.is_empty() {
        overrides.insert(OUTPUT_RELATION.to_owned(), columns(shape));
    }
    let request = (
        format!("SELECT * FROM {OUTPUT_RELATION} WHERE {expression}"),
        vec![(OUTPUT_RELATION.to_owned(), !shape.is_empty())],
        overrides,
    );
    let rows = context
        .catalog
        .binding_results(
            vec![request],
            crate::semantic_validation::models::UnknownFunctions::Errors,
        )
        .map_err(internal)?
        .into_iter()
        .next()
        .unwrap_or_default();
    Ok(rows
        .into_iter()
        .filter(|row| row.code != UNKNOWN_TABLE_CODE)
        .map(|row| Unlocated {
            code: row.code,
            message: format!("{EXPRESSION_AUDIT}: {}", row.message),
            severity: row.severity,
            name: expression.to_owned(),
        })
        .collect())
}

fn typed_findings(rows: Vec<DiagnosticRow>, label: &str, column: &str) -> Vec<Unlocated> {
    rows.into_iter()
        .map(|row| {
            let code = if row.severity == ERROR_SEVERITY {
                AUDIT_TYPE_CODE.to_owned()
            } else {
                row.code
            };
            Unlocated {
                code,
                message: format!("{label}: {}", row.message),
                severity: row.severity,
                name: column.to_owned(),
            }
        })
        .collect()
}

fn single_column(column_type: &str) -> Columns {
    Columns(vec![("value".to_owned(), Some(column_type.to_owned()))])
}

fn relationship_findings(
    audit: &MetadataAudit,
    column: &str,
    shape: &[(String, String)],
    context: &AuditContext<'_>,
) -> Result<Vec<Unlocated>, SemanticFailure> {
    let key = referenced_name(&audit.target).unwrap_or_else(|| audit.target.clone());
    let target_shape = context.shapes.get(key.as_str()).copied();
    let resolved = resolve_binding_column(
        &audit.field,
        target_shape.unwrap_or(&[]),
        context.dialect,
        context.quoted_ignore_case,
    );
    let Some(target_shape) = target_shape else {
        return Ok(Vec::new());
    };
    let Some(resolved) = resolved else {
        if target_shape.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![Unlocated {
            code: MISSING_TARGET_COLUMN_CODE.to_owned(),
            message: format!("relationships target has no column '{}'", audit.field),
            severity: ERROR_SEVERITY.to_owned(),
            name: audit.field.clone(),
        }]);
    };
    let Some((_, left)) = shape.iter().find(|(name, _)| name == column) else {
        return Ok(Vec::new());
    };
    let right = target_shape
        .iter()
        .find(|(name, _)| name == resolved)
        .map_or("", |(_, data_type)| data_type.as_str());
    let rows = standalone_validation(
        "SELECT lhs.value = rhs.value FROM lhs CROSS JOIN rhs",
        vec![("lhs", single_column(left)), ("rhs", single_column(right))],
        context.dialect,
    )?;
    Ok(typed_findings(rows, RELATIONSHIPS_AUDIT, column))
}

/// Python's `_audit_literal`.
fn audit_literal(value: &AuditValue) -> Option<String> {
    match value {
        AuditValue::Null => Some("NULL".to_owned()),
        AuditValue::Bool(true) => Some("TRUE".to_owned()),
        AuditValue::Bool(false) => Some("FALSE".to_owned()),
        AuditValue::Number(text) => Some(text.clone()),
        AuditValue::Text(text) => Some(format!("'{}'", text.replace('\'', "''"))),
        AuditValue::Other => None,
    }
}

fn accepted_value_findings(
    values: &[AuditValue],
    column: &str,
    column_type: &str,
    context: &AuditContext<'_>,
) -> Result<Vec<Unlocated>, SemanticFailure> {
    let literals: Option<Vec<String>> = values.iter().map(audit_literal).collect();
    let Some(literals) = literals.filter(|literals| !literals.is_empty()) else {
        return Ok(Vec::new());
    };
    let rows = standalone_validation(
        &format!(
            "SELECT value NOT IN ({}) FROM {OUTPUT_RELATION}",
            literals.join(", ")
        ),
        vec![(OUTPUT_RELATION, single_column(column_type))],
        context.dialect,
    )?;
    Ok(typed_findings(rows, ACCEPTED_VALUES_AUDIT, column))
}

fn audit_findings(
    audit: &MetadataAudit,
    shape: &[(String, String)],
    context: &AuditContext<'_>,
) -> Result<Vec<Unlocated>, SemanticFailure> {
    let column = audit.column.as_deref();
    let column_type = column.and_then(|column| {
        shape
            .iter()
            .find(|(name, _)| name == column)
            .map(|(_, data_type)| data_type.as_str())
    });
    match audit.definition.as_str() {
        EXPRESSION_AUDIT => match &audit.expression {
            Some(expression) => expression_findings(expression, shape, context),
            None => Ok(Vec::new()),
        },
        RELATIONSHIPS_AUDIT => match column.filter(|column| !column.is_empty()) {
            Some(column) => relationship_findings(audit, column, shape, context),
            None => Ok(Vec::new()),
        },
        ACCEPTED_VALUES_AUDIT => match (column, column_type, &audit.values) {
            (Some(column), Some(column_type), Some(values)) => {
                accepted_value_findings(values, column, column_type, context)
            }
            _ => Ok(Vec::new()),
        },
        _ => Ok(Vec::new()),
    }
}

/// Every schema audit finding of a checked model with a closed shape, in Python's order.
pub(crate) fn audit_errors(
    model: &MetadataModel,
    shape: &[(String, String)],
    context: &AuditContext<'_>,
) -> Result<Vec<AuditFinding>, SemanticFailure> {
    let mut findings: Vec<AuditFinding> = Vec::new();
    for audit in &model.audits {
        for Unlocated {
            code,
            message,
            severity,
            name,
        } in audit_findings(audit, shape, context)?
        {
            let (line, column) = text_position(&model.authored_sql, &name, 0)?;
            findings.push(AuditFinding {
                code,
                message,
                line,
                column,
                is_error: severity == ERROR_SEVERITY,
            });
        }
    }
    Ok(findings)
}
