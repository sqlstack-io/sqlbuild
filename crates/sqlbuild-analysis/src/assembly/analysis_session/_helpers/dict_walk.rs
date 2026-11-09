//! Python's view of a parsed query: the wheel's expression accessors and its dict walk.

use polyglot_sql::expressions::Cte;
use polyglot_sql::traversal::ExpressionWalk;
use polyglot_sql::{ComplexityGuardOptions, DataType, Dialect, Expression, ParseOptions, ast_json};
use serde_json::{Map, Value, json};

use crate::assembly::analysis_session::models::ProjectionFacts;
use crate::relation_markers::constants::MARKER_NAME_PREFIX;

use crate::assembly::analysis_session::constants::{
    ALIAS_AST_KIND, ANNOTATED_AST_KIND, ARRAY_FIXTURE_TYPE, ARRAY_PAYLOAD_KIND, CATALOG_TABLE_ARG,
    COLUMN_AST_KIND, CTE_ALIAS_PAYLOAD_KEY, CTE_LIST_PAYLOAD_KEY, DEPENDENCY_FUNCTION_NAMES,
    FUNCTION_AST_KIND, FUNCTION_FIXTURE_TYPES, FUNCTION_PAYLOAD_KIND, HELPER_READS_DIALECT,
    LITERAL_FIXTURE_TYPES, LITERAL_PAYLOAD_KIND, MAX_FUNCTION_CALL_DEPTH, NAME_PAYLOAD_KEY,
    NULL_PAYLOAD_KIND, RENDERED_TYPE_NAMES, SCHEMA_TABLE_ARG, SELECT_PAYLOAD_KEY,
    SET_OPERATION_PAYLOAD_KEYS, STAR_AST_KIND, STRUCTURED_PAYLOAD_KINDS, TABLE_AST_KIND,
    VARIANT_FIXTURE_TYPE, WILDCARD,
};

/// Python's `bool(value)`.
pub(crate) fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Null) | None => false,
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(items)) => !items.is_empty(),
        Some(Value::Number(number)) => number.as_f64() != Some(0.0),
    }
}

/// The single key of a one-key object, or "" for anything else.
pub(crate) fn node_key(node: Option<&Value>) -> &str {
    let Some(object) = node
        .and_then(Value::as_object)
        .filter(|object| object.len() == 1)
    else {
        return "";
    };
    object.keys().next().map_or("", String::as_str)
}

/// The payload under a node's single key.
pub(crate) fn payload(node: Option<&Value>) -> Option<&Map<String, Value>> {
    let key: &str = node_key(node);
    node.and_then(|node| node.get(key))
        .and_then(Value::as_object)
}

/// Python's `_dict_list`: the objects of a list, or nothing.
pub(crate) fn dict_list(value: Option<&Value>) -> Vec<&Value> {
    match value.and_then(Value::as_array) {
        Some(items) => objects(items),
        None => Vec::new(),
    }
}

fn objects(items: &[Value]) -> Vec<&Value> {
    items.iter().filter(|item| item.is_object()).collect()
}

/// Python's `_nested`: follow object keys.
pub(crate) fn nested<'a>(node: Option<&'a Value>, keys: &[&str]) -> Option<&'a Value> {
    let mut current: Option<&Value> = node;
    for key in keys {
        current = current
            .and_then(Value::as_object)
            .and_then(|object| object.get(*key));
    }
    current
}

/// Python's `_identifier_name`.
pub(crate) fn identifier_name(node: Option<&Value>) -> Option<&str> {
    match node {
        Some(Value::String(name)) => Some(name),
        Some(Value::Object(object)) => object
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty()),
        _ => None,
    }
}

/// Python's `_column_name`.
pub(crate) fn column_name(node: Option<&Value>) -> Option<&str> {
    if node_key(node) != COLUMN_AST_KIND {
        return None;
    }
    identifier_name(payload(node)?.get("name"))
}

