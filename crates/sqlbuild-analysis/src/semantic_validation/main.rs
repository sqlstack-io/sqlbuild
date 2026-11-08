//! Stable SQLBuild boundary around Polyglot schema-aware validation.

pub mod diagnostics;
pub mod map_diagnostics;
pub mod new_analysis_pool;
pub mod normalize;
pub mod normalize_batch;
pub mod normalize_dialect;
pub mod normalize_mapped;
pub mod normalize_native;
pub mod sqlbuild_code;
pub mod text_findings;
pub mod truth_test_spans;
use crate::semantic_validation::models::ProjectCatalog;

use polyglot_sql::{
    Dialect, DialectType, Expression, ExpressionWalk, Resolver, SchemaValidationOptions,
    ValidationError, ValidationResult, ValidationSchema, build_scope,
    mapping_schema_from_validation_schema_with_dialect, validate_with_schema,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Deserialize;

const MINIMUM_USING_SOURCE_COUNT: usize = 2;
const MAX_VALIDATION_WORKERS: usize = 4;
const VALIDATION_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
pub(super) struct ValidationRequest {
    pub(super) sql: String,
    #[serde(default = "default_dialect")]
    pub(super) dialect: String,
    pub(super) schema: ValidationSchema,
    #[serde(default)]
    pub(super) options: SchemaValidationOptions,
    #[serde(default)]
    pub(super) quoted_ignore_case: bool,
}

#[derive(Debug)]
pub(crate) struct ParsedValidationRequest<'a> {
    pub(crate) sql: &'a str,
    pub(crate) dialect: DialectType,
    pub(crate) schema: &'a ValidationSchema,
    pub(crate) result: ValidationResult,
    pub(crate) expression: Option<&'a Expression>,
}

struct ClauseResolver<'a, 'b> {
    scope: &'a polyglot_sql::Scope,
    resolver: Resolver<'b>,
}

fn default_dialect() -> String {
    "generic".to_string()
}

