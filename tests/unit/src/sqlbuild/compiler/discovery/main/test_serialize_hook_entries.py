from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import PythonHookEntry, SqlHookEntry, serialize_hook_entries
from tests.unit.src.sqlbuild.compiler.discovery.main._test_types import (
    SerializeHookEntriesTestCase,
)

SQL_HOOK_OUTPUT_FIELDS: tuple[str, ...] = (
    "statement",
    "name",
    "relative_path",
    "definition_sql",
    "kwargs",
    "description",
)

_NAMED_SQL_HOOK: SqlHookEntry = SqlHookEntry(
    statement="INSERT INTO order_audit SELECT 'orders'",
    name="record_order_access",
    relative_path=Path("sql/hooks/sql/record_order_access.sql"),
    definition_sql="INSERT INTO order_audit SELECT {{ table_name }}",
    kwargs={"table_name": "'orders'"},
    description="Record order table access",
)


@pytest.mark.parametrize(
    "test_case",
    [
        SerializeHookEntriesTestCase(
            description="serializes every output field of a named SQL hook in order",
            value=[_NAMED_SQL_HOOK],
            sql_fields=SQL_HOOK_OUTPUT_FIELDS,
            python_hook_fields={},
            expected_hooks=[
                {
                    "type": "sql",
                    "statement": "INSERT INTO order_audit SELECT 'orders'",
                    "name": "record_order_access",
                    "relative_path": "sql/hooks/sql/record_order_access.sql",
                    "definition_sql": "INSERT INTO order_audit SELECT {{ table_name }}",
                    "kwargs": {"table_name": "'orders'"},
                    "description": "Record order table access",
                }
            ],
        ),
        SerializeHookEntriesTestCase(
            description="omits unset fields of an inline SQL hook",
            value=(SqlHookEntry(statement="ANALYZE orders"),),
            sql_fields=SQL_HOOK_OUTPUT_FIELDS,
            python_hook_fields={},
            expected_hooks=[{"type": "sql", "statement": "ANALYZE orders"}],
        ),
        SerializeHookEntriesTestCase(
            description="keeps only allowlisted SQL hook fields",
            value=[_NAMED_SQL_HOOK],
            sql_fields=("statement", "kwargs"),
            python_hook_fields={},
            expected_hooks=[
                {
                    "type": "sql",
                    "statement": "INSERT INTO order_audit SELECT 'orders'",
                    "kwargs": {"table_name": "'orders'"},
                }
            ],
        ),
        SerializeHookEntriesTestCase(
            description="adds per-name fields to Python hooks and keeps hook order",
            value=[
                PythonHookEntry(name="notify_orders", kwargs={"channel": "orders"}),
                SqlHookEntry(statement="ANALYZE orders"),
                PythonHookEntry(name="refresh_customers", kwargs={}),
            ],
            sql_fields=SQL_HOOK_OUTPUT_FIELDS,
            python_hook_fields={"notify_orders": {"version_hash": "notify-hash"}},
            expected_hooks=[
                {
                    "type": "python",
                    "name": "notify_orders",
                    "kwargs": {"channel": "orders"},
                    "version_hash": "notify-hash",
                },
                {"type": "sql", "statement": "ANALYZE orders"},
                {"type": "python", "name": "refresh_customers", "kwargs": {}},
            ],
        ),
        SerializeHookEntriesTestCase(
            description="returns no hooks for a non-sequence value",
            value="ANALYZE orders",
            sql_fields=SQL_HOOK_OUTPUT_FIELDS,
            python_hook_fields={},
            expected_hooks=[],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hook_entries_when_serializing_then_returns_allowlisted_payloads(
    test_case: SerializeHookEntriesTestCase,
) -> None:
    hooks: list[dict[str, object]] = serialize_hook_entries(
        value=test_case.value,
        sql_fields=test_case.sql_fields,
        python_hook_fields=test_case.python_hook_fields,
    )

    assert hooks == test_case.expected_hooks
    assert [list(hook) for hook in hooks] == [list(hook) for hook in test_case.expected_hooks]