/// Python's `_relation_name`: a table, a column or a dependency call's first argument.
pub(crate) fn relation_name(node: Option<&Value>) -> Option<&str> {
    let key: &str = node_key(node);
    let payload: &Map<String, Value> = payload(node)?;
    if key == TABLE_AST_KIND || key == COLUMN_AST_KIND {
        return identifier_name(payload.get("name"))
            .filter(|name| !name.starts_with(MARKER_NAME_PREFIX));
    }
    let called: &str = payload
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if key == FUNCTION_AST_KIND && DEPENDENCY_FUNCTION_NAMES.contains(&casefold(called).as_str()) {
        return column_name(dict_list(payload.get("args")).first().copied());
    }
    None
}

/// Python's `_is_single_wildcard`.
pub(crate) fn is_single_wildcard(value: Option<&Value>) -> bool {
    let expressions: Vec<&Value> = dict_list(value);
    let [only] = expressions.as_slice() else {
        return false;
    };
    if node_key(Some(only)) != STAR_AST_KIND {
        return false;
    }
    let Some(star) = only.get(STAR_AST_KIND).and_then(Value::as_object) else {
        return false;
    };
    !["except", "replace", "rename", "table"]
        .iter()
        .any(|key| truthy(star.get(*key)))
}

/// Python's `_render_type` of a cast target.
pub(crate) fn render_type(node: Option<&Value>) -> Option<String> {
    let node: &Map<String, Value> = node?.as_object()?;
    let raw_type: &str = node
        .get("data_type")
        .and_then(Value::as_str)
        .filter(|raw| !raw.is_empty())?;
    let folded: String = casefold(raw_type);
    let rendered: String = RENDERED_TYPE_NAMES
        .iter()
        .find(|(name, _)| *name == folded)
        .map_or_else(
            || upper(&raw_type.replace('_', " ")),
            |(_, rendered)| (*rendered).to_owned(),
        );
    if let Some(length) = python_int(node.get("length")) {
        return Some(format!("{rendered}({length})"));
    }
    let Some(precision) = python_int(node.get("precision")) else {
        return Some(rendered);
    };
    Some(match python_int(node.get("scale")) {
        Some(scale) => format!("{rendered}({precision}, {scale})"),
        None => format!("{rendered}({precision})"),
    })
}

/// Python's `str(value)` for a value `isinstance(value, int)` accepts.
pub(crate) fn python_int(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Bool(flag) => Some(if *flag { "True" } else { "False" }.to_owned()),
        Value::Number(number) if number.is_i64() || number.is_u64() => Some(number.to_string()),
        _ => None,
    }
}

/// Python's `str.casefold`.
pub(crate) fn casefold(text: &str) -> String {
    str::to_lowercase(text)
}

/// Python's `str.upper`.
pub(crate) fn upper(text: &str) -> String {
    str::to_uppercase(text)
}

/// A value the wheel hands Python for one payload key: an expression, a list of them, or data.
#[derive(Debug, Clone)]
pub(crate) enum PyValue {
    Expr(Expression),
    Exprs(Vec<Expression>),
    Raw(Value),
}

/// The wheel's guarded `parse_one`; Ok(None) is the `PolyglotError` Python catches.
pub(crate) fn parse_one(sql: &str, dialect: &str) -> Result<Option<Expression>, String> {
    match parse_one_or_error(sql, dialect)? {
        Ok(parsed) => Ok(Some(parsed)),
        Err(_polyglot_error) => Ok(None),
    }
}

/// The wheel's guarded `parse_one`, with the message of the `PolyglotError` it raises.
pub(crate) fn parse_one_or_error(
    sql: &str,
    dialect: &str,
) -> Result<Result<Expression, String>, String> {
    let guard: ComplexityGuardOptions =
        serde_json::from_value(json!({"maxFunctionCallDepth": MAX_FUNCTION_CALL_DEPTH}))
            .map_err(|error| error.to_string())?;
    let options: ParseOptions = ParseOptions {
        complexity_guard: Some(guard),
    };
    let parsed: Result<Vec<Expression>, _> = Dialect::get_by_name(dialect)
        .ok_or_else(|| format!("unknown dialect {dialect}"))?
        .parse_with_options(sql, &options);
    let mut expressions: Vec<Expression> = match parsed {
        Ok(expressions) => expressions,
        Err(error) => return Ok(Err(error.to_string())),
    };
    if expressions.len() != 1 {
        return Ok(Err(format!(
            "Expected 1 statement, found {}",
            expressions.len()
        )));
    }
    Ok(expressions.pop().ok_or_else(|| "no statement".to_owned()))
}

