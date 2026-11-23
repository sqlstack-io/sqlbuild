"""E2E coverage for macro-mode SQL tests and `__macro__` mocks of SQL macros."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroCommandTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    ALL_MACROS,
    orders_model,
    output,
    run_project,
)

_EXPRESSION_TEST: str = (
    'TEST (mode macro, name "cents_to_dollars_rounds");\n\nWITH\n'
    "input_values AS (SELECT CAST(1250 AS BIGINT) AS amount_cents),\n"
    "__macro_actual__ AS (\n"
    "  SELECT @sql.cents_to_dollars(amount_cents) AS dollars FROM input_values\n),\n"
    "__macro_expected__ AS (SELECT CAST(12.5 AS DECIMAL(18, 2)) AS dollars)\n"
)
_COLUMN_LIST_TEST: str = (
    'TEST (mode macro, name "audit_columns_flag_updates");\n\nWITH\n'
    "input_rows AS (\n  SELECT TIMESTAMP '2026-01-01' AS created_at, "
    "TIMESTAMP '2026-01-02' AS updated_at\n),\n"
    "__macro_actual__ AS (SELECT @sql.audit_columns(input_rows) FROM input_rows),\n"
    "__macro_expected__ AS (\n  SELECT TIMESTAMP '2026-01-01' AS created_at, "
    "TIMESTAMP '2026-01-02' AS updated_at, TRUE AS was_updated\n)\n"
)
_CTE_LIST_TEST: str = (
    'TEST (mode macro, name "clean_orders_keeps_latest_live");\n\nWITH\n'
    "input_orders AS (\n"
    "  SELECT 1 AS order_id, TIMESTAMP '2026-01-01' AS updated_at, FALSE AS is_deleted\n"
    "  UNION ALL SELECT 1, TIMESTAMP '2026-01-02', FALSE\n"
    "  UNION ALL SELECT 2, TIMESTAMP '2026-01-01', TRUE\n),\n"
    "@sql.clean_orders(input_orders)\n"
    "__macro_actual__ AS (SELECT order_id, updated_at FROM latest),\n"
    "__macro_expected__ AS (SELECT 1 AS order_id, TIMESTAMP '2026-01-02' AS updated_at)\n"
)
_ORDERS_CLEAN: dict[str, str] = {
    "sql/models/marts/orders_clean.sql": (
        'MODEL (description "Clean orders");\n\n'
        'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\n'
        "@sql.clean_orders(orders_raw)\n"
        "final AS (SELECT o.order_id, @sql.net_amount(o.amount_cents, o.fee_cents) AS net "
        "FROM latest AS o)\nSELECT * FROM final\n"
    )
}
_CALLER: dict[str, str] = orders_model(
    'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\n'
    "@sql.clean_orders(orders_raw)\n"
    "final AS (SELECT o.order_id, @sql.cents_to_dollars(o.amount_cents) AS amount, "
    "@sql.audit_columns(o) FROM latest AS o)\nSELECT * FROM final\n"
)
_MOCK_TEST: str = (
    'TEST (name "orders_clean_with_mocked_net");\n\nWITH\n'
    "__macro__net_amount AS (SELECT '0'),\n"
    "__ref__stg_orders AS (\n"
    "  SELECT 1 AS order_id, 'open' AS status, 1250 AS amount_cents, 100 AS fee_cents,\n"
    "    20 AS tax_cents, TIMESTAMP '2026-01-01' AS created_at,\n"
    "    TIMESTAMP '2026-01-02' AS updated_at, FALSE AS is_deleted, 'GB' AS country_code\n),\n"
    '__assert__net_is_mocked AS (SELECT * FROM __ref("orders_clean") WHERE net <> 0)\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCommandTestCase(
            description="macro_mode_tests_of_each_form_pass",
            files={
                **ALL_MACROS,
                **_CALLER,
                "tests/unit/test_cents.sql": _EXPRESSION_TEST,
                "tests/unit/test_audit_columns.sql": _COLUMN_LIST_TEST,
                "tests/unit/test_clean_orders.sql": _CTE_LIST_TEST,
            },
            command=("test",),
            expected_returncode=0,
            expected_fragments=(
                "PASS=3",
                "cents_to_dollars_rounds",
                "audit_columns_flag_updates",
                "clean_orders_keeps_latest_live",
            ),
        ),
        SqlMacroCommandTestCase(
            description="failing_expectation_of_a_cte_macro_fails",
            files={
                **ALL_MACROS,
                **_CALLER,
                "tests/unit/test_clean_orders.sql": _CTE_LIST_TEST.replace(
                    "TIMESTAMP '2026-01-02' AS updated_at)",
                    "TIMESTAMP '2026-01-01' AS updated_at)",
                ),
            },
            command=("test",),
            expected_returncode=1,
            expected_fragments=("FAIL=1",),
        ),
        SqlMacroCommandTestCase(
            description="mock_of_a_sql_macro_replaces_the_model_call",
            files={**ALL_MACROS, **_ORDERS_CLEAN, "tests/unit/test_orders_clean.sql": _MOCK_TEST},
            command=("test",),
            expected_returncode=0,
            expected_fragments=("PASS=1",),
        ),
        SqlMacroCommandTestCase(
            description="mock_of_a_column_list_call_without_commas_keeps_the_separators",
            files={
                **ALL_MACROS,
                "sql/models/marts/orders_audited.sql": (
                    'MODEL (description "Audited orders");\n\n'
                    "SELECT o.order_id @sql.audit_columns(o) o.status "
                    'FROM __ref("stg_orders") AS o\n'
                ),
                "tests/unit/test_orders_audited.sql": (
                    'TEST (name "orders_audited_with_mocked_columns");\n\nWITH\n'
                    "__macro__audit_columns AS (SELECT 'NULL AS created_at, NULL AS updated_at, "
                    "TRUE AS was_updated'),\n"
                    "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status),\n"
                    "__expected__orders_audited AS (SELECT 1 AS order_id, NULL AS created_at, "
                    "NULL AS updated_at, TRUE AS was_updated, 'open' AS status)\n"
                ),
            },
            command=("test",),
            expected_returncode=0,
            expected_fragments=("PASS=1",),
        ),
        SqlMacroCommandTestCase(
            description="cte_macro_in_a_model_test_with_list_is_rejected",
            files={
                **ALL_MACROS,
                **orders_model('SELECT order_id FROM __ref("stg_orders")\n'),
                "tests/unit/test_orders.sql": (
                    'TEST (name "orders_keep_ids");\n\nWITH\n'
                    "__ref__stg_orders AS (SELECT 1 AS order_id, "
                    "TIMESTAMP '2026-01-01' AS updated_at, FALSE AS is_deleted),\n"
                    "@sql.clean_orders(__ref__stg_orders)\n"
                    "__expected__orders AS (SELECT 1 AS order_id)\n"
                ),
            },
            command=("compile",),
            expected_returncode=1,
            expected_fragments=(
                "calls CTE-list SQL macro @sql.clean_orders() in its WITH list; only a mode "
                "macro test may call one there",
            ),
        ),
        SqlMacroCommandTestCase(
            description="unknown_cte_macro_in_a_macro_test_is_rejected",
            files={
                **ALL_MACROS,
                "tests/unit/test_clean_orders.sql": _CTE_LIST_TEST.replace(
                    "@sql.clean_orders(", "@sql.clean_orderz("
                ),
            },
            command=("compile",),
            expected_returncode=1,
            expected_fragments=("references unknown macro '@sql.clean_orderz'",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_tests_of_sql_macros_when_running_then_each_outcome_is_reported(
    test_case: SqlMacroCommandTestCase, tmp_path: Path
) -> None:
    _, result = run_project(tmp_path=tmp_path, files=test_case.files, command=test_case.command)
    printed: str = output(result)

    assert result.returncode == test_case.expected_returncode, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed
