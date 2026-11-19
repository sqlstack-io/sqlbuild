//! Resolve every reference to one relation column, with the spans a rename rewrites.

use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;
use polyglot_sql::{
    Dialect, DialectType, Expression, MappingSchema, Resolver, Scope, ValidationSchema,
    build_scope, mapping_schema_from_validation_schema_with_dialect, walk_in_scope,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Deserialize)]
struct ReferenceRequest {
    sql: String,
    #[serde(default = "default_dialect")]
    dialect: String,
    schema: ValidationSchema,
    column: String,
    #[serde(default)]
    target_tables: Vec<String>,
    #[serde(default)]
    target_ctes: Vec<String>,
    #[serde(default)]
    output_ctes: Vec<String>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceResponse {
    parsed: bool,
    references: Vec<ReferenceSite>,
    stars: Vec<StarSite>,
    joins: Vec<SpanSite>,
    unresolved: Vec<SpanSite>,
    outputs: Vec<OutputSite>,
    star_outputs: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceSite {
    start: usize,
    end: usize,
    name_start: usize,
    name_end: usize,
    scope: String,
    projection: Option<ProjectionSite>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionSite {
    alias: Option<String>,
    alias_start: Option<usize>,
    alias_end: Option<usize>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StarSite {
    start: Option<usize>,
    end: Option<usize>,
    scope: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpanSite {
    start: Option<usize>,
    end: Option<usize>,
    scope: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputSite {
    scope: String,
    start: Option<usize>,
    end: Option<usize>,
    alias_start: Option<usize>,
    alias_end: Option<usize>,
}

const ROOT_SCOPE: &str = "root";
const DERIVED_SCOPE: &str = "derived";
const SUBQUERY_SCOPE: &str = "subquery";
const CTE_SCOPE_PREFIX: &str = "cte:";
const STAR: &str = "*";

struct Analysis<'a> {
    schema: &'a MappingSchema,
    column: String,
    target_tables: HashSet<String>,
    target_ctes: HashSet<String>,
    output_ctes: HashSet<String>,
    response: ReferenceResponse,
    seen: HashSet<(usize, usize)>,
    later_branch: bool,
}

fn default_dialect() -> String {
    "generic".to_string()
}

pub(crate) fn analyze_json(request_json: &str) -> Result<String, String> {
    let request: ReferenceRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    let dialect: DialectType = request.dialect.parse().map_err(|error| {
        format!(
            "unsupported Polyglot dialect '{}': {error}",
            request.dialect
        )
    })?;
    let Ok(statements) =
        Dialect::get(dialect).parse_with_options(&request.sql, &polyglot_parse_options())
    else {
        return serde_json::to_string(&ReferenceResponse::default())
            .map_err(|error| error.to_string());
    };
    let schema = mapping_schema_from_validation_schema_with_dialect(&request.schema, dialect);
    let scopes: Vec<Scope> = statements.iter().map(build_scope).collect();
    let ctes: Vec<&polyglot_sql::expressions::Cte> = scopes.iter().flat_map(scope_ctes).collect();
    let target_ctes: HashSet<String> = star_derived_ctes(
        &ctes,
        &lowered(&request.target_tables),
        lowered(&request.target_ctes),
    );
    let mut analysis = Analysis {
        schema: &schema,
        column: request.column.to_lowercase(),
        target_tables: lowered(&request.target_tables),
        target_ctes,
        output_ctes: lowered(&request.output_ctes),
        response: ReferenceResponse {
            parsed: true,
            ..ReferenceResponse::default()
        },
        seen: HashSet::new(),
        later_branch: false,
    };
    for scope in &scopes {
        analysis.visit(scope, ROOT_SCOPE.to_string(), &[], false);
    }
    serde_json::to_string(&analysis.response).map_err(|error| error.to_string())
}

fn lowered(names: &[String]) -> HashSet<String> {
    names.iter().map(|name| name.to_lowercase()).collect()
}

fn scope_ctes(scope: &Scope) -> Vec<&polyglot_sql::expressions::Cte> {
    let mut ctes: Vec<&polyglot_sql::expressions::Cte> = Vec::new();
    if let Expression::Cte(cte) = &scope.expression {
        ctes.push(cte);
    }
    for child in scope
        .cte_scopes
        .iter()
        .chain(scope.derived_table_scopes.iter())
        .chain(scope.subquery_scopes.iter())
        .chain(scope.udtf_scopes.iter())
        .chain(scope.union_scopes.iter())
    {
        ctes.extend(scope_ctes(child));
    }
    ctes
}

/// CTEs whose output passes the target column through a star become targets themselves.
fn star_derived_ctes(
    ctes: &[&polyglot_sql::expressions::Cte],
    target_tables: &HashSet<String>,
    seed: HashSet<String>,
) -> HashSet<String> {
    let mut derived: HashSet<String> = seed;
    loop {
        let before = derived.len();
        for cte in ctes {
            let name = cte.alias.name.to_lowercase();
            if !derived.contains(&name)
                && selects_star_over_targets(&cte.this, target_tables, &derived)
            {
                derived.insert(name);
            }
        }
        if derived.len() == before {
            return derived;
        }
    }
}

/// Only the first branch of a set operation names its output columns.
fn selects_star_over_targets(
    body: &Expression,
    target_tables: &HashSet<String>,
    target_ctes: &HashSet<String>,
) -> bool {
    first_branch_select(body)
        .is_some_and(|select| select_stars_over_targets(select, target_tables, target_ctes))
}

fn select_stars_over_targets(
    select: &polyglot_sql::expressions::Select,
    target_tables: &HashSet<String>,
    target_ctes: &HashSet<String>,
) -> bool {
    let relations: HashMap<String, String> = select_relation_names(select);
    for item in &select.expressions {
        let covered: bool = match star_table(item) {
            Some(None) => relations
                .values()
                .any(|name| is_target_name(name, target_tables, target_ctes)),
            Some(Some(qualifier)) => relations
                .get(&qualifier)
                .is_some_and(|name| is_target_name(name, target_tables, target_ctes)),
            None => false,
        };
        if covered {
            return true;
        }
    }
    false
}

fn is_target_name(
    name: &str,
    target_tables: &HashSet<String>,
    target_ctes: &HashSet<String>,
) -> bool {
    target_tables.contains(name) || target_ctes.contains(name)
}

fn first_branch_select(body: &Expression) -> Option<&polyglot_sql::expressions::Select> {
    match body {
        Expression::Select(select) => Some(select),
        Expression::Union(union) => first_branch_select(&union.left),
        Expression::Intersect(intersect) => first_branch_select(&intersect.left),
        Expression::Except(except) => first_branch_select(&except.left),
        Expression::Subquery(subquery) => first_branch_select(&subquery.this),
        Expression::Paren(paren) => first_branch_select(&paren.this),
        _ => None,
    }
}

/// Map each visible alias of a SELECT's FROM and JOIN tables to its lower-cased table name.
fn select_relation_names(select: &polyglot_sql::expressions::Select) -> HashMap<String, String> {
    let mut names: HashMap<String, String> = HashMap::new();
    let tables = select
        .from
        .iter()
        .flat_map(|from| from.expressions.iter())
        .chain(select.joins.iter().map(|join| &join.this));
    for table in tables {
        if let Expression::Table(table) = table {
            let name = table.name.name.to_lowercase();
            let visible = table
                .alias
                .as_ref()
                .map_or_else(|| name.clone(), |alias| alias.name.to_lowercase());
            names.insert(visible, name);
        }
    }
    names
}

/// `Some(None)` for `*`, `Some(Some(qualifier))` for `t.*`, `None` otherwise.
fn star_table(item: &Expression) -> Option<Option<String>> {
    match item {
        Expression::Star(star) => Some(star.table.as_ref().map(|table| table.name.to_lowercase())),
        Expression::Column(column) if column.name.name == STAR => {
            Some(column.table.as_ref().map(|table| table.name.to_lowercase()))
        }
        _ => None,
    }
}

fn star_span(item: &Expression) -> (Option<usize>, Option<usize>) {
    match item {
        Expression::Star(star) => (
            star.span.map(|span| span.start),
            star.span.map(|span| span.end),
        ),
        Expression::Column(column) => (
            column.span.map(|span| span.start),
            column.span.map(|span| span.end),
        ),
        _ => (None, None),
    }
}

fn scope_select(expression: &Expression) -> Option<&polyglot_sql::expressions::Select> {
    match select_body(expression)? {
        Expression::Select(select) => Some(select),
        _ => None,
    }
}

fn select_body(expression: &Expression) -> Option<&Expression> {
    match expression {
        Expression::Select(_) => Some(expression),
        Expression::Cte(cte) => select_body(&cte.this),
        Expression::Subquery(subquery) => select_body(&subquery.this),
        Expression::Paren(paren) => select_body(&paren.this),
        _ => None,
    }
}

impl Analysis<'_> {
    /// Visit one scope; `later_branch` marks a set-operation branch that names no outputs.
    fn visit(&mut self, scope: &Scope, role: String, outer: &[&Scope], later_branch: bool) {
        if let Some(select) = scope_select(&scope.expression) {
            let local: Scope = local_scope(scope, select);
            let mut chain: Vec<&Scope> = vec![&local];
            chain.extend(outer.iter().copied());
            self.later_branch = later_branch;
            self.visit_select(select, &scope.expression, &chain, &role);
        }
        for child in &scope.cte_scopes {
            let name = match &child.expression {
                Expression::Cte(cte) => cte.alias.name.clone(),
                _ => String::new(),
            };
            self.visit(child, format!("{CTE_SCOPE_PREFIX}{name}"), &[], false);
        }
        for child in &scope.derived_table_scopes {
            self.visit(child, DERIVED_SCOPE.to_string(), &[], false);
        }
        let mut nested: Vec<&Scope> = vec![scope];
        nested.extend(outer.iter().copied());
        for child in scope.subquery_scopes.iter().chain(scope.udtf_scopes.iter()) {
            self.visit(child, SUBQUERY_SCOPE.to_string(), &nested, false);
        }
        for (position, child) in scope.union_scopes.iter().enumerate() {
            self.visit(child, role.clone(), outer, later_branch || position > 0);
        }
    }

    fn visit_select(
        &mut self,
        select: &polyglot_sql::expressions::Select,
        expression: &Expression,
        chain: &[&Scope],
        role: &str,
    ) {
        self.record_outputs(select, role);
        if !self.later_branch {
            self.record_stars(select, chain[0], role);
        }
        self.record_joins(select, chain[0], role);
        let Some(body) = select_body(expression) else {
            return;
        };
        let shadowed: HashSet<(usize, usize)> = self.lambda_parameter_uses(body);
        for node in walk_in_scope(body, false) {
            let Expression::Column(column) = node else {
                continue;
            };
            if column.name.name == STAR || column.name.name.to_lowercase() != self.column {
                continue;
            }
            if column.table.is_none()
                && column
                    .span
                    .is_some_and(|span| shadowed.contains(&(span.start, span.end)))
            {
                continue;
            }
            self.record_column(column, select, chain, role);
        }
    }

    /// Spans of unqualified uses of a lambda parameter named like the column, which read the parameter.
    fn lambda_parameter_uses(&self, body: &Expression) -> HashSet<(usize, usize)> {
        let mut shadowed: HashSet<(usize, usize)> = HashSet::new();
        for node in walk_in_scope(body, false) {
            let Expression::Lambda(lambda) = node else {
                continue;
            };
            if !lambda
                .parameters
                .iter()
                .any(|parameter| parameter.name.to_lowercase() == self.column)
            {
                continue;
            }
            for inner in walk_in_scope(&lambda.body, false) {
                if let Expression::Column(column) = inner
                    && column.table.is_none()
                    && column.name.name.to_lowercase() == self.column
                    && let Some(span) = column.span
                {
                    shadowed.insert((span.start, span.end));
                }
            }
        }
        shadowed
    }

    fn record_column(
        &mut self,
        column: &polyglot_sql::expressions::Column,
        select: &polyglot_sql::expressions::Select,
        chain: &[&Scope],
        role: &str,
    ) {
        let (Some(span), Some(name_span)) = (column.span, column.name.span) else {
            return;
        };
        if !self.seen.insert((span.start, span.end)) {
            return;
        }
        match self.resolve(column, chain) {
            Resolution::Target => self.response.references.push(ReferenceSite {
                start: span.start,
                end: span.end,
                name_start: name_span.start,
                name_end: name_span.end,
                scope: role.to_string(),
                projection: if self.later_branch {
                    None
                } else {
                    projection_site(select, span.start)
                },
            }),
            Resolution::Other => {}
            Resolution::Unknown => {
                if chain.iter().any(|scope| self.has_target_source(scope)) {
                    self.response.unresolved.push(SpanSite {
                        start: Some(span.start),
                        end: Some(span.end),
                        scope: role.to_string(),
                    });
                }
            }
        }
    }

    fn resolve(&self, column: &polyglot_sql::expressions::Column, chain: &[&Scope]) -> Resolution {
        if let Some(qualifier) = &column.table {
            for scope in chain {
                if let Some(alias) = source_key(scope, &qualifier.name) {
                    return self.classify(scope, &alias);
                }
            }
            return Resolution::Unknown;
        }
        for scope in chain {
            let mut resolver = Resolver::new(scope, self.schema, false);
            if resolver.is_ambiguous(&column.name.name) {
                return Resolution::Unknown;
            }
            if let Some(alias) = resolver.get_table(&column.name.name) {
                return self.classify(scope, &alias);
            }
        }
        Resolution::Unknown
    }

    fn classify(&self, scope: &Scope, alias: &str) -> Resolution {
        match scope.sources.get(alias) {
            Some(source) if self.is_target(&source.expression) => Resolution::Target,
            Some(_) => Resolution::Other,
            None => Resolution::Unknown,
        }
    }

    fn is_target(&self, expression: &Expression) -> bool {
        match expression {
            Expression::Table(table) => {
                table.schema.is_none()
                    && self.target_tables.contains(&table.name.name.to_lowercase())
            }
            Expression::Cte(cte) => self.target_ctes.contains(&cte.alias.name.to_lowercase()),
            _ => false,
        }
    }

    fn has_target_source(&self, scope: &Scope) -> bool {
        scope
            .sources
            .values()
            .any(|source| self.is_target(&source.expression))
    }

    fn record_stars(
        &mut self,
        select: &polyglot_sql::expressions::Select,
        scope: &Scope,
        role: &str,
    ) {
        for item in &select.expressions {
            let covers_target = match star_table(item) {
                Some(None) => self.has_target_source(scope),
                Some(Some(qualifier)) => source_key(scope, &qualifier)
                    .and_then(|alias| scope.sources.get(&alias))
                    .is_some_and(|source| self.is_target(&source.expression)),
                None => false,
            };
            if covers_target {
                let (start, end) = star_span(item);
                self.response.stars.push(StarSite {
                    start,
                    end,
                    scope: role.to_string(),
                });
            }
        }
    }

    fn record_joins(
        &mut self,
        select: &polyglot_sql::expressions::Select,
        scope: &Scope,
        role: &str,
    ) {
        if !self.has_target_source(scope) {
            return;
        }
        for join in &select.joins {
            let natural = format!("{:?}", join.kind).starts_with("Natural");
            let using = join
                .using
                .iter()
                .find(|identifier| identifier.name.to_lowercase() == self.column);
            if natural || using.is_some() {
                let span = using.and_then(|identifier| identifier.span);
                self.response.joins.push(SpanSite {
                    start: span.map(|value| value.start),
                    end: span.map(|value| value.end),
                    scope: role.to_string(),
                });
            }
        }
    }

    fn record_outputs(&mut self, select: &polyglot_sql::expressions::Select, role: &str) {
        let name = role.strip_prefix(CTE_SCOPE_PREFIX).unwrap_or(role);
        if !self.output_ctes.contains(&name.to_lowercase()) {
            return;
        }
        for item in &select.expressions {
            match item {
                Expression::Alias(alias) if alias.alias.name.to_lowercase() == self.column => {
                    self.response.outputs.push(OutputSite {
                        scope: role.to_string(),
                        start: None,
                        end: None,
                        alias_start: alias.alias.span.map(|span| span.start),
                        alias_end: alias.alias.span.map(|span| span.end),
                    });
                }
                Expression::Column(column) if column.name.name.to_lowercase() == self.column => {
                    self.response.outputs.push(OutputSite {
                        scope: role.to_string(),
                        start: column.span.map(|span| span.start),
                        end: column.span.map(|span| span.end),
                        alias_start: None,
                        alias_end: None,
                    });
                }
                _ if star_table(item).is_some() => {
                    self.response.star_outputs.push(role.to_string());
                }
                _ => {}
            }
        }
    }
}

enum Resolution {
    Target,
    Other,
    Unknown,
}

/// Keep only the CTEs a SELECT reads; Polyglot also lists every visible CTE as a source.
fn local_scope(scope: &Scope, select: &polyglot_sql::expressions::Select) -> Scope {
    let read: HashSet<String> = select_relation_names(select).into_keys().collect();
    let mut local: Scope = Scope::new(scope.expression.clone());
    local.sources = scope
        .sources
        .iter()
        .filter(|(name, source)| {
            !matches!(*source.expression, Expression::Cte(_)) || read.contains(&name.to_lowercase())
        })
        .map(|(name, source)| (name.clone(), source.clone()))
        .collect();
    local.lateral_sources = scope.lateral_sources.clone();
    local
}

fn source_key(scope: &Scope, name: &str) -> Option<String> {
    if scope.sources.contains_key(name) {
        return Some(name.to_string());
    }
    scope
        .sources
        .keys()
        .find(|key| key.eq_ignore_ascii_case(name))
        .cloned()
}

fn projection_site(
    select: &polyglot_sql::expressions::Select,
    start: usize,
) -> Option<ProjectionSite> {
    for item in &select.expressions {
        match item {
            Expression::Column(column) if column.span.map(|span| span.start) == Some(start) => {
                return Some(ProjectionSite {
                    alias: None,
                    alias_start: None,
                    alias_end: None,
                });
            }
            Expression::Alias(alias) => {
                if let Expression::Column(column) = &alias.this
                    && column.span.map(|span| span.start) == Some(start)
                {
                    return Some(ProjectionSite {
                        alias: Some(alias.alias.name.clone()),
                        alias_start: alias.alias.span.map(|span| span.start),
                        alias_end: alias.alias.span.map(|span| span.end),
                    });
                }
            }
            _ => {}
        }
    }
    None
}
