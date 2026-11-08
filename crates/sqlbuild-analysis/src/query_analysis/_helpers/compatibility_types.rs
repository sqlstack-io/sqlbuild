//! Borrowed-tree type evaluation with immutable lexical CTE outputs.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use polyglot_sql::expressions::{DataType, Select, Star, Values, With};
use polyglot_sql::{Dialect, DialectType, Expression, ValidationSchema};

type Outputs = Vec<(String, Option<String>)>;
type Relations = HashMap<String, Rc<Outputs>>;
#[derive(Default)]
struct Sources {
    items: Vec<(String, Rc<Outputs>)>,
    using_columns: HashSet<String>,
}
pub(super) const NULL_TYPE: &str = "\0NULL";
const SQL_WILDCARD: &str = "*";

#[cfg(test)]
pub(super) fn infer(
    expression: &Expression,
    schema: Option<&ValidationSchema>,
    functions: &HashMap<String, String>,
    dialect: DialectType,
) -> HashMap<String, String> {
    let mut types: HashMap<String, String> = HashMap::new();
    for (name, data_type) in infer_outputs(expression, schema, functions, dialect) {
        if let Some(value) = data_type.filter(|value| value != NULL_TYPE) {
            types.insert(name, value);
        }
    }
    types
}

pub(super) fn infer_outputs(
    expression: &Expression,
    schema: Option<&ValidationSchema>,
    functions: &HashMap<String, String>,
    dialect: DialectType,
) -> Outputs {
    evaluate_outputs(expression, schema, functions, dialect).0
}

/// Return outputs only when evaluation never consults mutable AST type annotations.
pub(super) fn infer_unannotated_outputs(
    expression: &Expression,
    schema: Option<&ValidationSchema>,
    functions: &HashMap<String, String>,
    dialect: DialectType,
) -> Option<Outputs> {
    let (outputs, used_annotations) = evaluate_outputs(expression, schema, functions, dialect);
    (!used_annotations).then_some(outputs)
}

fn evaluate_outputs(
    expression: &Expression,
    schema: Option<&ValidationSchema>,
    functions: &HashMap<String, String>,
    dialect: DialectType,
) -> (Outputs, bool) {
    let mut relations = Relations::new();
    if let Some(schema) = schema {
        for table in &schema.tables {
            relations.insert(table.name.to_lowercase(), Rc::new(table_outputs(table)));
        }
    }
    let context = TypeContext {
        functions,
        dialect,
        used_annotations: Cell::new(false),
    };
    let outputs = context.query(expression, &relations);
    (outputs, context.used_annotations.get())
}

/// A schema table's columns with their declared types.
fn table_outputs(table: &polyglot_sql::SchemaTable) -> Outputs {
    table
        .columns
        .iter()
        .map(|column| (column.name.clone(), Some(column.data_type.clone())))
        .collect()
}

struct TypeContext<'a> {
    functions: &'a HashMap<String, String>,
    dialect: DialectType,
    used_annotations: Cell<bool>,
}

