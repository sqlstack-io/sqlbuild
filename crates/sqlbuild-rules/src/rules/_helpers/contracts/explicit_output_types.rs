use sqlbuild_analysis::sql_macros::models::TypedCall;
use sqlbuild_analysis::type_system::main::normalize_type::normalize_type;

use crate::constants::ENFORCED_CONTRACT;
use crate::models::{Column, Fault, Model, RuleMetadata};
use crate::query_facts::models::{ExpressionItem, ItemFacts, QueryFacts, SelectFacts, SetFacts};
use crate::rules::_helpers::dispatch::evaluation::{
    ParsedModel, plain_select, reads_dependency, sole_table_name,
};
use crate::rules::models::FaultCollector;

#[derive(Clone, Copy)]
struct OutputScope<'a> {
    columns: &'a [Column],
    set_branch: bool,
    by_name: bool,
}

struct OutputRule<'a> {
    parsed: &'a ParsedModel<'a>,
    model: &'a Model,
    rule: &'a RuleMetadata,
    faults: &'a FaultCollector,
    dialect: &'a str,
}

pub(crate) fn evaluate(
    parsed: &ParsedModel<'_>,
    rule: &RuleMetadata,
    faults: &FaultCollector,
    dialect: &str,
) {
    let model = parsed.model;
    let query = parsed.facts;
    if model
        .config
        .get("contract")
        .and_then(serde_json::Value::as_str)
        != Some(ENFORCED_CONTRACT)
        || (!model
            .columns
            .iter()
            .any(|column| !column.data_type.is_empty())
            && !model
                .dynamic_columns
                .iter()
                .any(|family| !family.data_type.is_empty()))
    {
        return;
    }
    let evaluator = OutputRule {
        parsed,
        model,
        rule,
        faults,
        dialect,
    };
    if let Some((boundary, columns)) = final_cte_output_boundary(query, &model.columns) {
        evaluator.evaluate_set(
            &boundary.body,
            OutputScope {
                columns: &columns,
                set_branch: false,
                by_name: false,
            },
        );
        return;
    }
    let columns = columns_in_output_order(&query.body, &model.columns);
    evaluator.evaluate_set(
        &query.body,
        OutputScope {
            columns: &columns,
            set_branch: false,
            by_name: false,
        },
    );
}

pub(crate) fn evaluate_proven_dynamic(model: &Model, rule: &RuleMetadata, faults: &FaultCollector) {
    for column in model
        .columns
        .iter()
        .filter(|column| !column.data_type.is_empty() && !column.type_proven)
    {
        faults.push(Fault {
            unevaluated: false,
            code: rule.code.clone(),
            path: model.relative_path.clone(),
            line: 1,
            column: 1,
            message: format!(
                "fixed dynamic-pivot output {:?} is not proven against declared type {}",
                column.name, column.data_type
            ),
            remediation: format!(
                "Establish type {} on the authoritative pivot input column.",
                column.data_type
            ),
        });
    }
    for family in model
        .dynamic_columns
        .iter()
        .filter(|family| !family.type_proven)
    {
        faults.push(Fault {
            unevaluated: false,
            code: rule.code.clone(),
            path: model.relative_path.clone(),
            line: 1,
            column: 1,
            message: format!(
                "dynamic output family {:?} is not proven against declared type {}",
                family.name, family.data_type
            ),
            remediation: format!(
                "Establish type {} on the authoritative pivot aggregate input.",
                family.data_type
            ),
        });
    }
}

impl OutputRule<'_> {
    fn evaluate_set(&self, set_expr: &SetFacts, scope: OutputScope<'_>) {
        match set_expr {
            SetFacts::Select(select) => self.evaluate_select(select, scope),
            SetFacts::Query(query) => self.evaluate_set(&query.body, scope),
            SetFacts::SetOperation {
                by_name,
                left,
                right,
            } => {
                let branch_scope = OutputScope {
                    columns: scope.columns,
                    set_branch: true,
                    by_name: *by_name,
                };
                self.evaluate_set(left, branch_scope);
                self.evaluate_set(right, branch_scope);
            }
            SetFacts::Other => self.push(
                None,
                format!(
                    "model {:?} does not expose an explicit SELECT output boundary",
                    self.model.name
                ),
                "Project the enforced contract columns explicitly and establish every declared type at that boundary."
                    .into(),
            ),
        }
    }

    fn evaluate_select(&self, select: &SelectFacts, scope: OutputScope<'_>) {
        for (index, item) in select.items.iter().enumerate() {
            let ItemFacts::Expression(expression) = item else {
                if self.model.dynamic_columns_proven
                    && self
                        .model
                        .dynamic_columns
                        .iter()
                        .all(|family| family.type_proven)
                {
                    continue;
                }
                if proven_dependency_wildcard(select, scope.columns) {
                    continue;
                }
                let ItemFacts::Star { item_start, .. } = item else {
                    continue;
                };
                self.push(
                    *item_start,
                    "wildcard output does not establish enforced contract types explicitly".into(),
                    "Enumerate the contract columns; pass through columns with proven exact types and cast every calculated output."
                        .into(),
                );
                continue;
            };
            let output_name = expression.output_name.clone();
            let column = if scope.by_name {
                output_name
                    .as_deref()
                    .and_then(|name| column_named(scope.columns, name))
            } else {
                scope.columns.get(index)
            };
            let Some(column) = column.filter(|column| !column.data_type.is_empty()) else {
                continue;
            };
            let direct = expression.direct_column.is_some();
            let cast_matches_contract = expression
                .cast_type
                .as_ref()
                .is_some_and(|data_type| same_type(data_type, &column.data_type, self.dialect));
            let call: Option<&TypedCall> = self.whole_call(expression);
            let passes = cast_matches_contract || column.type_proven && direct && !scope.set_branch;
            if passes {
                continue;
            }
            let name = output_name.unwrap_or_else(|| column.name.clone());
            if let Some(cast_type) = expression
                .cast_type
                .as_ref()
                .filter(|_| !cast_matches_contract)
            {
                self.push(
                    expression.span.map(|(start, _)| start).or(expression.start),
                    format!(
                        "output {name:?} has an outer cast to {cast_type}, which is not its declared contract type {}",
                        column.data_type
                    ),
                    format!(
                        "Change the outer cast to CAST(... AS {}).",
                        column.data_type
                    ),
                );
                continue;
            }
            if let Some(call) =
                call.filter(|call| call.returns.is_some() || call.returns_like.is_some())
            {
                self.push(
                    expression.start,
                    typed_call_message(&name, call, &column.data_type),
                    format!(
                        "Cast the result in the macro or at the call, as CAST(... AS {}); returns \
                         types the call for checks but does not change the column's physical type.",
                        call.returns.as_deref().unwrap_or(&column.data_type)
                    ),
                );
                continue;
            }
            let reason = if direct && scope.set_branch {
                "is in a set-operation branch that must cast each output explicitly"
            } else if direct {
                "is a passthrough whose type is not proven against the declared contract"
            } else {
                "is calculated without an outer explicit cast"
            };
            self.push(
                expression.start,
                format!(
                    "output {:?} {reason}; declared type is {}",
                    name, column.data_type
                ),
                match call.filter(|call| call.returns.is_none() && call.returns_like.is_none()) {
                    Some(call) => format!(
                        "Declare returns {} in SQL macro {}(), or wrap the call in CAST(... AS {}).",
                        column.data_type, call.macro_name, column.data_type
                    ),
                    None => format!(
                        "Wrap the complete expression in CAST(... AS {}) or establish that exact type on a direct upstream column.",
                        column.data_type
                    ),
                },
            );
        }
    }

    /// The expression SQL macro call `expression` is the whole of, found by its byte range.
    fn whole_call(&self, expression: &ExpressionItem) -> Option<&TypedCall> {
        let (start, end) = expression.span?;
        let (start, end) = self.parsed.source.byte_range(start, end);
        let sql: &str = &self.model.query_sql;
        self.model.typed_calls.iter().rev().find(|call| {
            call.start == start
                && end <= call.end
                && sql.get(end..call.end).is_some_and(only_trivia)
        })
    }

    fn push(&self, offset: Option<usize>, message: String, remediation: String) {
        let (line, column) = offset.map_or((1, 1), |offset| self.parsed.source.line_column(offset));
        self.faults.push(Fault {
            unevaluated: false,
            code: self.rule.code.clone(),
            path: self.model.relative_path.clone(),
            line,
            column,
            message,
            remediation,
        });
    }
}

/// Why one call to a typed SQL macro does not establish its output's contract type.
fn typed_call_message(name: &str, call: &TypedCall, declared: &str) -> String {
    let macro_name: &str = &call.macro_name;
    match call.returns.as_deref() {
        Some(returns) => format!(
            "output {name:?} is a call to SQL macro {macro_name}(), which declares returns \
             {returns}, but its body isn't cast to it, so the column's physical type may differ; \
             declared type is {declared}"
        ),
        None => format!(
            "output {name:?} is a call to SQL macro {macro_name}(), whose returns TYPE OF does \
             not set the column's physical type; declared type is {declared}"
        ),
    }
}

/// Whether `text` holds only whitespace and comments.
fn only_trivia(mut text: &str) -> bool {
    loop {
        text = text.trim_start();
        if text.is_empty() {
            return true;
        }
        if let Some(comment) = text.strip_prefix("--") {
            text = comment.split_once('\n').map_or("", |(_, rest)| rest);
        } else if let Some(comment) = text.strip_prefix("/*") {
            let Some((_, rest)) = comment.split_once("*/") else {
                return false;
            };
            text = rest;
        } else {
            return false;
        }
    }
}

fn proven_dependency_wildcard(select: &SelectFacts, columns: &[Column]) -> bool {
    plain_select(select, true)
        && reads_dependency(select)
        && columns
            .iter()
            .filter(|column| !column.data_type.is_empty())
            .all(|column| column.type_proven)
}

