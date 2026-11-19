//! Constant string literals that reach an output column, and where the authored file writes them.

use polyglot_sql::expressions::{Cte, Literal, Select, With};
use polyglot_sql::tokens::{Token, TokenType};
use polyglot_sql::{Dialect, DialectType, Expression};

use crate::contracts::models::LiteralLocation;
use crate::parse_guard::main::polyglot_parse_options::polyglot_parse_options;

/// How deep the walk follows set operations and passthrough columns before it stops.
const MAX_DEPTH: usize = 64;

/// The output column a walk follows: by name, or by position in a set-operation branch.
#[derive(Clone, Copy)]
enum Target<'n> {
    Name(&'n str),
    Position(usize),
}

/// The CTEs a query can read, in definition order, and how deep the walk has gone.
#[derive(Clone)]
struct Scope<'a> {
    ctes: Vec<&'a Cte>,
    depth: usize,
}

impl<'a> Scope<'a> {
    /// The scope one step deeper, with `with`'s CTEs visible unless they are recursive.
    fn within(&self, with: Option<&'a With>) -> Self {
        let mut ctes: Vec<&'a Cte> = self.ctes.clone();
        if let Some(with) = with
            && !with.recursive
        {
            ctes.extend(with.ctes.iter());
        }
        Self {
            ctes,
            depth: self.depth + 1,
        }
    }

    /// The scope a CTE body sees: the CTEs defined before it.
    fn before(&self, index: usize) -> Self {
        Self {
            ctes: self.ctes[..index].to_vec(),
            depth: self.depth + 1,
        }
    }
}

/// The literals found so far.
#[derive(Default)]
struct LiteralWalk {
    found: Vec<String>,
}

/// Every constant string literal that fills output column `column` of `query`, in query order.
pub(crate) fn output_literals(query: &Expression, column: &str) -> Vec<String> {
    let mut walk: LiteralWalk = LiteralWalk::default();
    let scope: Scope<'_> = Scope {
        ctes: Vec::new(),
        depth: 0,
    };
    walk.query(query, Target::Name(column), &scope);
    walk.found
}

/// The parsed statement of `sql` under `dialect`, when it is one query.
pub(crate) fn parsed_query(sql: &str, dialect: &str) -> Option<Expression> {
    let Ok(dialect) = dialect.parse::<DialectType>() else {
        return None;
    };
    let Ok(mut statements) =
        polyglot_sql::parse_with_options(sql, dialect, &polyglot_parse_options())
    else {
        return None;
    };
    (statements.len() == 1).then(|| statements.pop()).flatten()
}

/// The first string literal with text `value` in `query`, located in `authored`, the whole file.
pub(crate) fn literal_location(
    (authored, query): (&str, &str),
    value: &str,
    dialect: &str,
) -> Option<LiteralLocation> {
    let dialect: DialectType = dialect
        .parse::<DialectType>()
        .unwrap_or(DialectType::Generic);
    let Ok(tokens) = Dialect::get(dialect).tokenize(query) else {
        return None;
    };
    let matches = |index: &usize| {
        tokens[*index].token_type == TokenType::String && tokens[*index].text == value
    };
    let index: usize = (0..tokens.len())
        .filter(matches)
        .find(|index| select_item(&tokens, *index))
        .or_else(|| (0..tokens.len()).find(matches))?;
    let token = &tokens[index];
    let query_start: usize = authored.find(query)?;
    let offset: usize = authored[..query_start].chars().count();
    let (line, column) = line_column(authored, offset + token.span.start);
    let (end_line, end_column) = line_column(authored, offset + token.span.end);
    Some(LiteralLocation {
        line,
        column,
        end_line,
        end_column,
    })
}