impl TypeContext<'_> {
    fn query(&self, expression: &Expression, inherited: &Relations) -> Outputs {
        let expression = unwrapped(expression);
        let mut relations = inherited.clone();
        if let Some(with) = query_with(expression) {
            for cte in &with.ctes {
                let mut outputs = self.query(&cte.this, &relations);
                for ((name, _), alias) in outputs.iter_mut().zip(&cte.columns) {
                    *name = alias.name.clone();
                }
                relations.insert(cte.alias.name.to_lowercase(), Rc::new(outputs));
            }
        }
        match expression {
            Expression::Select(select) => self.select(select, &relations),
            Expression::Values(values) => self.values_outputs(values),
            Expression::Union(set) => self.set(&set.left, &set.right, set.by_name, &relations),
            Expression::Intersect(set) => self.set(&set.left, &set.right, set.by_name, &relations),
            Expression::Except(set) => self.set(&set.left, &set.right, set.by_name, &relations),
            Expression::Subquery(subquery) => self.query(&subquery.this, &relations),
            _ => Vec::new(),
        }
    }

    fn set(
        &self,
        left: &Expression,
        right: &Expression,
        by_name: bool,
        relations: &Relations,
    ) -> Outputs {
        let left = self.query(left, relations);
        let right = self.query(right, relations);
        if by_name {
            let mut result: Outputs = Vec::new();
            for (name, value) in &left {
                let other = lookup(&right, name).unwrap_or(Some(NULL_TYPE));
                result.push((name.clone(), common_type(value.as_deref(), other)));
            }
            for (name, value) in right {
                if lookup(&left, &name).is_none() {
                    result.push((name, value));
                }
            }
            return result;
        }
        if left.len() != right.len() {
            return Vec::new();
        }
        left.into_iter()
            .zip(right)
            .map(|((name, left), (_, right))| {
                (name, common_type(left.as_deref(), right.as_deref()))
            })
            .collect()
    }

    fn select(&self, select: &Select, relations: &Relations) -> Outputs {
        let mut aliases = Sources {
            using_columns: select
                .joins
                .iter()
                .flat_map(|join| &join.using)
                .map(|column| column.name.to_lowercase())
                .collect(),
            ..Default::default()
        };
        let tables = select
            .from
            .iter()
            .flat_map(|from| &from.expressions)
            .chain(select.joins.iter().map(|join| &join.this));
        for source in tables {
            if let Expression::Values(values) = unwrapped(source) {
                aliases.items.push((
                    values
                        .alias
                        .as_ref()
                        .map_or(String::new(), |alias| alias.name.to_lowercase()),
                    Rc::new(self.values_outputs(values)),
                ));
            }
            if let Expression::Subquery(subquery) = unwrapped(source)
                && let Some(alias) = &subquery.alias
            {
                let mut outputs = self.query(&subquery.this, relations);
                for ((name, _), alias) in outputs.iter_mut().zip(&subquery.column_aliases) {
                    *name = alias.name.clone();
                }
                aliases
                    .items
                    .push((alias.name.to_lowercase(), Rc::new(outputs)));
            }
            if let Expression::Table(table) = unwrapped(source)
                && let Some(outputs) = relations.get(&table.name.name.to_lowercase())
            {
                let outputs = if table.column_aliases.is_empty() {
                    outputs.clone()
                } else {
                    let mut renamed = outputs.as_ref().clone();
                    for ((name, _), alias) in renamed.iter_mut().zip(&table.column_aliases) {
                        *name = alias.name.clone();
                    }
                    Rc::new(renamed)
                };
                aliases.items.push((
                    table
                        .alias
                        .as_ref()
                        .unwrap_or(&table.name)
                        .name
                        .to_lowercase(),
                    outputs,
                ));
            }
        }
        let mut result: Outputs = Vec::new();
        for projection in &select.expressions {
            let projection = unwrapped(projection);
            if let Some(star) = super::borrowed_facts::projection_star(projection) {
                result.extend(star_outputs(&star, &aliases, self));
            } else {
                let name = projection.get_output_name();
                if name != SQL_WILDCARD {
                    let inner = match projection {
                        Expression::Alias(alias) => &alias.this,
                        _ => projection,
                    };
                    let value = self.projection_type(inner, &aliases);
                    result.push((name.to_string(), value));
                }
            }
        }
        result
    }

    fn values_outputs(&self, values: &Values) -> Outputs {
        let mut outputs: Outputs = Vec::new();
        for row in &values.expressions {
            for (index, expression) in row.expressions.iter().enumerate() {
                let value = self.projection_type(expression, &Sources::default());
                if let Some((_, prior)) = outputs.get_mut(index) {
                    *prior = common_type(prior.as_deref(), value.as_deref());
                } else {
                    let name = values.column_aliases.get(index).map_or_else(
                        || format!("column{}", index + 1),
                        |alias| alias.name.clone(),
                    );
                    outputs.push((name, value));
                }
            }
        }
        outputs
    }

    fn projection_type(&self, expression: &Expression, aliases: &Sources) -> Option<String> {
        if asserted_expression(expression).is_none()
            && unresolved_like(expression).is_none()
            && matches!(unwrapped(expression), Expression::Null(_))
        {
            Some(NULL_TYPE.to_string())
        } else {
            self.expression(expression, aliases).or_else(|| {
                self.used_annotations.set(true);
                expression
                    .inferred_type()
                    .filter(|data_type| **data_type != DataType::Unknown)
                    .and_then(|data_type| self.cast_type(data_type))
            })
        }
    }

    fn expression(&self, expression: &Expression, aliases: &Sources) -> Option<String> {
        if let Some((like, inner)) = unresolved_like(expression) {
            let known = |data_type: &String| !data_type.to_ascii_uppercase().contains("UNKNOWN");
            return self
                .expression(inner, aliases)
                .filter(|inferred| known(inferred) && inferred != NULL_TYPE)
                .or_else(|| self.expression(like, aliases).filter(known));
        }
        if let Some((asserted, inner)) = asserted_expression_type(expression) {
            if matches!(unwrapped(inner), Expression::Null(_)) {
                return self.cast_type(asserted);
            }
            return self
                .expression(inner, aliases)
                .filter(|inferred| !inferred.to_ascii_uppercase().contains("UNKNOWN"));
        }
        let expression = unwrapped(expression);
        let function_name = match expression {
            Expression::Function(function) => function.name.as_str(),
            Expression::AggregateFunction(function) => function.name.as_str(),
            _ => expression.variant_name(),
        };
        if !matches!(expression, Expression::Column(_))
            && let Some(value) = self.functions.get(&function_name.to_uppercase())
        {
            return Some(value.clone());
        }
        match expression {
            Expression::Alias(alias) => self.expression(&alias.this, aliases),
            Expression::Cast(cast) | Expression::TryCast(cast) => self.cast_type(&cast.to),
            Expression::Column(column) => {
                if let Some(table) = &column.table {
                    let key = table.name.to_lowercase();
                    let (_, source) = aliases.items.iter().find(|(name, _)| name == &key)?;
                    return lookup(source, &column.name.name)
                        .flatten()
                        .map(str::to_string);
                }
                let mut values = aliases
                    .items
                    .iter()
                    .filter_map(|(_, source)| lookup(source, &column.name.name));
                let value = values.next()??;
                if aliases
                    .using_columns
                    .contains(&column.name.name.to_lowercase())
                {
                    for other in values {
                        if canonical(other?) != canonical(value) {
                            return None;
                        }
                    }
                    Some(value.to_string())
                } else {
                    values.next().is_none().then(|| value.to_string())
                }
            }
            Expression::Boolean(_)
            | Expression::And(_)
            | Expression::Between(_)
            | Expression::Eq(_)
            | Expression::Exists(_)
            | Expression::Gt(_)
            | Expression::Gte(_)
            | Expression::ILike(_)
            | Expression::In(_)
            | Expression::Is(_)
            | Expression::IsNull(_)
            | Expression::Like(_)
            | Expression::Lt(_)
            | Expression::Lte(_)
            | Expression::Neq(_)
            | Expression::Not(_)
            | Expression::Or(_)
            | Expression::RegexpLike(_) => Some("BOOLEAN".to_string()),
            Expression::Min(function) | Expression::Max(function) => {
                self.expression(&function.this, aliases)
            }
            Expression::WindowFunction(function) => self.expression(&function.this, aliases),
            Expression::WithinGroup(function) => self.expression(&function.this, aliases),
            Expression::Case(case) => self.results(
                case.whens
                    .iter()
                    .map(|(_, result)| result)
                    .chain(case.else_.iter()),
                aliases,
            ),
            Expression::IfFunc(function) => self.results(
                std::iter::once(&function.true_value).chain(function.false_value.iter()),
                aliases,
            ),
            Expression::Coalesce(function) => self.results(function.expressions.iter(), aliases),
            Expression::Concat(binary) => {
                let left = self.string_operand(&binary.left, aliases)?;
                let right = self.string_operand(&binary.right, aliases)?;
                (is_text(&left) && is_text(&right)).then(|| "TEXT".to_string())
            }
            _ => None,
        }
    }

    fn string_operand(&self, expression: &Expression, aliases: &Sources) -> Option<String> {
        if matches!(unwrapped(expression), Expression::Literal(literal) if matches!(literal.as_ref(), polyglot_sql::expressions::Literal::String(_)))
        {
            Some("TEXT".to_string())
        } else {
            self.expression(expression, aliases)
        }
    }

    fn results<'a>(
        &self,
        values: impl Iterator<Item = &'a Expression>,
        aliases: &Sources,
    ) -> Option<String> {
        let mut types = values
            .filter(|value| !matches!(unwrapped(value), Expression::Null(_)))
            .map(|value| self.expression(value, aliases));
        let first = types.next()??;
        types
            .all(|value| {
                value
                    .as_deref()
                    .is_some_and(|value| canonical(value) == canonical(&first))
            })
            .then_some(first)
    }

    fn cast_type(&self, data_type: &DataType) -> Option<String> {
        if let DataType::VarChar {
            length: Some(length),
            ..
        }
        | DataType::String {
            length: Some(length),
        }
        | DataType::TextWithLength { length } = data_type
        {
            return Some(format!("VARCHAR({length})"));
        }
        match Dialect::get(self.dialect).generate(&Expression::DataType(data_type.clone())) {
            Ok(rendered) => Some(rendered.replace(", ", ",")),
            Err(_) => None,
        }
    }
}

