use crate::query_facts::_helpers::shape::stub_call;
use crate::query_facts::_helpers::tokens::{ComparisonOperator, QueryTokens, SelectTokens};
use crate::query_facts::constants::{
    MAX_FUNCTION_CALL_DEPTH, TABLE_FUNCTION_STUB, TRIPLE_QUOTE_LENGTH,
};
use crate::query_facts::models::{
    CallKind, ColumnFact, ComparisonFacts, ComparisonSpan, CteFacts, ExpressionItem, ItemFacts,
    ModelShape, ParsedFixture, QueryFacts, RelationFacts, SelectFacts, SetFacts, SourceText,
};
use polyglot_sql::dialects::Dialect;
use polyglot_sql::expressions::{Identifier, Literal, Select, TableRef, With};
use polyglot_sql::{ComplexityGuardOptions, Expression, ExpressionWalk, ParseOptions};
use sqlbuild_analysis::lineage::main::parser_dialect::parser_dialect;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

fn parse_query(sql: &str, dialect_name: &str) -> Result<(Expression, Dialect), String> {
    let dialect_type = parser_dialect(Some(dialect_name))
        .map_err(|unknown| format!("unknown dialect {unknown}"))?;
    let dialect = Dialect::get(dialect_type);
    let options = ParseOptions {
        complexity_guard: Some(ComplexityGuardOptions {
            max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
            ..Default::default()
        }),
    };
    let mut statements = dialect
        .parse_with_options(sql, &options)
        .map_err(|error| error.to_string())?;
    if statements.len() != 1 {
        return Err("expected one statement".to_owned());
    }
    let statement = statements.remove(0);
    if !is_query(&statement) {
        return Err("expected a query".to_owned());
    }
    Ok((statement, dialect))
}

/// Parse a stubbed SQL-test fixture as one query.
pub(crate) fn parse_fixture(
    shape: ModelShape,
    dialect_name: &str,
) -> Result<ParsedFixture, String> {
    let (statement, dialect) = parse_query(&shape.sql, dialect_name)?;
    let tokens = dialect
        .tokenize(&shape.sql)
        .map_err(|error| error.to_string())?;
    let tokens = QueryTokens::new(tokens, &shape.sql);
    let selects = tokens.selects();
    let order = tokens.top_level_word("ORDER").unwrap_or(tokens.len());
    let order_end = ["LIMIT", "OFFSET", "FETCH"]
        .into_iter()
        .filter_map(|word| tokens.top_level_word(word))
        .filter(|index| *index > order)
        .min()
        .unwrap_or(tokens.len());
    let projection_function = selects.first().is_some_and(|select| {
        select
            .items
            .iter()
            .any(|item| tokens.calls_function(item.clone()))
    });
    Ok(ParsedFixture {
        select_function: tokens.calls_function(0..order),
        projection_function,
        order_by_function: tokens.calls_function(order..order_end),
        selects: selects.len(),
        calls: shape.calls,
        statement,
    })
}

/// Parse normalized model SQL with Polyglot and read the facts built-in rules evaluate.
pub(crate) fn query_facts(sql: &str, dialect_name: &str) -> Result<QueryFacts, String> {
    let (statement, dialect) = parse_query(sql, dialect_name)?;
    let tokens = dialect.tokenize(sql).map_err(|error| error.to_string())?;
    let tokens = QueryTokens::new(tokens, sql);
    let builder = Builder {
        selects: tokens.selects(),
        tokens,
        spans: RefCell::default(),
        cursor: Cell::default(),
        claimed: RefCell::default(),
    };
    Ok(builder.query(&statement))
}

fn is_query(expression: &Expression) -> bool {
    match expression {
        Expression::Select(_)
        | Expression::Union(_)
        | Expression::Intersect(_)
        | Expression::Except(_)
        | Expression::Values(_) => true,
        Expression::Subquery(subquery) => subquery.alias.is_none() && is_query(&subquery.this),
        Expression::Paren(paren) => is_query(&paren.this),
        _ => false,
    }
}

struct Builder<'a> {
    tokens: QueryTokens<'a>,
    selects: Vec<SelectTokens>,
    spans: RefCell<HashMap<*const Expression, Option<ComparisonSpan>>>,
    cursor: Cell<usize>,
    claimed: RefCell<HashSet<usize>>,
}