/// Whether the token at `index` stands alone as a select-list item, as a projected literal does.
fn select_item(tokens: &[Token], index: usize) -> bool {
    let opens: bool = index.checked_sub(1).is_some_and(|previous| {
        matches!(
            tokens[previous].token_type,
            TokenType::Select | TokenType::Distinct | TokenType::All | TokenType::Comma
        )
    });
    let closes: bool = tokens.get(index + 1).is_none_or(|next| {
        matches!(
            next.token_type,
            TokenType::As
                | TokenType::Comma
                | TokenType::From
                | TokenType::Union
                | TokenType::Semicolon
                | TokenType::RParen
                | TokenType::Identifier
                | TokenType::QuotedIdentifier
                | TokenType::Var
        )
    });
    opens && closes
}

/// The one-based line and column of character `offset` in `text`.
fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let mut line: usize = 1;
    let mut column: usize = 1;
    for character in text.chars().take(offset) {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// The position of output column `name` in the leftmost branch of `query`, before any star.
fn output_position(query: &Expression, name: &str, depth: usize) -> Option<usize> {
    if depth > MAX_DEPTH {
        return None;
    }
    match query {
        Expression::Select(select) => {
            for (position, projection) in select.expressions.iter().enumerate() {
                match projection {
                    Expression::Star(_) => return None,
                    projection
                        if output_name(projection).is_some_and(|known| same(known, name)) =>
                    {
                        return Some(position);
                    }
                    _ => {}
                }
            }
            None
        }
        Expression::Union(union) => output_position(&union.left, name, depth + 1),
        Expression::Intersect(intersect) => output_position(&intersect.left, name, depth + 1),
        Expression::Subquery(subquery) if subquery.column_aliases.is_empty() => {
            output_position(&subquery.this, name, depth + 1)
        }
        Expression::Paren(paren) => output_position(&paren.this, name, depth + 1),
        _ => None,
    }
}

/// The name a projection outputs, when it names one.
fn output_name(projection: &Expression) -> Option<&str> {
    match projection {
        Expression::Alias(alias) => Some(alias.alias.name.as_str()),
        Expression::Column(column) => Some(column.name.name.as_str()),
        _ => None,
    }
}

fn same(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// The one `FROM` item a select passes every row of through, when `qualifier` names it or is absent.
fn passthrough_source<'a>(select: &'a Select, qualifier: Option<&str>) -> Option<&'a Expression> {
    if !keeps_every_row(select) {
        return None;
    }
    let [source] = select.from.as_ref()?.expressions.as_slice() else {
        return None;
    };
    match qualifier {
        Some(qualifier) if !source_alias(source).is_some_and(|alias| same(alias, qualifier)) => {
            None
        }
        _ => Some(source),
    }
}

/// Whether a select outputs a row for every row it reads: no filter, join, limit or sampling.
fn keeps_every_row(select: &Select) -> bool {
    keeps_any_row(select)
        && select.joins.is_empty()
        && select.lateral_views.is_empty()
        && select.prewhere.is_none()
        && select.where_clause.is_none()
        && select.having.is_none()
        && select.qualify.is_none()
        && select.distinct_on.is_none()
        && select.connect.is_none()
}

/// Whether a select has no row limit or sampling, which could leave a literal with no rows.
fn keeps_any_row(select: &Select) -> bool {
    select.limit.is_none()
        && select.offset.is_none()
        && select.limit_by.is_none()
        && select.fetch.is_none()
        && select.top.is_none()
        && select.sample.is_none()
}

/// The name a `FROM` item is read by: its alias, or a table's own name.
fn source_alias(source: &Expression) -> Option<&str> {
    match source {
        Expression::Table(table) => Some(
            table
                .alias
                .as_ref()
                .map_or(table.name.name.as_str(), |alias| alias.name.as_str()),
        ),
        Expression::Subquery(subquery) => subquery.alias.as_ref().map(|alias| alias.name.as_str()),
        _ => None,
    }
}

