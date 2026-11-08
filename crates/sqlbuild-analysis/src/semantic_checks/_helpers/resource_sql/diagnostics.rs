//! Python's `get_resource_sql_diagnostics`: bind resource SQL against the closed model shapes.

use std::collections::{BTreeSet, HashMap, HashSet};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics::replace_intrinsics;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::semantic_checks::_helpers::explanation::opt_outs::unneeded_opt_out;
use crate::semantic_checks::_helpers::metadata_checks::positions::{text_position, word_start};
use crate::semantic_checks::_helpers::resource_sql::help::{HelpContext, resource_sql_help};
use crate::semantic_checks::_helpers::resource_sql::locations::{
    Finding, line_at, location, text_from,
};
use crate::semantic_checks::_helpers::resource_sql::resources::{
    MissingColumnAudit, ResourceSql, audit_label, missing_column_audits, resources,
};
use crate::semantic_checks::models::{
    ResourceSpan, ResourceSqlDiagnostic, ResourceSqlFailure, ResourceSqlKind, ResourceSqlRequest,
    SemanticFailure,
};
use crate::semantic_validation::main::normalize::normalize_analysis_sql;
use crate::semantic_validation::models::{Columns, NormalizationInput, ProjectCatalog};
use crate::semantic_validation::types::{BindingRequest, Relations};

const UNKNOWN_TABLE_CODE: &str = "B000";
const UNKNOWN_COLUMN_CODE: &str = "B002";
const UNKNOWN_TABLE_PREFIX: &str = "Unknown table ";
const SYNTAX_ERROR_CODE: &str = "P001";
const OPT_OUT_CODE: &str = "P009";
const ERROR_SEVERITY: &str = "error";
const CURSOR_CONTEXT: &str = "Model SQL";
const ANALYSIS_CURSOR_LITERAL: &str = "CAST('2000-01-01 00:00:00' AS TIMESTAMP)";
const GENERIC_DIALECT: &str = "generic";

/// A finding's identity: its path, code, message, start and native order when unlocated.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    path: String,
    code: String,
    message: String,
    line: i64,
    column: i64,
    /// The finding's native index when it has no start, else `-1`.
    order: i64,
}

fn kind_label(kind: ResourceSqlKind) -> &'static str {
    match kind {
        ResourceSqlKind::Audit => "audit",
        ResourceSqlKind::SqlTest => "SQL test",
        ResourceSqlKind::Model => "model",
    }
}

fn semantic(failure: SemanticFailure) -> ResourceSqlFailure {
    ResourceSqlFailure::Internal(format!("{failure:?}"))
}

/// Python's `normalize_analysis_sql(cursor_intrinsics_analysis_sql(sql))`.
fn cleaned_sql(sql: &str, dialect: Option<&str>) -> Result<String, ResourceSqlFailure> {
    let (analysis_sql, _) =
        replace_intrinsics(sql, CURSOR_CONTEXT, |_| ANALYSIS_CURSOR_LITERAL.to_owned())
            .map_err(ResourceSqlFailure::CursorIntrinsic)?;
    normalize_analysis_sql(NormalizationInput {
        sql: analysis_sql,
        dialect: dialect.unwrap_or(GENERIC_DIALECT).to_owned(),
        stubs: HashMap::new(),
        placeholders: HashMap::new(),
    })
    .map_err(ResourceSqlFailure::Normalization)
}

/// The binding request for `sql` over `relations`, with each closed shape as an override.
fn binding_request(
    sql: &str,
    relations: &BTreeSet<String>,
    shapes: &HashMap<&str, &[(String, String)]>,
) -> BindingRequest {
    let mut references: Vec<(String, bool)> = Vec::with_capacity(relations.len());
    let mut overrides: Relations = Relations::new();
    for name in relations {
        let columns = shapes.get(name.as_str()).copied().unwrap_or(&[]);
        references.push((name.clone(), !columns.is_empty()));
        if !columns.is_empty() {
            overrides.insert(
                name.clone(),
                Columns(
                    columns
                        .iter()
                        .map(|(column, data_type)| (column.clone(), Some(data_type.clone())))
                        .collect(),
                ),
            );
        }
    }
    (sql.to_owned(), references, overrides)
}

