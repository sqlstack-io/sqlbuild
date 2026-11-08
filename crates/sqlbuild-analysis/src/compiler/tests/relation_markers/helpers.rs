//! The relation-marker walk checked against the original JSON-tree walk, its oracle.

use serde_json::Value;

use crate::compiler::_helpers::sql_tests::relation_markers::named_marker_calls;
use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;

type MarkerCalls = Option<Vec<(String, String)>>;

/// The original JSON-tree walk, kept as the oracle the direct relation walk must match.
fn json_relation_marker_calls(value: &Value) -> Vec<(String, String)> {
    let from_markers = ["from", "from_clause"]
        .into_iter()
        .filter_map(|key| value.get(key))
        .filter_map(|from| from.get("expressions"))
        .filter_map(Value::as_array)
        .flatten()
        .chain(value.get("left").filter(|_| value.get("joins").is_some()))
        .filter_map(json_marker);
    let join_markers = ["joins", "from_joins"]
        .into_iter()
        .filter_map(|key| value.get(key))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|join| join.get("this"))
        .filter_map(json_marker);
    let children: Vec<&Value> = value
        .as_object()
        .map(|object| object.values().collect())
        .or_else(|| value.as_array().map(|values| values.iter().collect()))
        .unwrap_or_default();
    from_markers
        .chain(join_markers)
        .chain(children.into_iter().flat_map(json_relation_marker_calls))
        .collect()
}

fn json_marker(expression: &Value) -> Option<(String, String)> {
    let object = expression.as_object()?;
    object.get("alias").map_or_else(
        || json_function_marker(object),
        |alias| alias.get("this").and_then(json_marker),
    )
}

fn json_function_marker(object: &serde_json::Map<String, Value>) -> Option<(String, String)> {
    let function = object.get("function")?.as_object()?;
    let function_name = function.get("name")?.as_str()?.to_ascii_lowercase();
    let names: Vec<String> = function
        .get("args")?
        .as_array()?
        .iter()
        .map(|arg| {
            arg.get("column")?
                .get("name")?
                .get("name")?
                .as_str()
                .map(str::to_string)
        })
        .collect::<Option<_>>()?;
    let referenced_name = (function_name == "__dbt_ref" && names.len() == 2)
        .then(|| names.join("__"))
        .or_else(|| (names.len() == 1).then(|| names[0].clone()))?;
    Some((function_name, referenced_name))
}

/// Owned marker calls for comparison with collected calls.
pub(crate) fn owned_marker_calls(calls: &[(&str, &str)]) -> MarkerCalls {
    Some(
        calls
            .iter()
            .map(|(function, name)| ((*function).to_string(), (*name).to_string()))
            .collect(),
    )
}

/// Parse one statement and return the direct walk's and the JSON oracle's marker calls.
pub(crate) fn relation_markers_and_json_oracle(
    dialect: &str,
    sql: &str,
) -> (MarkerCalls, MarkerCalls) {
    let mut statements = sql_test_parser(Some(dialect))
        .expect("test dialect must exist")
        .parse(sql)
        .expect("test SQL must parse")
        .statements;
    assert_eq!(statements.len(), 1, "{sql}");
    let expression = statements.remove(0);
    (
        Some(named_marker_calls(&expression, &[])),
        serde_json::to_value(&expression)
            .ok()
            .map(|value| json_relation_marker_calls(&value)),
    )
}

/// Return every generated relation shape whose collected markers differ from the JSON oracle.
pub(crate) fn relation_marker_oracle_mismatches(dialect: &str) -> Vec<String> {
    let relations: &[&str] = &[
        "__ref(\"orders\")",
        "__source(\"raw_orders\") AS s",
        "__dbt_ref(\"shop\", \"customers\") c",
        "(SELECT * FROM __seed(\"regions\") JOIN __ref(\"stores\") ON TRUE) AS d",
        "LATERAL (SELECT * FROM __ref(\"items\")) AS l",
        "__udf(\"fn_orders\")",
        "plain_table",
    ];
    let wrappers = [
        "SELECT * FROM {a} JOIN {b} ON TRUE",
        "WITH x AS (SELECT * FROM {a}) SELECT (SELECT 1 FROM {b}) AS v FROM x",
        "SELECT * FROM {a} WHERE EXISTS (SELECT 1 FROM {b})",
        "SELECT * FROM {a} UNION SELECT * FROM {b}",
        "SELECT * FROM {a}, {b}",
        "SELECT * FROM ({a} JOIN {b} ON TRUE)",
        "SELECT * FROM {a} AS x LEFT JOIN LATERAL (SELECT * FROM {b}) AS y ON TRUE",
        "SELECT * FROM (SELECT * FROM {a}) AS x CROSS JOIN {b} WHERE x.id IN (SELECT id FROM {a})",
        "UPDATE orders SET amount = 1 FROM {a} JOIN {b} ON TRUE",
        "DELETE FROM orders USING {a} JOIN {b} ON TRUE",
        "INSERT INTO orders SELECT * FROM {a} JOIN {b} ON TRUE",
    ];
    let parser = sql_test_parser(Some(dialect)).expect("test dialect must exist");
    wrappers
        .iter()
        .flat_map(|wrapper| {
            relations.iter().flat_map(move |first| {
                relations
                    .iter()
                    .map(move |second| wrapper.replace("{a}", first).replace("{b}", second))
            })
        })
        .filter(|sql| {
            parser
                .parse(sql)
                .map(|parsed| parsed.statements)
                .unwrap_or_default()
                .iter()
                .any(|expression| {
                    Some(named_marker_calls(expression, &[]))
                        != serde_json::to_value(expression)
                            .ok()
                            .map(|value| json_relation_marker_calls(&value))
                })
        })
        .collect()
}