/// `str(getattr(node, "kind", ""))`.
pub(crate) fn kind(node: Option<&Expression>) -> &'static str {
    node.map_or("", Expression::variant_name)
}

/// `node.name`, "" for None.
pub(crate) fn name(node: Option<&Expression>) -> &str {
    node.map_or("", Expression::get_name)
}

/// `node.output_name`, "" for None.
pub(crate) fn output_name(node: Option<&Expression>) -> &str {
    node.map_or("", Expression::get_output_name)
}

/// `node.alias_or_name`.
pub(crate) fn alias_or_name(node: &Expression) -> &str {
    let alias: &str = node.get_alias();
    if alias.is_empty() {
        node.get_name()
    } else {
        alias
    }
}

/// `node.is_star`.
pub(crate) fn is_star(node: Option<&Expression>) -> bool {
    match node {
        Some(Expression::Star(_)) => true,
        Some(Expression::Column(column)) => column.name.name == WILDCARD,
        _ => false,
    }
}

/// Python's `_unwrap_polyglot_annotations`.
pub(crate) fn unwrap_annotations(mut node: Option<&Expression>) -> Option<&Expression> {
    while kind(node) == ANNOTATED_AST_KIND {
        node = node.and_then(Expression::get_this);
    }
    node
}

/// What a type assertion asserts: a type, or the type of another expression.
pub(crate) enum Assertion<'a> {
    Fixed(&'a DataType),
    Like(&'a Expression),
}

/// The first type assertion around `node`, past plain annotations, and the expression it types.
pub(crate) fn typed_annotation(
    mut node: Option<&Expression>,
) -> Option<(Assertion<'_>, &Expression)> {
    while let Some(Expression::Annotated(annotated)) = node {
        if let Some(asserted) = &annotated.type_assertion {
            return Some((Assertion::Fixed(asserted), &annotated.this));
        }
        if let Some(like) = &annotated.type_like {
            return Some((Assertion::Like(like), &annotated.this));
        }
        node = Some(&annotated.this);
    }
    None
}

/// A projection's expression: an alias's `this`, otherwise the projection.
pub(crate) fn projected_expression(projection: Option<&Expression>) -> Option<&Expression> {
    if kind(projection) == ALIAS_AST_KIND {
        return projection.and_then(Expression::get_this);
    }
    projection
}

/// `node.this` then `node.expressions`: Python's `_polyglot_expression_args`.
pub(crate) fn expression_args(node: Option<&Expression>) -> Vec<&Expression> {
    let Some(node) = node else {
        return Vec::new();
    };
    node.get_this()
        .into_iter()
        .chain(node.get_expressions())
        .collect()
}

/// `node.children()`.
pub(crate) fn children(node: &Expression) -> Vec<&Expression> {
    ExpressionWalk::children(node)
}

/// Python's `_polyglot_direct_select_tables`: the table children.
pub(crate) fn direct_tables(node: &Expression) -> Vec<&Expression> {
    children(node)
        .into_iter()
        .filter(|child| child.variant_name() == TABLE_AST_KIND)
        .collect()
}

/// `node.find_all(kind)`: descendants in depth-first order, the node itself excluded.
pub(crate) fn find_all<'a>(node: &'a Expression, wanted: &str) -> Vec<&'a Expression> {
    node.dfs()
        .skip(1)
        .filter(|descendant| descendant.variant_name() == wanted)
        .collect()
}

/// `node.to_dict()`.
pub(crate) fn to_dict(node: &Expression) -> Result<Value, String> {
    serde_json::to_value(node).map_err(|error| error.to_string())
}

/// The wheel's `expression_payload`: the tagged value's fields.
pub(crate) fn expression_payload(node: &Expression) -> Result<Map<String, Value>, String> {
    Ok(match to_dict(node)? {
        Value::Object(map) => match map.into_iter().next() {
            Some((_, Value::Object(payload))) => payload,
            _ => Map::new(),
        },
        _ => Map::new(),
    })
}

/// `node.args.get(key)` (and `node.arg(key)` outside its special keys); None for Python's None.
pub(crate) fn arg(node: Option<&Expression>, key: &str) -> Result<Option<PyValue>, String> {
    let Some(node) = node else {
        return Ok(None);
    };
    Ok(expression_payload(node)?.remove(key).and_then(py_value))
}

/// The wheel's `value_to_python_object`; None where Python sees None.
fn py_value(value: Value) -> Option<PyValue> {
    if let Ok(expression) = ast_json::expression_from_value(value.clone()) {
        return Some(PyValue::Expr(expression));
    }
    if let Ok(expressions) = ast_json::expressions_from_value(value.clone()) {
        return Some(PyValue::Exprs(expressions));
    }
    (!value.is_null()).then_some(PyValue::Raw(value))
}

/// Python's `bool()` of a converted value: expressions are always truthy.
pub(crate) fn py_truthy(value: Option<&PyValue>) -> bool {
    match value {
        None => false,
        Some(PyValue::Expr(_)) => true,
        Some(PyValue::Exprs(items)) => !items.is_empty(),
        Some(PyValue::Raw(raw)) => truthy(Some(raw)),
    }
}

/// Python's `_polyglot_name_payload_value` of a converted value.
pub(crate) fn name_payload(value: Option<&PyValue>) -> String {
    match value {
        Some(PyValue::Raw(raw)) => raw_name_payload(Some(raw)),
        _ => String::new(),
    }
}

/// Python's `_polyglot_name_payload_value` of plain data.
pub(crate) fn raw_name_payload(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(object)) => object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    }
}

/// Python's `_polyglot_column_table_name`: `to_dict()["column"]["table"]["name"]`.
pub(crate) fn column_table_name(node: &Expression) -> Result<String, String> {
    let value: Value = to_dict(node)?;
    let table: Option<&Value> = value
        .get("column")
        .filter(|column| column.is_object())
        .and_then(|column| column.get("table"))
        .filter(|table| table.is_object());
    Ok(raw_name_payload(table))
}

/// The top-level CTEs `with_ctes()` returns: name, whether it has column aliases, and its body.
pub(crate) fn top_level_ctes(node: &Expression) -> Vec<(&str, bool, &Expression)> {
    let with = match node {
        Expression::Select(select) => select.with.as_ref(),
        Expression::Union(union) => union.with.as_ref(),
        Expression::Intersect(intersect) => intersect.with.as_ref(),
        Expression::Except(except) => except.with.as_ref(),
        _ => None,
    };
    with.map(|with| with.ctes.iter().map(cte_entry).collect())
        .unwrap_or_default()
}

fn cte_entry(cte: &Cte) -> (&str, bool, &Expression) {
    (cte.alias.name.as_str(), !cte.columns.is_empty(), &cte.this)
}

/// A set operation's `arg("left")`, `arg("right")` and `arg("by_name")`.
pub(crate) fn set_operation(node: &Expression) -> Option<(&Expression, &Expression, bool)> {
    match node {
        Expression::Union(operation) => {
            Some((&operation.left, &operation.right, operation.by_name))
        }
        Expression::Intersect(operation) => {
            Some((&operation.left, &operation.right, operation.by_name))
        }
        Expression::Except(operation) => {
            Some((&operation.left, &operation.right, operation.by_name))
        }
        _ => None,
    }
}

/// A select's `arg("where_clause")` as plain data.
pub(crate) fn where_clause(node: &Expression) -> Result<Option<Value>, String> {
    let Expression::Select(select) = node else {
        return Ok(None);
    };
    select
        .where_clause
        .as_ref()
        .map(|clause| serde_json::to_value(clause).map_err(|error| error.to_string()))
        .transpose()
}