struct ComparisonParts<'a> {
    operator: ComparisonOperator,
    operands: Vec<&'a Expression>,
    negated: bool,
}

struct Modifiers<'a> {
    with: Option<&'a With>,
    order_by: bool,
    limit: bool,
    fetch: bool,
    locks: bool,
}

impl Builder<'_> {
    fn query(&self, expression: &Expression) -> QueryFacts {
        let modifiers: Modifiers<'_> = match expression {
            Expression::Select(select) => Modifiers {
                with: select.with.as_ref(),
                order_by: select.order_by.is_some(),
                limit: select.limit.is_some() || select.offset.is_some(),
                fetch: select.fetch.is_some(),
                locks: !select.locks.is_empty(),
            },
            Expression::Union(set) => set_modifiers(
                set.with.as_ref(),
                set.order_by.is_some(),
                set.limit.is_some() || set.offset.is_some(),
            ),
            Expression::Intersect(set) => set_modifiers(
                set.with.as_ref(),
                set.order_by.is_some(),
                set.limit.is_some() || set.offset.is_some(),
            ),
            Expression::Except(set) => set_modifiers(
                set.with.as_ref(),
                set.order_by.is_some(),
                set.limit.is_some() || set.offset.is_some(),
            ),
            _ => set_modifiers(None, false, false),
        };
        let mut ctes: Vec<CteFacts> = Vec::new();
        for cte in modifiers.with.iter().flat_map(|with| with.ctes.iter()) {
            ctes.push(CteFacts {
                name: cte.alias.name.clone(),
                name_start: cte.alias.span.map(|span| span.start),
                columns: cte
                    .columns
                    .iter()
                    .map(|column| column.name.clone())
                    .collect(),
                query: self.query(&cte.this),
                dependency_calls: dependency_calls(&cte.this),
            });
        }
        let body = match expression {
            Expression::Subquery(subquery) => SetFacts::Query(Box::new(self.query(&subquery.this))),
            Expression::Paren(paren) => SetFacts::Query(Box::new(self.query(&paren.this))),
            _ => self.set(expression),
        };
        QueryFacts {
            ctes,
            body,
            has_order_by: modifiers.order_by,
            has_limit: modifiers.limit,
            has_fetch: modifiers.fetch,
            has_locks: modifiers.locks,
        }
    }

    fn set(&self, expression: &Expression) -> SetFacts {
        match expression {
            Expression::Select(select) => SetFacts::Select(Box::new(self.select(select))),
            Expression::Union(set) => self.set_operation(set.by_name, &set.left, &set.right),
            Expression::Intersect(set) => self.set_operation(set.by_name, &set.left, &set.right),
            Expression::Except(set) => self.set_operation(set.by_name, &set.left, &set.right),
            Expression::Subquery(subquery) if subquery.alias.is_none() => {
                SetFacts::Query(Box::new(self.query(&subquery.this)))
            }
            Expression::Paren(paren) => SetFacts::Query(Box::new(self.query(&paren.this))),
            _ => SetFacts::Other,
        }
    }

    fn set_operation(&self, by_name: bool, left: &Expression, right: &Expression) -> SetFacts {
        SetFacts::SetOperation {
            by_name,
            left: Box::new(self.set(left)),
            right: Box::new(self.set(right)),
        }
    }

    fn select(&self, select: &Select) -> SelectFacts {
        let from_first: Option<usize> = select_anchor(select, &self.tokens)
            .and_then(|anchor| self.tokens.leading_from_keyword(anchor));
        let tokens: Option<&SelectTokens> = self
            .select_tokens(select)
            .filter(|tokens| tokens.items.len() == select.expressions.len())
            .filter(|tokens| from_first.is_none_or(|from| tokens.start > from));
        let mut items: Vec<ItemFacts> = Vec::with_capacity(select.expressions.len());
        for (index, expression) in select.expressions.iter().enumerate() {
            let item: Option<&Range<usize>> = tokens.map(|tokens| &tokens.items[index]);
            items.push(self.item(expression, item, from_first));
        }
        let mut comparisons: Vec<ComparisonFacts> = Vec::new();
        for join in &select.joins {
            if let Some(on) = &join.on {
                comparisons.extend(self.comparisons(on, None));
            }
        }
        for expression in &select.expressions {
            let (alias, value) = match expression {
                Expression::Alias(alias) => (Some(alias.alias.name.as_str()), &alias.this),
                _ => (None, expression),
            };
            comparisons.extend(self.case_comparisons(value, alias));
        }
        for clause in [
            select.where_clause.as_ref().map(|clause| &clause.this),
            select.having.as_ref().map(|clause| &clause.this),
            select.qualify.as_ref().map(|clause| &clause.this),
        ]
        .into_iter()
        .flatten()
        {
            comparisons.extend(self.comparisons(clause, None));
        }
        SelectFacts {
            items,
            from: select
                .from
                .iter()
                .flat_map(|from| from.expressions.iter())
                .map(|expression| relation(expression, &self.tokens))
                .collect(),
            joins: select
                .joins
                .iter()
                .map(|join| relation(&join.this, &self.tokens))
                .collect(),
            filtered: select.where_clause.is_some()
                || select.having.is_some()
                || select.qualify.is_some()
                || select.group_by.is_some(),
            comparisons,
        }
    }

    /// The SELECT region around the select's first located part, else the next unclaimed one.
    fn select_tokens(&self, select: &Select) -> Option<&SelectTokens> {
        let found = match select_anchor(select, &self.tokens) {
            Some(anchor) => self
                .selects
                .iter()
                .filter(|tokens| tokens.start < anchor && anchor < tokens.end)
                .max_by_key(|tokens| tokens.start),
            None => {
                let after = self.cursor.get();
                let claimed = self.claimed.borrow();
                self.selects
                    .iter()
                    .find(|tokens| tokens.start >= after && !claimed.contains(&tokens.start))
            }
        }?;
        self.claimed.borrow_mut().insert(found.start);
        self.cursor.set(self.cursor.get().max(found.end));
        Some(found)
    }

    fn item(
        &self,
        expression: &Expression,
        item: Option<&Range<usize>>,
        from_first: Option<usize>,
    ) -> ItemFacts {
        if let Expression::Star(_) = expression {
            return ItemFacts::Star {
                item_start: item
                    .filter(|item| !item.is_empty())
                    .map(|item| self.tokens.start(item.start))
                    .or(from_first),
                star_start: item
                    .and_then(|item| self.tokens.star_start(item))
                    .or(from_first),
            };
        }
        let (alias, value) = match expression {
            Expression::Alias(alias) => (Some(alias.alias.name.clone()), &alias.this),
            _ => (None, expression),
        };
        let range = item.map(|item| self.tokens.expression_range(item, alias.is_some()));
        let unwrapped = unwrap_paren(value);
        let cast_type = match unwrapped {
            Expression::Cast(_) | Expression::TryCast(_) | Expression::SafeCast(_) => range
                .as_ref()
                .and_then(|range| self.tokens.cast_type(range)),
            _ => None,
        };
        ItemFacts::Expression(ExpressionItem {
            output_name: alias.or_else(|| column_name(value)),
            direct_projection: column_name(value).is_some(),
            direct_column: column_name(unwrapped),
            cast_type,
            span: range
                .as_ref()
                .filter(|range| !range.is_empty())
                .map(|range| {
                    (
                        self.tokens.start(range.start),
                        self.tokens.end(range.end - 1),
                    )
                }),
            start: range.and_then(|range| self.tokens.leaf_start(&range)),
        })
    }

    fn comparisons(&self, root: &Expression, output_name: Option<&str>) -> Vec<ComparisonFacts> {
        let mut nodes: Vec<(&Expression, ComparisonParts<'_>)> = Vec::new();
        for node in root.dfs() {
            if let Some(parts) = comparison_parts(node) {
                nodes.push((node, parts));
            }
        }
        for (node, parts) in nodes.iter().rev() {
            if anchors(parts) != (None, None) {
                self.span(node, parts);
            }
        }
        let mut output: Vec<ComparisonFacts> = Vec::with_capacity(nodes.len());
        for (node, parts) in &nodes {
            let mut fact = self.comparison(node, parts);
            fact.output_name = output_name.map(str::to_owned);
            output.push(fact);
        }
        output
    }

    fn span(&self, node: &Expression, parts: &ComparisonParts<'_>) -> Option<ComparisonSpan> {
        let key: *const Expression = node;
        if let Some(span) = self.spans.borrow().get(&key) {
            return *span;
        }
        let literals: Vec<&str> = node
            .dfs()
            .filter_map(|value| match value {
                Expression::Literal(literal) => Some(literal.value_str()),
                _ => None,
            })
            .collect();
        let span =
            self.tokens
                .comparison_range(parts.operator, anchors(parts), parts.negated, &literals);
        self.spans.borrow_mut().insert(key, span);
        span
    }

    fn case_comparisons(
        &self,
        root: &Expression,
        output_name: Option<&str>,
    ) -> Vec<ComparisonFacts> {
        let mut output: Vec<ComparisonFacts> = Vec::new();
        for node in root.dfs() {
            if let Expression::Case(case) = node {
                if let Some(operand) = &case.operand {
                    output.extend(self.comparisons(operand, output_name));
                }
                for (condition, _) in &case.whens {
                    output.extend(self.comparisons(condition, output_name));
                }
            }
        }
        output
    }

    fn comparison(&self, node: &Expression, parts: &ComparisonParts<'_>) -> ComparisonFacts {
        let ComparisonParts {
            operator,
            operands,
            negated,
        } = parts;
        let (operator, negated) = (*operator, *negated);
        let span = self.span(node, parts);
        let mut fact = ComparisonFacts {
            span,
            equality: operator == ComparisonOperator::Eq
                || (operator == ComparisonOperator::In && !negated),
            value_list: operator == ComparisonOperator::In,
            binary: !matches!(
                operator,
                ComparisonOperator::Between | ComparisonOperator::In
            ),
            ..ComparisonFacts::default()
        };
        let mut string_tokens: Vec<(usize, usize, &str)> = span.map_or_else(Vec::new, |span| {
            self.tokens.string_tokens(span.start, span.end)
        });
        for value in node.dfs() {
            match value {
                Expression::Column(column) => {
                    fact.columns.insert(column.name.name.clone());
                }
                Expression::Identifier(identifier) => {
                    fact.columns.insert(identifier.name.clone());
                }
                Expression::Literal(literal) => {
                    if let Some(generated) = string_literal(literal) {
                        let position = string_tokens
                            .iter()
                            .position(|(_, _, text)| *text == literal.value_str());
                        let range = position.map(|index| {
                            let (start, end, _) = string_tokens.remove(index);
                            (start, end)
                        });
                        fact.string_literals.push(SourceText { range, generated });
                    }
                }
                _ => {}
            }
        }
        for operand in operands.iter().copied() {
            if let Some(number) = numeric_value(operand) {
                fact.numeric_literals.push(number);
            }
            if direct_column(operand).is_none() {
                fact.modified_columns
                    .extend(operand.dfs().filter_map(direct_column));
            }
            if direct_string_literal(operand).is_none()
                && operand
                    .dfs()
                    .any(|value| direct_string_literal(value).is_some())
            {
                fact.modified_string_literal = true;
            }
        }
        fact
    }
}

