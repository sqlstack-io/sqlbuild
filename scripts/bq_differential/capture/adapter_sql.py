"""Call every BigQuery adapter render_* method that takes plain arguments, with placeholders.

Writes adapter_sql.jsonl ({"id", "sql"} per rendered statement) and prints the methods it could not
call, so the gaps in adapter coverage are listed rather than silent.
"""

import enum
import inspect
import json
import re
import sys
import typing

from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter

STATEMENT = re.compile(r"^\s*(\(\s*)*(SELECT|WITH|CREATE|INSERT|MERGE|UPDATE|DELETE|DROP|ALTER|TRUNCATE|DECLARE|BEGIN|GRANT|REVOKE)\b", re.I)
RELATION = "example-project.analytics.orders"
VALUES = {
    "database": "example-project",
    "schema": "analytics",
    "sql": "SELECT 1 AS id, 'a' AS label, CURRENT_TIMESTAMP() AS updated_at",
    "columns": ("id", "label", "updated_at"),
    "cast_column_names": ("id",),
    "unique_key": ("id",),
    "key_columns": ("id",),
    "check_columns": ("label",),
    "cursor_column": "updated_at",
    "updated_at": "updated_at",
    "updated_at_column": "updated_at",
    "cursor_start": "2024-01-01 00:00:00",
    "cursor_end": "2024-02-01 00:00:00",
    "cursor_end_exclusive": "2024-02-01 00:00:00",
    "maximum_allowed": "2024-02-01 00:00:00",
    "cursor_type": "TIMESTAMP",
    "expression": "amount",
    "target_type": "NUMERIC",
    "alias": "amount",
    "where_sql": " WHERE id > 0",
    "projections": ("CAST(id AS INT64) AS id",),
    "cast_projections": ("CAST(id AS INT64) AS id",),
    "retain_versions": 3,
    "value": "2024-01-01",
    "call_suffix_sql": "(1, 2)",
    "body_sql": "x + 1",
    "returns": "INT64",
    "column": "updated_at",
    "name": "id",
    "col_name": "id",
    "old_name": "label",
    "new_name": "label_text",
    "join_condition": "__target.id = __source.id",
    "update_assignments": "label = __source.label",
    "insert_columns": "id, label",
    "insert_values": "__source.id, __source.label",
}
RELATION_NAMES = re.compile(r"destination|origin|target|relation|left|right|stage|staged|table_name|source")


def value_for(parameter):
    name = parameter.name
    if name in VALUES:
        return VALUES[name]
    annotation = str(parameter.annotation)
    if "bool" in annotation:
        return name != "is_date"
    if RELATION_NAMES.search(name) and "str" in annotation:
        return RELATION
    if parameter.default is not inspect.Parameter.empty:
        return parameter.default
    raise LookupError(f"{name}: {annotation}")


adapter = BigQueryAdapter.__new__(BigQueryAdapter)
adapter._location = "US"
out = open(sys.argv[1], "w")
failed = []
seen = set()
count = 0
for cls in BigQueryAdapter.__mro__:
    for method_name, function in vars(cls).items():
        if not method_name.startswith("render_") or method_name in seen or not callable(function):
            continue
        seen.add(method_name)
        parameters = list(inspect.signature(function).parameters.values())[1:]
        try:
            kwargs = {p.name: value_for(p) for p in parameters if p.kind is p.KEYWORD_ONLY}
            args = [value_for(p) for p in parameters if p.kind is p.POSITIONAL_OR_KEYWORD]
            result = getattr(adapter, method_name)(*args, **kwargs)
        except Exception as error:  # noqa: BLE001 - scratch survey
            failed.append(f"{method_name}: {type(error).__name__}: {str(error)[:100]}")
            continue
        texts = result if isinstance(result, (tuple, list)) else (result,)
        for index, text in enumerate(texts):
            text = getattr(text, "sql", text)
            if isinstance(text, str) and STATEMENT.match(text):
                out.write(json.dumps({"id": f"adapter/{method_name}/{index}", "sql": text}) + "\n")
                count += 1
print("rendered", count, "statements;", len(failed), "methods not called")
for line in failed:
    print("  ", line)