/// Python's `_simple_projection_facts` of the first `SELECT`; empty where the parse fails.
pub(crate) fn fixture_projection_facts(
    sql: &str,
    dialect: Option<&str>,
) -> Result<ProjectionFacts, String> {
    let dialect: &str = dialect.filter(|name| !name.is_empty()).unwrap_or("generic");
    let Some(parsed) = parse_one(sql, dialect)? else {
        return Ok(ProjectionFacts::default());
    };
    let payload: Value = to_dict(&parsed)?;
    let Some(select) = first_select(&payload) else {
        return Ok(ProjectionFacts::default());
    };
    let mut facts: ProjectionFacts = collected_facts(select);
    let top: Option<&Map<String, Value>> = payload.as_object();
    let direct: bool = matches!(
        top.and_then(|map| map.get(SELECT_PAYLOAD_KEY)),
        Some(Value::Object(_))
    );
    let set_operation: bool = top.is_some_and(|map| {
        SET_OPERATION_PAYLOAD_KEYS
            .iter()
            .any(|key| matches!(map.get(*key), Some(Value::Object(_))))
    });
    if !direct && !set_operation {
        facts.null_literal_names.clear();
    }
    if set_operation {
        let branches: Vec<&Map<String, Value>> = branch_selects(&payload);
        let null_in_every_branch = |index: usize| {
            branches
                .iter()
                .all(|branch| null_positions(branch).get(index) == Some(&true))
        };
        let kept: Vec<String> = null_names_by_position(select)
            .into_iter()
            .filter(|(index, _)| null_in_every_branch(*index))
            .map(|(_, name)| name)
            .collect();
        facts.null_literal_names.retain(|name| kept.contains(name));
    }
    Ok(facts)
}

/// Every branch `SELECT` of a set operation, without the selects nested inside them.
fn branch_selects(payload: &Value) -> Vec<&Map<String, Value>> {
    let mut branches: Vec<&Map<String, Value>> = Vec::new();
    let mut pending: Vec<&Value> = vec![payload];
    while let Some(node) = pending.pop() {
        match node {
            Value::Object(map) => {
                if let Some(Value::Object(select)) = map.get(SELECT_PAYLOAD_KEY) {
                    branches.push(select);
                    continue;
                }
                pending.extend(map.values().rev());
            }
            Value::Array(items) => pending.extend(items.iter().rev()),
            _ => {}
        }
    }
    branches
}

/// The projection payloads of a select, each with any alias removed.
fn projected_values(select: &Map<String, Value>) -> Vec<&Map<String, Value>> {
    select
        .get("expressions")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|expression| {
            match expression.get("alias").and_then(|alias| alias.get("this")) {
                Some(Value::Object(value)) => Some(value),
                _ => expression.as_object(),
            }
        })
        .collect()
}

/// Whether each projection of a select is a bare `NULL`.
fn null_positions(select: &Map<String, Value>) -> Vec<bool> {
    projected_values(select)
        .into_iter()
        .map(|value| value.keys().next().map(String::as_str) == Some(NULL_PAYLOAD_KIND))
        .collect()
}

/// The folded alias of each bare `NULL` projection of a select, with its position.
fn null_names_by_position(select: &Map<String, Value>) -> Vec<(usize, String)> {
    let expressions: &[Value] = select
        .get("expressions")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    expressions
        .iter()
        .enumerate()
        .filter_map(|(index, expression)| {
            let alias_payload = expression.get("alias")?;
            let name: &str = alias_payload.get("alias")?.get("name")?.as_str()?;
            let kind: &str = alias_payload.get("this")?.as_object()?.keys().next()?;
            (kind == NULL_PAYLOAD_KIND).then(|| (index, str::to_lowercase(name)))
        })
        .collect()
}

/// Python's recursive `_first_select`, depth first in payload order, without recursion.
fn first_select(payload: &Value) -> Option<&Map<String, Value>> {
    branch_selects(payload).into_iter().next()
}