fn comparison_parts(node: &Expression) -> Option<ComparisonParts<'_>> {
    let (operator, operands, negated) = match node {
        Expression::Eq(op) => (ComparisonOperator::Eq, vec![&op.left, &op.right], false),
        Expression::Neq(op) => (ComparisonOperator::NotEq, vec![&op.left, &op.right], false),
        Expression::Lt(op) => (ComparisonOperator::Lt, vec![&op.left, &op.right], false),
        Expression::Lte(op) => (ComparisonOperator::LtEq, vec![&op.left, &op.right], false),
        Expression::Gt(op) => (ComparisonOperator::Gt, vec![&op.left, &op.right], false),
        Expression::Gte(op) => (ComparisonOperator::GtEq, vec![&op.left, &op.right], false),
        Expression::Between(between) => (
            ComparisonOperator::Between,
            vec![&between.this, &between.low, &between.high],
            between.not,
        ),
        Expression::In(list) if list.query.is_none() && list.unnest.is_none() => (
            ComparisonOperator::In,
            std::iter::once(&list.this)
                .chain(list.expressions.iter())
                .collect(),
            list.not,
        ),
        _ => return None,
    };
    Some(ComparisonParts {
        operator,
        operands,
        negated,
    })
}

fn anchors(parts: &ComparisonParts<'_>) -> (Option<usize>, Option<usize>) {
    let left_end = span_bounds(parts.operands[0]).map(|(_, end)| end);
    let right_start = parts.operands[1..]
        .iter()
        .filter_map(|operand| span_bounds(operand))
        .map(|(start, _)| start)
        .min();
    (left_end, right_start)
}