pub fn validation_json(request_json: &str) -> Result<String, String> {
    let request: ValidationRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    let result = validation_result(request)?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

pub fn validations_json(request_json: &str) -> Result<String, String> {
    let requests: Vec<ValidationRequest> =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    if requests.is_empty() {
        return Ok("[]".to_string());
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(MAX_VALIDATION_WORKERS.min(requests.len()))
        .stack_size(VALIDATION_WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())?;
    let results: Result<Vec<ValidationResult>, String> =
        pool.install(|| requests.into_par_iter().map(validation_result).collect());
    serde_json::to_string(&results?).map_err(|error| error.to_string())
}

pub(super) fn validation_result(request: ValidationRequest) -> Result<ValidationResult, String> {
    let dialect: DialectType = request.dialect.parse().map_err(|error| {
        format!(
            "unsupported Polyglot dialect '{}': {error}",
            request.dialect
        )
    })?;
    if request.quoted_ignore_case {
        return ProjectCatalog::with_options(dialect, request.options, true)
            .validate(&request.sql, &request.schema);
    }
    let mut result = validate_with_schema(
        request.sql.as_str(),
        dialect,
        &request.schema,
        &request.options,
    );
    if result.valid && may_have_extra_clause_checks(&request.sql) {
        let statements = Dialect::get(dialect)
            .parse_with_options(
                &request.sql,
                &polyglot_sql::ParseOptions {
                    complexity_guard: request.options.complexity_guard,
                },
            )
            .map_err(|error| error.to_string())?;
        let schema = mapping_schema_from_validation_schema_with_dialect(&request.schema, dialect);
        for statement in statements {
            if !statement.dfs().any(|expression| {
                matches!(expression, Expression::Select(select)
                    if select.qualify.is_some() || select.joins.iter().any(|join| !join.using.is_empty()))
            }) {
                continue;
            }
            let scope = build_scope(&statement);
            result
                .errors
                .extend(missing_clause_columns(&scope, &schema));
        }
        result.valid = !result
            .errors
            .iter()
            .any(|error| error.severity == polyglot_sql::ValidationSeverity::Error);
    }
    Ok(result)
}

pub(crate) fn may_have_extra_clause_checks(sql: &str) -> bool {
    sql.as_bytes()
        .windows(5)
        .any(|word| word.eq_ignore_ascii_case(b"using"))
        || sql
            .as_bytes()
            .windows(7)
            .any(|word| word.eq_ignore_ascii_case(b"qualify"))
}

pub(crate) fn complete_parsed_validation(
    request: ParsedValidationRequest<'_>,
) -> Result<ValidationResult, String> {
    let ParsedValidationRequest {
        sql,
        dialect,
        schema,
        mut result,
        expression,
    } = request;
    if !result.valid || !may_have_extra_clause_checks(sql) {
        return Ok(result);
    }
    let mapping = mapping_schema_from_validation_schema_with_dialect(schema, dialect);
    if let Some(expression) = expression {
        result
            .errors
            .extend(missing_clause_columns(&build_scope(expression), &mapping));
    } else {
        for expression in Dialect::get(dialect)
            .parse(sql)
            .map_err(|error| error.to_string())?
        {
            result
                .errors
                .extend(missing_clause_columns(&build_scope(&expression), &mapping));
        }
    }
    result.valid = !result
        .errors
        .iter()
        .any(|error| error.severity == polyglot_sql::ValidationSeverity::Error);
    Ok(result)
}

pub(super) fn missing_clause_columns(
    scope: &polyglot_sql::Scope,
    schema: &polyglot_sql::MappingSchema,
) -> Vec<ValidationError> {
    let mut errors: Vec<ValidationError> = Vec::new();
    let mut clause_resolver = ClauseResolver {
        scope,
        resolver: Resolver::new(scope, schema, false),
    };
    if let Expression::Select(select) = &scope.expression {
        let projection_names: std::collections::HashSet<String> = select
            .expressions
            .iter()
            .filter_map(|expression| match expression {
                Expression::Alias(alias) => Some(alias.alias.to_string().to_lowercase()),
                Expression::Column(column) => Some(column.name.to_string().to_lowercase()),
                _ => None,
            })
            .collect();
        if let Some(qualify) = &select.qualify {
            for node in qualify.this.dfs() {
                let Expression::Column(column) = node else {
                    continue;
                };
                if column.table.is_none()
                    && projection_names.contains(&column.name.to_string().to_lowercase())
                {
                    continue;
                }
                if !clause_resolver.column_exists(column) {
                    let mut error = ValidationError::error(
                        format!("Unknown column '{}' in QUALIFY", column.name),
                        "E201",
                    );
                    if let Some(span) = column.span {
                        error = error
                            .with_location(span.line, span.column)
                            .with_span(Some(span.start), Some(span.end));
                    }
                    errors.push(error);
                }
            }
        }
        for join in &select.joins {
            for identifier in &join.using {
                let column_name = identifier.to_string();
                let mut matching_sources: usize = 0;
                for source_name in scope.sources.keys() {
                    if clause_resolver.source_has_column(source_name, &column_name) {
                        matching_sources += 1;
                    }
                }
                if matching_sources < MINIMUM_USING_SOURCE_COUNT {
                    errors.push(ValidationError::error(
                        format!("Unknown column '{column_name}' in JOIN USING"),
                        "E201",
                    ));
                }
            }
        }
    }
    for child in scope
        .cte_scopes
        .iter()
        .chain(scope.subquery_scopes.iter())
        .chain(scope.derived_table_scopes.iter())
        .chain(scope.udtf_scopes.iter())
        .chain(scope.union_scopes.iter())
    {
        errors.extend(missing_clause_columns(child, schema));
    }
    errors
}

impl ClauseResolver<'_, '_> {
    fn column_exists(&mut self, column: &polyglot_sql::expressions::Column) -> bool {
        let column_name = column.name.to_string();
        if let Some(table) = &column.table {
            return self.source_has_column(&table.to_string(), &column_name);
        }
        let source_names: Vec<String> = self.scope.sources.keys().cloned().collect();
        for source_name in source_names {
            if self.source_has_column(&source_name, &column_name) {
                return true;
            }
        }
        false
    }

    fn source_has_column(&mut self, source_name: &str, column_name: &str) -> bool {
        let Ok(columns) = self.resolver.get_source_columns(source_name) else {
            return self
                .scope
                .sources
                .keys()
                .any(|name| name.eq_ignore_ascii_case(source_name));
        };
        if columns.is_empty() {
            return true;
        }
        columns
            .iter()
            .any(|name| name.eq_ignore_ascii_case(column_name))
    }
}