fn collected_facts(select: &Map<String, Value>) -> ProjectionFacts {
    let mut facts: ProjectionFacts = ProjectionFacts::default();
    let mut quoted: Vec<String> = Vec::new();
    let mut nulls: Vec<String> = Vec::new();
    let expressions: &[Value] = select
        .get("expressions")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    for expression in expressions {
        let Some(Value::Object(alias_payload)) = expression.get("alias") else {
            continue;
        };
        let (Some(Value::Object(alias)), Some(Value::Object(value))) =
            (alias_payload.get("alias"), alias_payload.get("this"))
        else {
            continue;
        };
        let Some(Value::String(name)) = alias.get("name") else {
            continue;
        };
        let folded: String = str::to_lowercase(name);
        if alias.get("quoted") == Some(&Value::Bool(true)) {
            quoted.push(folded.clone());
        }
        let Some((kind, value_payload)) = value.iter().next() else {
            continue;
        };
        if kind == NULL_PAYLOAD_KIND {
            nulls.push(folded);
            continue;
        }
        if let Some(inferred) = simple_type(kind, value_payload) {
            facts.types.push((folded, inferred.to_owned()));
        }
    }
    facts.quoted_names = unique(quoted);
    facts.null_literal_names = unique(nulls);
    facts
}

/// Python's `_simple_expression_type`.
fn simple_type(kind: &str, payload: &Value) -> Option<&'static str> {
    if kind == LITERAL_PAYLOAD_KIND {
        let literal_type: &str = payload.get("literal_type")?.as_str()?;
        return lookup(&LITERAL_FIXTURE_TYPES, literal_type);
    }
    if kind == ARRAY_PAYLOAD_KIND {
        return Some(ARRAY_FIXTURE_TYPE);
    }
    if STRUCTURED_PAYLOAD_KINDS.contains(&kind) {
        return Some(VARIANT_FIXTURE_TYPE);
    }
    if kind == FUNCTION_PAYLOAD_KIND {
        let name: &str = payload
            .as_object()?
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return lookup(&FUNCTION_FIXTURE_TYPES, &str::to_uppercase(name));
    }
    None
}

fn lookup(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, value)| *value)
}

fn unique(names: Vec<String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::with_capacity(names.len());
    for name in names {
        if !kept.contains(&name) {
            kept.push(name);
        }
    }
    kept
}

/// Python's `_parsed_reads`: helper keys `sql` reads, first first; None where `parse_one` raises.
pub(crate) fn helper_parsed_reads(
    sql: &str,
    keys: &[String],
) -> Result<Option<Vec<String>>, String> {
    let folded: String = str::to_lowercase(sql);
    if !keys.iter().any(|key| folded.contains(key.as_str())) {
        return Ok(Some(Vec::new()));
    }
    let Some(parsed) = parse_one(sql, HELPER_READS_DIALECT)? else {
        return Ok(None);
    };
    let nested: Vec<String> = defined_cte_keys(&to_dict(&parsed)?);
    let mut reads: Vec<String> = Vec::new();
    for table in find_all(&parsed, TABLE_AST_KIND) {
        if arg(Some(table), SCHEMA_TABLE_ARG)?.is_some()
            || arg(Some(table), CATALOG_TABLE_ARG)?.is_some()
        {
            continue;
        }
        let key: String = str::to_lowercase(name(Some(table)));
        if keys.contains(&key) && !nested.contains(&key) && !reads.contains(&key) {
            reads.push(key);
        }
    }
    Ok(Some(reads))
}

/// Python's `_defined_cte_keys`: every CTE alias at any depth, casefolded.
fn defined_cte_keys(tree: &Value) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    let mut pending: Vec<&Value> = vec![tree];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(items) => pending.extend(items.iter()),
            Value::Object(map) => {
                if let Some(Value::Array(ctes)) = map.get(CTE_LIST_PAYLOAD_KEY) {
                    keys.extend(ctes.iter().filter_map(|cte| {
                        let alias: &Value = cte.as_object()?.get(CTE_ALIAS_PAYLOAD_KEY)?;
                        Some(str::to_lowercase(
                            alias.as_object()?.get(NAME_PAYLOAD_KEY)?.as_str()?,
                        ))
                    }));
                }
                pending.extend(map.values());
            }
            _ => {}
        }
    }
    keys
}
