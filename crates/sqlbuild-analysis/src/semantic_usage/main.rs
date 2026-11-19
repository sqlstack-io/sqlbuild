//! On-demand direct non-projection column usage extraction.

use crate::parse_guard::main::polyglot_complexity_guard::polyglot_complexity_guard;
use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;
use polyglot_sql::{
    Dialect, DialectType, Expression, ExpressionWalk, Resolver, SchemaValidationOptions,
    ValidationSchema, build_scope, mapping_schema_from_validation_schema_with_dialect,
    validate_with_schema,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Deserialize)]
struct UsageRequest {
    sql: String,
    #[serde(default = "default_dialect")]
    dialect: String,
    schema: ValidationSchema,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageResponse {
    uses: Vec<UsageFact>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageFact {
    context: String,
    expression_sql: String,
    source_name: String,
    source_alias: Option<String>,
    column: String,
    confidence: String,
    start: Option<usize>,
    end: Option<usize>,
    line: Option<usize>,
    column_offset: Option<usize>,
}

struct UsageCollector<'a, 'b> {
    scope: &'a polyglot_sql::Scope,
    resolver: Resolver<'b>,
}

fn default_dialect() -> String {
    "generic".to_string()
}

pub fn analyze_json(request_json: &str) -> Result<String, String> {
    let request: UsageRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    let dialect: DialectType = request.dialect.parse().map_err(|error| {
        format!(
            "unsupported Polyglot dialect '{}': {error}",
            request.dialect
        )
    })?;
    let validation = validate_with_schema(
        &request.sql,
        dialect,
        &request.schema,
        &SchemaValidationOptions {
            check_references: true,
            strict: Some(true),
            complexity_guard: Some(polyglot_complexity_guard()),
            ..SchemaValidationOptions::default()
        },
    );
    if !validation.valid {
        return serde_json::to_string(&UsageResponse { uses: Vec::new() })
            .map_err(|error| error.to_string());
    }
    let statements = Dialect::get(dialect)
        .parse_with_options(&request.sql, &polyglot_parse_options())
        .map_err(|error| error.to_string())?;
    let schema = mapping_schema_from_validation_schema_with_dialect(&request.schema, dialect);
    let mut uses: Vec<UsageFact> = Vec::new();
    for statement in statements {
        let scope = build_scope(&statement);
        uses.extend(collect_scope(&scope, &schema));
    }
    let uses = dedupe(uses);
    serde_json::to_string(&UsageResponse { uses }).map_err(|error| error.to_string())
}

fn collect_scope(
    scope: &polyglot_sql::Scope,
    schema: &polyglot_sql::MappingSchema,
) -> Vec<UsageFact> {
    let mut collector = UsageCollector {
        scope,
        resolver: Resolver::new(scope, schema, false),
    };
    let mut uses: Vec<UsageFact> = Vec::new();
    if let Expression::Select(select) = &scope.expression {
        for join in &select.joins {
            if let Some(expression) = &join.on {
                uses.extend(collector.collect_expression(expression, "join_on"));
            }
            if let Some(expression) = &join.match_condition {
                uses.extend(collector.collect_expression(expression, "join_match_condition"));
            }
            for identifier in &join.using {
                uses.extend(collector.collect_using(identifier.to_string()));
            }
        }
        if let Some(clause) = &select.where_clause {
            uses.extend(collector.collect_expression(&clause.this, "where"));
        }
        if let Some(clause) = &select.group_by {
            for expression in &clause.expressions {
                uses.extend(collector.collect_expression(expression, "group_by"));
            }
        }
        if let Some(clause) = &select.having {
            uses.extend(collector.collect_expression(&clause.this, "having"));
        }
        if let Some(clause) = &select.qualify {
            uses.extend(collector.collect_expression(&clause.this, "qualify"));
        }
        if let Some(clause) = &select.order_by {
            for ordered in &clause.expressions {
                uses.extend(collector.collect_expression(&ordered.this, "order_by"));
            }
        }
        for projection in &select.expressions {
            for node in projection.dfs() {
                if let Expression::WindowFunction(window) = node {
                    for expression in &window.over.partition_by {
                        uses.extend(
                            collector.collect_expression(expression, "window_partition_by"),
                        );
                    }
                    for ordered in &window.over.order_by {
                        uses.extend(collector.collect_expression(&ordered.this, "window_order_by"));
                    }
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
        uses.extend(collect_scope(child, schema));
    }
    uses
}

impl UsageCollector<'_, '_> {
    fn collect_expression(&mut self, expression: &Expression, context: &str) -> Vec<UsageFact> {
        let mut uses: Vec<UsageFact> = Vec::new();
        let expression_sql = expression.to_string();
        for node in expression.dfs() {
            let Expression::Column(column) = node else {
                continue;
            };
            let column_name = column.name.to_string();
            let alias = column
                .table
                .as_ref()
                .map(ToString::to_string)
                .or_else(|| self.resolver.get_table(&column_name));
            let Some(source_alias) = alias else {
                continue;
            };
            let Some(source) = self.scope.sources.get(&source_alias) else {
                continue;
            };
            let source_name =
                physical_source_name(&source.expression).unwrap_or(source_alias.clone());
            let span = column.span;
            uses.push(UsageFact {
                context: context.to_string(),
                expression_sql: expression_sql.clone(),
                source_name,
                source_alias: source.alias.clone().or(Some(source_alias)),
                column: column_name,
                confidence: "high".to_string(),
                start: span.map(|value| value.start),
                end: span.map(|value| value.end),
                line: span.map(|value| value.line),
                column_offset: span.map(|value| value.column),
            });
        }
        uses
    }

    fn collect_using(&mut self, column_name: String) -> Vec<UsageFact> {
        let mut uses: Vec<UsageFact> = Vec::new();
        let source_names: Vec<String> = self.scope.sources.keys().cloned().collect();
        for source_alias in source_names {
            let Ok(columns) = self.resolver.get_source_columns(&source_alias) else {
                continue;
            };
            if !columns.iter().any(|column| column == &column_name) {
                continue;
            }
            let Some(source) = self.scope.sources.get(&source_alias) else {
                continue;
            };
            uses.push(UsageFact {
                context: "join_using".to_string(),
                expression_sql: column_name.clone(),
                source_name: physical_source_name(&source.expression)
                    .unwrap_or(source_alias.clone()),
                source_alias: source.alias.clone().or(Some(source_alias)),
                column: column_name.clone(),
                confidence: "high".to_string(),
                start: None,
                end: None,
                line: None,
                column_offset: None,
            });
        }
        uses
    }
}

fn physical_source_name(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Table(table) => Some(table.name.to_string()),
        _ => None,
    }
}

/// The identity two usage facts share when one repeats the other.
#[derive(PartialEq, Eq, Hash)]
struct UsageKey {
    context: String,
    expression_sql: String,
    source_name: String,
    column: String,
    start: Option<usize>,
}

fn dedupe(mut uses: Vec<UsageFact>) -> Vec<UsageFact> {
    let mut seen: HashSet<UsageKey> = HashSet::new();
    uses.retain(|fact| {
        seen.insert(UsageKey {
            context: fact.context.clone(),
            expression_sql: fact.expression_sql.clone(),
            source_name: fact.source_name.clone(),
            column: fact.column.clone(),
            start: fact.start,
        })
    });
    uses
}