fn set_modifiers(with: Option<&With>, order_by: bool, limit: bool) -> Modifiers<'_> {
    Modifiers {
        with,
        order_by,
        limit,
        fetch: false,
        locks: false,
    }
}

fn dependency_calls(root: &Expression) -> Vec<usize> {
    root.dfs()
        .filter_map(|node| match node {
            Expression::Table(table) if table.schema.is_none() && table.catalog.is_none() => {
                stub_call(&table.name.name)
            }
            _ => None,
        })
        .filter(|(kind, _)| *kind != CallKind::Other)
        .map(|(_, index)| index)
        .collect()
}

fn relation(expression: &Expression, tokens: &QueryTokens<'_>) -> RelationFacts {
    match expression {
        Expression::Alias(alias) => {
            let name = Some(alias.alias.name.clone());
            match relation(&alias.this, tokens) {
                RelationFacts::Call { kind, index, .. } => RelationFacts::Call {
                    kind,
                    index,
                    alias: name,
                },
                RelationFacts::Table {
                    display,
                    parts,
                    start,
                    last,
                    ..
                } => RelationFacts::Table {
                    display,
                    parts,
                    start,
                    last,
                    alias: name,
                },
                other => other,
            }
        }
        Expression::Table(table) => table_relation(table, tokens),
        Expression::Pivot(pivot) if !pivot.unpivot => {
            if pivot.fields.iter().any(|field| {
                matches!(field, Expression::In(list)
                    if list.expressions.iter().any(|value| matches!(value, Expression::PivotAny(_))))
            }) {
                RelationFacts::DynamicPivot
            } else {
                RelationFacts::Other
            }
        }
        _ => RelationFacts::Other,
    }
}