fn query_with(expression: &Expression) -> Option<&With> {
    match expression {
        Expression::Select(query) => query.with.as_ref(),
        Expression::Union(query) => query.with.as_ref(),
        Expression::Intersect(query) => query.with.as_ref(),
        Expression::Except(query) => query.with.as_ref(),
        _ => None,
    }
}

/// The expression a type assertion around `expression` types, past parentheses and comments.
fn asserted_expression(expression: &Expression) -> Option<&Expression> {
    asserted_expression_type(expression).map(|(_, inner)| inner)
}

/// The type assertion around `expression`, past parentheses and comments, and what it types.
fn asserted_expression_type(mut expression: &Expression) -> Option<(&DataType, &Expression)> {
    loop {
        expression = match expression {
            Expression::Annotated(annotation) if annotation.type_assertion.is_some() => {
                return annotation
                    .type_assertion
                    .as_ref()
                    .map(|asserted| (asserted, &annotation.this));
            }
            Expression::Annotated(annotation) => &annotation.this,
            Expression::Paren(paren) => &paren.this,
            _ => return None,
        };
    }
}

/// The like and typed expressions of an unresolved typed-like assertion wrapping `expression`.
fn unresolved_like(mut expression: &Expression) -> Option<(&Expression, &Expression)> {
    loop {
        expression = match expression {
            Expression::Annotated(annotation) if annotation.type_assertion.is_some() => {
                return None;
            }
            Expression::Annotated(annotation) => match &annotation.type_like {
                Some(like) => return Some((like, &annotation.this)),
                None => &annotation.this,
            },
            Expression::Paren(paren) => &paren.this,
            _ => return None,
        };
    }
}