/// `_reported_diagnostic`: drop open-relation lookups; parser rejections are P001, natively P026's.
fn reported(resource: &ResourceSql, finding: &Finding, native_hooks: bool) -> Option<Finding> {
    if finding.code != UNKNOWN_TABLE_CODE {
        return Some(finding.clone());
    }
    if finding.message.starts_with(UNKNOWN_TABLE_PREFIX)
        || (native_hooks && resource.kind == ResourceSqlKind::Model)
    {
        return None;
    }
    Some(Finding {
        code: SYNTAX_ERROR_CODE.to_owned(),
        message: format!(
            "SQL syntax error in {} '{}': {}",
            kind_label(resource.kind),
            resource.name,
            finding.message
        ),
        line: finding.line,
        column: finding.column,
        start: finding.start,
        end: finding.end,
        severity: ERROR_SEVERITY.to_owned(),
    })
}

/// What one resource's findings are checked against.
struct Checking<'a> {
    request: &'a ResourceSqlRequest,
    cleaned_sql: &'a str,
}

/// Python's `_resource_diagnostics`: located findings keyed for deduplication.
fn resource_diagnostics(
    resource: &ResourceSql,
    findings: &[Finding],
    checking: &Checking<'_>,
) -> Result<Vec<(Key, ResourceSqlDiagnostic)>, ResourceSqlFailure> {
    let mut found: Vec<(Key, ResourceSqlDiagnostic)> = Vec::new();
    let native_hooks: bool = matches!(
        analysis_route(checking.request.dialect.as_deref()),
        AnalysisRoute::Native(_)
    );
    for (index, native) in findings.iter().enumerate() {
        let Some(finding) = reported(resource, native, native_hooks) else {
            continue;
        };
        let span = location(resource, checking.cleaned_sql, &finding)?;
        let key: Key = Key {
            path: resource.path.clone(),
            code: finding.code.clone(),
            message: finding.message.clone(),
            line: span.line,
            column: span.column,
            order: if finding.start.is_none() {
                index as i64
            } else {
                -1
            },
        };
        if found.iter().any(|(existing, _)| *existing == key) {
            continue;
        }
        let authored_text = text_from(resource, span);
        let authored_line = line_at(resource, span);
        let help = resource_sql_help(
            &finding.code,
            &finding.message,
            &HelpContext {
                kind: resource.kind,
                authored_text: &authored_text,
                authored_line: &authored_line,
                column: span.column,
                authored_body: &resource.authored_body,
                audit_definition: resource.audit_definition.as_deref(),
                shapes: &checking.request.shapes,
                relations: &resource.relations,
                dialect: checking.request.dialect.as_deref(),
            },
        )
        .map_err(semantic)?;
        let message = match &resource.label {
            Some(label) if finding.code != SYNTAX_ERROR_CODE => {
                format!("{label}: {}", finding.message)
            }
            _ => finding.message.clone(),
        };
        found.push((
            key,
            ResourceSqlDiagnostic {
                kind: resource.kind,
                resource_name: resource.name.clone(),
                is_error: finding.severity == ERROR_SEVERITY,
                code: finding.code.clone(),
                message,
                path: resource.path.clone(),
                span,
                note: None,
                help,
            },
        ));
    }
    Ok(found)
}

fn opt_out_diagnostic(
    resource: &ResourceSql,
    (line, column): (i64, i64),
    hidden: &[(Key, ResourceSqlDiagnostic)],
) -> Result<ResourceSqlDiagnostic, ResourceSqlFailure> {
    let codes: Vec<&str> = hidden.iter().map(|(_, item)| item.code.as_str()).collect();
    let (message, note, help) =
        unneeded_opt_out(kind_label(resource.kind), &resource.name, false, &codes)
            .map_err(semantic)?;
    Ok(ResourceSqlDiagnostic {
        kind: resource.kind,
        resource_name: resource.name.clone(),
        is_error: true,
        code: OPT_OUT_CODE.to_owned(),
        message,
        path: resource.path.clone(),
        span: ResourceSpan::start(line, column),
        note: Some(note),
        help: Some(help),
    })
}

