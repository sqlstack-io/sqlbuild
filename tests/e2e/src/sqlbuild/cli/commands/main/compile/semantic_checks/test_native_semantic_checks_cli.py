"""Semantic diagnostics are completed natively with the expected CLI output."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.semantic_checks._test_types import (
    NativeSemanticChecksCliTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.semantic_checks.helpers import (
    SemanticRun,
    diagnostic_codes,
    diagnostic_notes,
    semantic_run,
)

_CONFIG: str = (
    'name = "orders_semantics"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_SOURCES: str = (
    "sources:\n  - name: raw_orders\n    description: Orders feed.\n    expression: >-\n"
    "      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount,\n"
    "      'placed' AS status, TIMESTAMP '2026-01-01 00:00:00' AS ordered_at)\n"
    "    columns:\n      - name: order_id\n        type: INTEGER\n"
    "      - name: customer_id\n        type: INTEGER\n      - name: amount\n        type: DOUBLE\n"
    "      - name: status\n        type: VARCHAR\n      - name: ordered_at\n        type: TIMESTAMP\n"
)
_FAILING_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _CONFIG + "\n[settings]\nrequire_sql_analysis = true\n",
    "sql/sources/raw.yml": _SOURCES,
    "sql/models/staging/stg_orders.sql": (
        'MODEL (description "Staged orders");\n\n'
        "SELECT order_id, customer_id, amonut, status + 1 AS status_rank, ordered_at\n"
        'FROM __source("raw_orders")\n'
    ),
    "sql/models/marts/customer_totals.sql": (
        'MODEL (description "Totals per customer");\n\n'
        "SELECT o.customer_id, SUM(o.amount) AS total_amount, MAX(o.status_rank) AS top_rank\n"
        'FROM __ref("stg_orders") AS o\nWHERE o.ordered_at > 5\nGROUP BY o.customer_id\n'
    ),
    "sql/models/marts/customer_labels.sql": (
        'MODEL (description "Labels per customer", sql_analysis false);\n\n'
        "SELECT customer_id, UPPER(labell) AS label\n"
        'FROM __ref("stg_orders")\n'
    ),
}
_METADATA_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _CONFIG,
    "sql/sources/raw.yml": _SOURCES
    + (
        "  - name: raw_events\n    description: Loaded events.\n    managed: true\n"
        "    write_strategy: append\n    contract: enforced\n    cursor_column: loaded_at\n"
        "    columns:\n      - name: event_id\n        type: INTEGER\n"
        "      - name: load_seq\n        type: INTEGER\n"
    ),
    "python/nodes/loaders/raw_events.py": (
        "from sqlbuild.loaders import loader\n\n\n@loader\n"
        "def raw_events(ctx):\n    '''Load events.'''\n"
        "    return [{'event_id': 1, 'load_seq': 1}]\n"
    ),
    "sql/functions/sql/scaled_amount.sql": (
        'FUNCTION (\n  description "Scale an amount",\n'
        "  arguments (p_amount DOUBLE, p_factor INTEGER),\n  returns DOUBLE,\n);\n\n"
        "p_amount * p_factor\n"
    ),
    "sql/models/staging/stg_orders.sql": (
        'MODEL (description "Staged orders");\n\n'
        "SELECT order_id, customer_id, amount, status, ordered_at\n"
        'FROM __source("raw_orders")\n'
    ),
    "sql/models/marts/customer_totals.sql": (
        'MODEL (\n  description "Totals per customer",\n'
        "  materialized incremental,\n  incremental_strategy delete_insert,\n"
        "  unique_key [customer_key],\n  cursor ordered_at,\n  cursor_type integer,\n"
        "  cursor_inputs (\n    stg_orders placed_at,\n  ),\n);\n\n"
        "SELECT o.customer_id, MAX(o.ordered_at) AS ordered_at,\n"
        '  SUM(__udf("scaled_amount")(o.status, 2)) AS scaled_total,\n'
        '  MAX(__udf("scaled_amount")(o.amount)) AS top_amount\n'
        'FROM __ref("stg_orders") AS o\nGROUP BY o.customer_id\n'
    ),
    "tests/unit/test_stg_orders.sql": (
        "TEST();\n\nWITH\n__source__raw_orders AS (\n"
        "  SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount,"
        " 'placed' AS status, TIMESTAMP '2026-01-01' AS ordered_at\n),\n"
        "__expected__stg_orders AS (\n  SELECT 1 AS order_id, 2 AS discount\n)\nSELECT 1\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        NativeSemanticChecksCliTestCase(
            description="a recovered typo, a poisoned type, a comparison and a rejected opt-out",
            files=_FAILING_FILES,
            expected_codes=("B217", "B002", "B212", "P009"),
            expected_notes=(
                "1 downstream uses of stg_orders.amount were not checked because of this error",
                "1 downstream output uses were not type-checked because of this error",
                "o.ordered_at is TIMESTAMP, 5 is INTEGER",
                "raw_orders has: amount, status, ordered_at, customer_id, order_id",
            ),
        ),
        NativeSemanticChecksCliTestCase(
            description="UDF calls, config and cursor references, a source cursor and a SQL test",
            files=_METADATA_FILES,
            expected_codes=("B301", "B102", "B300", "B300", "B301", "B300", "B302"),
            expected_notes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_project_when_compiling_then_diagnostics_are_reported(
    test_case: NativeSemanticChecksCliTestCase, tmp_path: Path
) -> None:
    run: SemanticRun = semantic_run(project_dir=tmp_path / "orders", files=test_case.files)

    assert run.returncode == 1
    assert diagnostic_codes(run.report) == test_case.expected_codes
    assert set(test_case.expected_notes) <= diagnostic_notes(run.report)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