impl LiteralWalk {
    fn query<'a>(&mut self, query: &'a Expression, target: Target<'_>, scope: &Scope<'a>) {
        if scope.depth > MAX_DEPTH {
            return;
        }
        match query {
            Expression::Select(select) => {
                self.select(select, target, &scope.within(select.with.as_ref()))
            }
            Expression::Union(union) if union.limit.is_none() && union.offset.is_none() => {
                let scope: Scope<'a> = scope.within(union.with.as_ref());
                let target: Option<Target<'_>> = match target {
                    Target::Name(_) if union.by_name => Some(target),
                    Target::Position(_) if union.by_name => None,
                    Target::Name(name) => {
                        output_position(&union.left, name, 0).map(Target::Position)
                    }
                    Target::Position(_) => Some(target),
                };
                if let Some(target) = target {
                    self.query(&union.left, target, &scope);
                    self.query(&union.right, target, &scope);
                }
            }
            Expression::Subquery(subquery)
                if subquery.column_aliases.is_empty()
                    && subquery.limit.is_none()
                    && subquery.offset.is_none() =>
            {
                self.query(&subquery.this, target, &scope.within(None));
            }
            Expression::Paren(paren) => self.query(&paren.this, target, &scope.within(None)),
            _ => {}
        }
    }

    fn select<'a>(&mut self, select: &'a Select, target: Target<'_>, scope: &Scope<'a>) {
        if !keeps_any_row(select) {
            return;
        }
        match target {
            Target::Position(position) => {
                let starred: bool = select
                    .expressions
                    .iter()
                    .take(position + 1)
                    .any(|projection| matches!(projection, Expression::Star(_)));
                if let Some(projection) = select.expressions.get(position)
                    && !starred
                {
                    self.expression(projection, select, scope);
                }
            }
            Target::Name(name) => {
                let named: Vec<&Expression> = select
                    .expressions
                    .iter()
                    .filter(|projection| {
                        output_name(projection).is_some_and(|known| same(known, name))
                    })
                    .collect();
                if let [projection] = named.as_slice() {
                    self.expression(projection, select, scope);
                    return;
                }
                if !named.is_empty() {
                    return;
                }
                let [Expression::Star(star)] = select.expressions.as_slice() else {
                    return;
                };
                if star.except.is_some() || star.replace.is_some() || star.rename.is_some() {
                    return;
                }
                let qualifier: Option<&str> = star.table.as_ref().map(|table| table.name.as_str());
                if let Some(source) = passthrough_source(select, qualifier) {
                    self.source(source, name, scope);
                }
            }
        }
    }

    fn expression<'a>(
        &mut self,
        expression: &'a Expression,
        select: &'a Select,
        scope: &Scope<'a>,
    ) {
        match expression {
            Expression::Alias(alias) => self.expression(&alias.this, select, scope),
            Expression::Paren(paren) => self.expression(&paren.this, select, scope),
            Expression::Literal(literal) => {
                if let Literal::String(value) = literal.as_ref() {
                    self.found.push(value.clone());
                }
            }
            Expression::Column(column) => {
                let qualifier: Option<&str> =
                    column.table.as_ref().map(|table| table.name.as_str());
                if let Some(source) = passthrough_source(select, qualifier) {
                    self.source(source, &column.name.name, scope);
                }
            }
            _ => {}
        }
    }

    /// The literals filling column `name` of a CTE or derived table that a `FROM` item reads.
    fn source<'a>(&mut self, source: &'a Expression, name: &str, scope: &Scope<'a>) {
        match source {
            Expression::Table(table) if table.schema.is_none() && table.catalog.is_none() => {
                let Some(index) = scope
                    .ctes
                    .iter()
                    .rposition(|cte| same(&cte.alias.name, &table.name.name))
                else {
                    return;
                };
                let cte: &Cte = scope.ctes[index];
                if table.column_aliases.is_empty() && cte.columns.is_empty() {
                    self.query(&cte.this, Target::Name(name), &scope.before(index));
                }
            }
            Expression::Subquery(subquery)
                if subquery.column_aliases.is_empty() && !subquery.lateral =>
            {
                self.query(&subquery.this, Target::Name(name), &scope.within(None));
            }
            _ => {}
        }
    }
}