fn final_cte_output_boundary<'a>(
    query: &'a QueryFacts,
    columns: &[Column],
) -> Option<(&'a QueryFacts, Vec<Column>)> {
    let final_cte = query.ctes.last()?;
    let terminal_reads_final_cte = sole_table_name(query)
        .is_some_and(|name| name.eq_ignore_ascii_case(&final_cte.name))
        && query
            .root_select()
            .is_some_and(|select| plain_select(select, true))
        && !query.has_order_by
        && !query.has_limit
        && !query.has_fetch
        && !query.has_locks;
    if !terminal_reads_final_cte {
        return None;
    }
    let select = query.root_select()?;
    if select.items.len() == 1 && matches!(select.items[0], ItemFacts::Star { .. }) {
        return Some((
            &final_cte.query,
            columns_in_output_order_with_missing(
                &final_cte.query.body,
                columns,
                &final_cte.columns,
            ),
        ));
    }
    if select.items.len() != columns.len() {
        return None;
    }
    let mut boundary_columns: Vec<Column> = Vec::with_capacity(columns.len());
    for item in &select.items {
        let ItemFacts::Expression(expression) = item else {
            return None;
        };
        let column = column_named(columns, expression.output_name.as_deref()?)?;
        let mut boundary_column = column.clone();
        boundary_column.name = expression.direct_column.clone()?;
        boundary_columns.push(boundary_column);
    }
    let ordered_columns = columns_in_output_order_with_missing(
        &final_cte.query.body,
        &boundary_columns,
        &final_cte.columns,
    );
    Some((&final_cte.query, ordered_columns))
}

fn columns_in_output_order_with_missing(
    set_expr: &SetFacts,
    columns: &[Column],
    cte_column_aliases: &[String],
) -> Vec<Column> {
    let Some(output_names) = set_expression_output_names_with_missing(set_expr) else {
        return columns.to_vec();
    };
    if output_names.len() == 1 && output_names[0].is_none() {
        return columns.to_vec();
    }
    output_names
        .into_iter()
        .enumerate()
        .map(|(index, body_name)| {
            let exposed_name = cte_column_aliases
                .get(index)
                .map(String::as_str)
                .or(body_name.as_deref());
            let mut column = exposed_name
                .and_then(|name| column_named(columns, name))
                .cloned()
                .unwrap_or_default();
            if let Some(body_name) = body_name {
                column.name = body_name;
            }
            column
        })
        .collect()
}

fn columns_in_output_order(set_expr: &SetFacts, columns: &[Column]) -> Vec<Column> {
    let Some(output_names) = set_expression_output_names(set_expr) else {
        return columns.to_vec();
    };
    let mut ordered: Vec<Column> = Vec::with_capacity(output_names.len());
    for name in output_names {
        let Some(column) = column_named(columns, &name) else {
            return columns.to_vec();
        };
        ordered.push(column.clone());
    }
    ordered
}

fn column_named<'a>(columns: &'a [Column], name: &str) -> Option<&'a Column> {
    columns
        .iter()
        .find(|column| column.name.eq_ignore_ascii_case(name))
}

fn projection_output_name(item: &ItemFacts) -> Option<String> {
    match item {
        ItemFacts::Expression(expression) => expression.output_name.clone(),
        ItemFacts::Star { .. } => None,
    }
}

fn set_expression_output_names(set_expr: &SetFacts) -> Option<Vec<String>> {
    match set_expr {
        SetFacts::Select(select) => select.items.iter().map(projection_output_name).collect(),
        SetFacts::Query(query) => set_expression_output_names(&query.body),
        SetFacts::SetOperation { left, .. } => set_expression_output_names(left),
        SetFacts::Other => None,
    }
}

fn set_expression_output_names_with_missing(set_expr: &SetFacts) -> Option<Vec<Option<String>>> {
    match set_expr {
        SetFacts::Select(select) => Some(select.items.iter().map(projection_output_name).collect()),
        SetFacts::Query(query) => set_expression_output_names_with_missing(&query.body),
        SetFacts::SetOperation {
            by_name,
            left,
            right,
        } => {
            let mut names = set_expression_output_names_with_missing(left)?;
            if *by_name {
                for right_name in set_expression_output_names_with_missing(right)? {
                    let already_present = right_name.as_ref().is_some_and(|right_name| {
                        names
                            .iter()
                            .flatten()
                            .any(|left_name| left_name.eq_ignore_ascii_case(right_name))
                    });
                    if !already_present {
                        names.push(right_name);
                    }
                }
            }
            Some(names)
        }
        SetFacts::Other => None,
    }
}

/// Whether two type spellings name one type in `dialect`, such as Snowflake `VARCHAR` and `STRING`.
fn same_type(left: &str, right: &str, dialect: &str) -> bool {
    if type_spelling_equal(left, right) {
        return true;
    }
    match (
        normalize_type(left, dialect),
        normalize_type(right, dialect),
    ) {
        (Ok(left), Ok(right)) => {
            left.parse_error.is_none()
                && right.parse_error.is_none()
                && left.normalized == right.normalized
        }
        _ => false,
    }
}

fn type_spelling_equal(left: &str, right: &str) -> bool {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|character| !character.is_whitespace())
            .flat_map(char::to_uppercase)
            .collect::<String>()
    };
    normalize(left) == normalize(right)
}
