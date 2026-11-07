"""Render the BigQuery adapter methods that take structured arguments, every bool combination.

Appends {"id", "sql"} lines to the given file and prints what it could not render.
"""

import itertools
import json
import re
import sys

from sqlbuild._native import SqlLogicalType, SqlValue, SqlValueKind
from sqlbuild.adapter.contract import models as contract
from sqlbuild.adapter.contract.types import FrameworkType, LoaderLogicalType
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild._native import FunctionArgument
from sqlbuild.compiler.compile.types import FunctionLanguage

STATEMENT = re.compile(r"^\s*(\(\s*)*(SELECT|WITH|CREATE|INSERT|MERGE|UPDATE|DELETE|DROP|ALTER|TRUNCATE|DECLARE|BEGIN|GRANT|REVOKE)\b", re.I)
DEST = "`example-project.analytics.orders_snapshot`"
ORIGIN = "`example-project.analytics.orders`"
adapter = BigQueryAdapter.__new__(BigQueryAdapter)
adapter._location = "US"
out = open(sys.argv[1], "a")
count = 0
failed = []


def emit(label, result):
    global count
    if result is None:
        return
    items = result if isinstance(result, (tuple, list)) else (result,)
    for index, item in enumerate(items):
        for text in [getattr(item, "sql", item), *getattr(item, "statements", ())]:
            if isinstance(text, str) and STATEMENT.match(text):
                out.write(json.dumps({"id": f"adapter/{label}/{index}", "sql": text}) + "\n")
                count += 1


def sweep(method, bools=(), **kwargs):
    for combo in itertools.product((False, True), repeat=len(bools)):
        args = dict(kwargs, **dict(zip(bools, combo)))
        label = method + "".join(f"/{b}={int(v)}" for b, v in zip(bools, combo))
        try:
            emit(label, getattr(adapter, method)(**args))
        except Exception as error:  # noqa: BLE001 - scratch survey
            failed.append(f"{label}: {type(error).__name__}: {str(error)[:120]}")


key = ("id",)
cols = ("id", "label", "updated_at")
snap = dict(valid_from_column="valid_from", valid_to_column="valid_to")
for table_type in ("TABLE", "table"):
    for strategy in ("timestamp", "check", None):
        sweep("render_create_initial_snapshot_destination", table_type=table_type, destination=DEST, origin=ORIGIN,
              snapshot_strategy=strategy, updated_at_column="updated_at", observed_at_column="observed_at",
              initial_valid_from="TIMESTAMP '2024-01-01'", **snap)
    sweep("render_create_initial_historical_timestamp_snapshot_destination", ("invalidate_hard_deletes",),
          table_type=table_type, destination=DEST, origin=ORIGIN, unique_key=key, updated_at_column="updated_at",
          observed_at_column="observed_at", output_columns=cols, **snap)
    sweep("render_create_initial_historical_timestamp_changes_destination", table_type=table_type, destination=DEST,
          origin=ORIGIN, unique_key=key, updated_at_column="updated_at", output_columns=cols, **snap)
    sweep("render_create_initial_historical_check_snapshot_destination", ("invalidate_hard_deletes",),
          table_type=table_type, destination=DEST, origin=ORIGIN, unique_key=key, check_columns=("label",),
          observed_at_column="observed_at", output_columns=cols, **snap)
for observed in ("observed_at", None):
    for initial in ("TIMESTAMP '2024-01-01'", None):
        sweep("render_apply_timestamp_snapshot_changes", ("invalidate_hard_deletes",), destination=DEST, origin=ORIGIN,
              unique_key=key, updated_at_column="updated_at", observed_at_column=observed, initial_valid_from=initial,
              output_columns=cols, **snap)
        for updated in ("updated_at", None):
            sweep("render_apply_check_snapshot_changes", ("invalidate_hard_deletes",),
                  target=contract.SnapshotChangeTarget(destination=DEST, origin=ORIGIN, unique_key=key, output_columns=cols, **snap),
                  check_columns=("label",), updated_at_column=updated, observed_at_column=observed, initial_valid_from=initial)
sweep("render_apply_historical_timestamp_snapshot_changes", ("invalidate_hard_deletes",), destination=DEST, origin=ORIGIN,
      unique_key=key, updated_at_column="updated_at", observed_at_column="observed_at", output_columns=cols, **snap)
sweep("render_apply_historical_timestamp_changes", destination=DEST, origin=ORIGIN, unique_key=key,
      updated_at_column="updated_at", output_columns=cols, **snap)
sweep("render_apply_historical_check_snapshot_changes", ("invalidate_hard_deletes",), destination=DEST, origin=ORIGIN,
      unique_key=key, check_columns=("label",), observed_at_column="observed_at", output_columns=cols, **snap)
for unique in (("id",), ("id", "label")):
    sweep("render_apply_check_snapshot_changes", ("invalidate_hard_deletes",),
          target=contract.SnapshotChangeTarget(destination=DEST, origin=ORIGIN, unique_key=unique, output_columns=cols, **snap),
          check_columns=("label", "updated_at"), updated_at_column=None, observed_at_column=None, initial_valid_from=None)