fn table_relation(table: &TableRef, tokens: &QueryTokens<'_>) -> RelationFacts {
    let alias = table.alias.as_ref().map(|alias| alias.name.clone());
    if table.schema.is_none() && table.catalog.is_none() {
        if let Some((kind, index)) = stub_call(&table.name.name) {
            return RelationFacts::Call {
                kind,
                index: Some(index),
                alias,
            };
        }
        if table.name.name.starts_with(TABLE_FUNCTION_STUB) {
            return RelationFacts::Call {
                kind: CallKind::Other,
                index: None,
                alias,
            };
        }
    }
    let parts: Vec<&Identifier> = [table.catalog.as_ref(), table.schema.as_ref()]
        .into_iter()
        .flatten()
        .chain(std::iter::once(&table.name))
        .collect();
    RelationFacts::Table {
        display: quoted_parts(&parts, tokens),
        parts: parts.len(),
        start: parts[0].span.map(|span| span.start),
        last: table.name.name.to_ascii_lowercase(),
        alias,
    }
}

fn quoted_parts(parts: &[&Identifier], tokens: &QueryTokens<'_>) -> String {
    parts
        .iter()
        .map(|part| {
            if let Some(text) = part
                .span
                .and_then(|span| tokens.slice(span.start, span.end))
            {
                text.to_owned()
            } else if part.quoted {
                format!("\"{}\"", part.name.replace('"', "\"\""))
            } else {
                part.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn select_anchor(select: &Select, tokens: &QueryTokens<'_>) -> Option<usize> {
    let parts: Vec<&Expression> = select
        .expressions
        .iter()
        .chain(select.from.iter().flat_map(|from| from.expressions.iter()))
        .chain(
            select
                .joins
                .iter()
                .flat_map(|join| std::iter::once(&join.this).chain(join.on.iter())),
        )
        .chain(select.where_clause.iter().map(|clause| &clause.this))
        .chain(
            select
                .group_by
                .iter()
                .flat_map(|group| group.expressions.iter()),
        )
        .chain(select.having.iter().map(|clause| &clause.this))
        .chain(select.qualify.iter().map(|clause| &clause.this))
        .collect();
    for part in &parts {
        if let Some(start) = own_span(part) {
            return Some(start);
        }
    }
    for part in &parts {
        if let Some(open) = span_bounds(part).and_then(|(start, _)| tokens.enclosing_open(start)) {
            return Some(open);
        }
    }
    None
}

fn own_span(expression: &Expression) -> Option<usize> {
    let own = match expression {
        Expression::Select(_)
        | Expression::Union(_)
        | Expression::Intersect(_)
        | Expression::Except(_)
        | Expression::Values(_) => return None,
        Expression::Subquery(subquery) => {
            return subquery
                .alias
                .as_ref()
                .and_then(|alias| alias.span)
                .map(|span| span.start);
        }
        Expression::Column(column) => column.span.or(column.name.span),
        Expression::Identifier(identifier) => identifier.span,
        Expression::Star(star) => star.table.as_ref().and_then(|table| table.span),
        Expression::Function(function) => function.span,
        Expression::Table(table) => table
            .catalog
            .as_ref()
            .or(table.schema.as_ref())
            .unwrap_or(&table.name)
            .span
            .or_else(|| table.alias.as_ref().and_then(|alias| alias.span)),
        Expression::Alias(alias) => {
            return own_span(&alias.this).or(alias.alias.span.map(|span| span.start));
        }
        _ => None,
    };
    own.map(|span| span.start)
        .or_else(|| expression.children().into_iter().find_map(own_span))
}

fn span_bounds(expression: &Expression) -> Option<(usize, usize)> {
    let mut bounds: Option<(usize, usize)> = None;
    for node in expression.dfs() {
        let span = match node {
            Expression::Column(column) => column.span.or(column.name.span),
            Expression::Identifier(identifier) => identifier.span,
            Expression::Star(star) => star.table.as_ref().and_then(|table| table.span),
            Expression::Function(function) => function.span,
            Expression::Count(count) => count.span,
            Expression::Table(table) => table.name.span,
            _ => None,
        };
        if let Some(span) = span {
            bounds = Some(bounds.map_or((span.start, span.end), |(start, end)| {
                (start.min(span.start), end.max(span.end))
            }));
        }
    }
    bounds
}

fn unwrap_paren(mut expression: &Expression) -> &Expression {
    while let Expression::Paren(paren) = expression {
        expression = &paren.this;
    }
    expression
}

fn column_name(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Column(column) => Some(column.name.name.clone()),
        Expression::Identifier(identifier) => Some(identifier.name.clone()),
        _ => None,
    }
}

fn direct_column(expression: &Expression) -> Option<ColumnFact> {
    match unwrap_paren(expression) {
        Expression::Column(column) => Some(ColumnFact {
            name: column.name.name.clone(),
            qualifier: column
                .table
                .as_ref()
                .map(|table| table.name.to_ascii_lowercase()),
        }),
        Expression::Identifier(identifier) => Some(ColumnFact {
            name: identifier.name.clone(),
            qualifier: None,
        }),
        _ => None,
    }
}

fn string_literal(literal: &Literal) -> Option<String> {
    match literal {
        Literal::String(value) => Some(format!("'{}'", value.replace('\'', "''"))),
        Literal::NationalString(value) => Some(format!("N'{}'", value.replace('\'', "''"))),
        Literal::TripleQuotedString(value, quote) => {
            let delimiter = quote.to_string().repeat(TRIPLE_QUOTE_LENGTH);
            Some(format!("{delimiter}{value}{delimiter}"))
        }
        Literal::EscapeString(value) => Some(if value.starts_with(['E', 'e']) {
            value.clone()
        } else {
            format!("E'{value}'")
        }),
        _ => None,
    }
}

fn direct_string_literal(expression: &Expression) -> Option<String> {
    match unwrap_paren(expression) {
        Expression::Literal(literal) => string_literal(literal),
        Expression::UnicodeString(unicode) => direct_string_literal(&unicode.this),
        _ => None,
    }
}

fn numeric_value(expression: &Expression) -> Option<String> {
    match unwrap_paren(expression) {
        Expression::Literal(literal) => match literal.as_ref() {
            Literal::Number(value) => Some(value.clone()),
            _ => None,
        },
        Expression::Neg(negation) => numeric_value(&negation.this).map(|value| format!("-{value}")),
        _ => None,
    }
}