/// One B002 for a built-in audit on a missing model column, at its MODEL header declaration.
pub(crate) fn missing_column_diagnostic(
    missing: &MissingColumnAudit<'_>,
) -> Result<ResourceSqlDiagnostic, ResourceSqlFailure> {
    let MissingColumnAudit {
        audit,
        model,
        column,
    } = missing;
    let definition: &str = &audit.definition_name;
    let column_start: usize = word_start(&model.authored_sql, column, 0).map_err(semantic)?;
    let (line, start) =
        text_position(&model.authored_sql, definition, column_start).map_err(semantic)?;
    let end: i64 = start + i64::try_from(definition.chars().count()).unwrap_or(0);
    Ok(ResourceSqlDiagnostic {
        kind: ResourceSqlKind::Audit,
        resource_name: audit.name.clone(),
        is_error: true,
        code: UNKNOWN_COLUMN_CODE.to_owned(),
        message: format!(
            "{}: Unknown column '{column}' in table '{}'",
            audit_label(audit),
            model.name
        ),
        path: model.path.clone(),
        span: ResourceSpan {
            line,
            column: start,
            end_line: Some(line),
            end_column: Some(end),
        },
        note: None,
        help: Some(format!(
            "add {column} to the SELECT list of model '{}' or attach the {definition} audit to \
             a column the model outputs",
            model.name
        )),
    })
}

/// Every audit, SQL test and SQL hook finding, in Python's first-seen order.
pub(crate) fn resource_sql_findings(
    request: &ResourceSqlRequest,
    catalog: &ProjectCatalog,
) -> Result<Vec<ResourceSqlDiagnostic>, ResourceSqlFailure> {
    let mut diagnostics: Vec<ResourceSqlDiagnostic> = missing_column_audits(request)
        .iter()
        .map(missing_column_diagnostic)
        .collect::<Result<_, _>>()?;
    let pool = || catalog.analysis_pool();
    let checked = resources(request, &pool).map_err(ResourceSqlFailure::Internal)?;
    if checked.is_empty() {
        return Ok(diagnostics);
    }
    let dialect = request.dialect.as_deref();
    let mut seen: HashSet<&str> = HashSet::with_capacity(checked.len());
    let unique: Vec<&str> = checked
        .iter()
        .map(|resource| resource.sql.as_str())
        .filter(|sql| seen.insert(sql))
        .collect();
    let cleaned: Vec<Result<String, ResourceSqlFailure>> =
        pool().map_err(ResourceSqlFailure::Internal)?.install(|| {
            unique
                .par_iter()
                .map(|sql| {
                    catch_compiler_panic(|| Ok(cleaned_sql(sql, dialect)))
                        .unwrap_or_else(|panic| Err(ResourceSqlFailure::Internal(panic)))
                })
                .collect()
        });
    let mut normalized: HashMap<&str, String> = HashMap::with_capacity(unique.len());
    for (sql, cleaned) in unique.into_iter().zip(cleaned) {
        normalized.insert(sql, cleaned?);
    }
    let shapes: HashMap<&str, &[(String, String)]> = request
        .shapes
        .iter()
        .map(|(name, columns)| (name.as_str(), columns.as_slice()))
        .collect();
    let mut request_index: HashMap<(&str, &BTreeSet<String>), usize> = HashMap::new();
    let mut requests: Vec<BindingRequest> = Vec::new();
    for resource in &checked {
        let cleaned = normalized[resource.sql.as_str()].as_str();
        request_index
            .entry((cleaned, &resource.relations))
            .or_insert_with(|| {
                requests.push(binding_request(cleaned, &resource.relations, &shapes));
                requests.len() - 1
            });
    }
    let results = catalog
        .binding_results(requests)
        .map_err(ResourceSqlFailure::Internal)?;
    let mut keys: HashSet<Key> = HashSet::new();
    let mut opt_out_keys: HashSet<(String, i64, i64)> = HashSet::new();
    for resource in &checked {
        let cleaned = normalized[resource.sql.as_str()].as_str();
        let findings = &results[request_index[&(cleaned, &resource.relations)]];
        let checking = Checking {
            request,
            cleaned_sql: cleaned,
        };
        let found = resource_diagnostics(resource, findings, &checking)?;
        match resource.opt_out {
            None => {
                for (key, diagnostic) in found {
                    if keys.insert(key) {
                        diagnostics.push(diagnostic);
                    }
                }
            }
            Some(position) => {
                let parses = !found.iter().any(|(_, item)| item.code == SYNTAX_ERROR_CODE);
                if parses && opt_out_keys.insert((resource.path.clone(), position.0, position.1)) {
                    diagnostics.push(opt_out_diagnostic(resource, position, &found)?);
                }
            }
        }
    }
    Ok(diagnostics)
}