columns = (contract.ColumnInfo(name="note", type="STRING"), contract.ColumnInfo(name="amount", type="NUMERIC(10, 2)"))
sweep("render_add_columns", destination=ORIGIN, columns=columns)
sweep("render_alter_column_types", destination=ORIGIN, columns=columns)
sweep("render_drop_columns", destination=ORIGIN, column_names=("note", "amount"))
for scope in contract.RetentionScope if hasattr(contract, "RetentionScope") else ():
    pass
try:
    from sqlbuild.adapter.contract.types import RetentionScope
    for scope in RetentionScope:
        request = contract.RetentionRequest(request_id="r1", scope=scope, database="example-project", schema="analytics", desired_days=3, name="orders")
        sweep("render_retention_changes", request=request, state=None)
except Exception as error:  # noqa: BLE001
    failed.append(f"retention setup: {error}")
for logical in LoaderLogicalType:
    sweep("render_loader_logical_type", type_name=logical)
    sweep("render_loader_value_literal", value={"BOOLEAN": True, "INTEGER": 3, "FLOAT": 1.5, "STRING": "it's", "TIMESTAMP": "2024-01-01T00:00:00Z", "DATE": "2024-01-01", "JSON": {"a": [1, "b"]}}[logical.name], logical_type=logical)
sweep("render_loader_value_literal", value=None, logical_type=None)
sweep("render_loader_rows_select", rows=({"id": 1, "label": "a", "payload": {"k": 1}}, {"id": 2, "label": None, "payload": None}),
      column_names=("id", "label", "payload"), column_sql_types={"id": "INT64", "label": "STRING", "payload": "JSON"},
      inferred_types={"id": LoaderLogicalType.INTEGER, "label": LoaderLogicalType.STRING, "payload": LoaderLogicalType.JSON})
sweep("render_loader_rows_select", rows=(), column_names=("id",), column_sql_types={"id": "INT64"}, inferred_types={"id": LoaderLogicalType.INTEGER})
for framework in FrameworkType:
    sweep("render_framework_type", type_name=framework)
for value in (SqlValue.__new__(SqlValue) if False else None,):
    pass
def typed(kind, raw, element=None):
    return SqlValue(SqlLogicalType(SqlValueKind(kind), element), raw)


samples = [
    ("string", typed("string", "it's")), ("integer", typed("integer", 3)), ("boolean", typed("boolean", True)),
    ("float", typed("float", 1.5)), ("decimal", typed("decimal", "12.50")), ("null", typed("null", None)),
]
try:
    samples.append(("list", typed("list", [typed("integer", 1), typed("integer", 2)], SqlLogicalType(SqlValueKind("integer")))))
    samples.append(("object", typed("object", {"a": typed("string", "x"), "b": typed("integer", 2)})))
except Exception as error:  # noqa: BLE001
    failed.append(f"compound SqlValue: {error}")
for label, value in samples:
    for name in ("render_typed_scalar", "render_typed_value_list", "render_typed_array", "render_typed_object"):
        try:
            text = getattr(adapter, name)(value=value)
            emit(f"{name}/{label}", f"SELECT {text} AS v")
        except Exception as error:  # noqa: BLE001
            failed.append(f"{name}/{label}: {type(error).__name__}: {str(error)[:100]}")

arguments = (FunctionArgument(name="p_amount", type="FLOAT64"), FunctionArgument(name="p_factor", type="INT64"))
table_columns = (FunctionArgument(name="id", type="INT64"), FunctionArgument(name="band", type="STRING"))
for language, body, returns, return_columns in (
    (FunctionLanguage.SQL, "p_amount * p_factor", "FLOAT64", ()),
    (FunctionLanguage.SQL, "SELECT 1 AS id, 'low' AS band WHERE p_factor > 0", "TABLE", table_columns),
    (FunctionLanguage.PYTHON, "def scaled(p_amount, p_factor):\n    return p_amount * p_factor\n", "FLOAT64", ()),
):
    for args in (arguments, ()):
        sweep("render_create_function", destination="`example-project.analytics.scaled`", arguments=args, returns=returns,
              body_sql=body, return_columns=return_columns, language=language, runtime_version="python-3.11" if language is FunctionLanguage.PYTHON else None,
              entry_point="scaled" if language is FunctionLanguage.PYTHON else None, packages=())
        try:
            definition = contract.FunctionDefinition(destination="example-project.analytics.scaled", arguments=args, returns=returns, body_sql=body,
                                                     return_columns=return_columns, language=language, runtime_version=None, entry_point=None, packages=(), source_file_path=None)
            sweep("render_create_temporary_function", name="__sqlbuild_tmp_scaled", definition=definition)
        except Exception as error:  # noqa: BLE001
            failed.append(f"FunctionDefinition: {error}")
grants = tuple(
    contract.RelationGrant(privilege=privilege, grantee=grantee, column=None, grantable=False, denied=False, grantee_kind=kind)
    for privilege, grantee, kind in (("roles/bigquery.dataViewer", "user:a@example.com", "user"), ("SELECT", "group:readers@example.com", "group"))
)
sweep("render_relation_grants", grants=grants, destination=ORIGIN, columns=("id", "label"))
sweep("render_relation_revokes", grants=grants, destination=ORIGIN)
print("rendered", count, "statements;", len(failed), "calls failed")
for line in failed:
    print("  ", line)