fn unwrapped(mut expression: &Expression) -> &Expression {
    loop {
        expression = match expression {
            Expression::Annotated(annotation) => &annotation.this,
            Expression::Paren(paren) => &paren.this,
            _ => return expression,
        };
    }
}

fn lookup<'a>(outputs: &'a Outputs, name: &str) -> Option<Option<&'a str>> {
    outputs
        .iter()
        .find(|(key, _)| key == name)
        .or_else(|| {
            outputs
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
        })
        .map(|(_, value)| value.as_deref())
}

fn star_outputs(star: &Star, aliases: &Sources, context: &TypeContext<'_>) -> Outputs {
    let excluded: HashSet<_> = star
        .except
        .iter()
        .flatten()
        .map(|name| name.name.to_lowercase())
        .collect();
    let mut result: Outputs = Vec::new();
    for (alias, source) in &aliases.items {
        if star
            .table
            .as_ref()
            .is_some_and(|table| !table.name.eq_ignore_ascii_case(alias))
        {
            continue;
        }
        for (name, value) in source.iter() {
            let key = name.to_lowercase();
            if excluded.contains(&key) {
                continue;
            }
            let replacement = star
                .replace
                .iter()
                .flatten()
                .find(|replacement| replacement.alias.name.eq_ignore_ascii_case(name));
            let value = replacement.map_or_else(
                || value.clone(),
                |replacement| context.projection_type(&replacement.this, aliases),
            );
            let renamed = star
                .rename
                .iter()
                .flatten()
                .find(|(original, _)| original.name.eq_ignore_ascii_case(name))
                .map_or(name.as_str(), |(_, renamed)| renamed.name.as_str());
            result.push((renamed.to_string(), value));
        }
    }
    result
}

fn canonical(value: &str) -> String {
    let compact: String = value
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .flat_map(char::to_uppercase)
        .collect();
    match compact.as_str() {
        "INTEGER" => "INT".to_string(),
        "VARCHAR" | "STRING" => "TEXT".to_string(),
        _ => compact,
    }
}

fn is_text(value: &str) -> bool {
    matches!(value, "TEXT" | "STRING") || value.starts_with("VARCHAR")
}

fn common_type(left: Option<&str>, right: Option<&str>) -> Option<String> {
    match (left, right) {
        (Some("\0NULL"), Some("\0NULL")) => None,
        (Some("\0NULL"), right) => right.map(str::to_string),
        (left, Some("\0NULL")) => left.map(str::to_string),
        (Some(left), Some(right)) if canonical(left) == canonical(right) => Some(left.to_string()),
        _ => None,
    }
}
