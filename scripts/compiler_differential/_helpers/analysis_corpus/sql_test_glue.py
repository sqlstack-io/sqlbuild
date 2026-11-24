"""Failure cases owned by the native SQL-test planning and assembly glue lane."""

from scripts.compiler_differential._helpers.corpus.case_builder import failure_case
from scripts.compiler_differential.models import FailureCase

_TEST_PATH: str = "tests/unit/test_customer_totals.sql"
_WINDOWED_PATH: str = "sql/models/marts/windowed_orders.sql"
_WINDOWED_MODEL: str = (
    "MODEL (\n  description 'Orders in the cursor window',\n  materialized incremental,\n"
    "  incremental_strategy delete_insert,\n  cursor order_id,\n  cursor_type integer,\n"
    "  cursor_inputs (raw_orders order_id,),\n);\n\n"
    'SELECT order_id, amount\nFROM __source("raw_orders")\n'
    "WHERE order_id >= __cursor_start() AND order_id < __cursor_end()\n"
)
_ORDERS_FIXTURE: str = "SELECT 1 AS order_id, 10 AS customer_id, 5.0 AS amount, 'placed' AS status"


def _test(*, header: str, ctes: str) -> dict[str, str]:
    return {_TEST_PATH: f"TEST ({header});\n\nWITH\n{ctes}\nSELECT 1\n"}


def sql_test_glue_failure_cases() -> tuple[FailureCase, ...]:
    """Return this lane's failure cases; the lane appends here without editing shared lists."""

    return (
        failure_case(
            name="sql-test-glue-missing-mock",
            expected_code="S000",
            expected_message=(
                "test 'totals_without_source': model 'stg_orders' references "
                '__source("raw_orders") which has no mock'
            ),
            files=_test(
                header='name "totals_without_source"',
                ctes=(
                    "__ref__stg_orders AS (SELECT 10 AS customer_id, 5.0 AS amount),\n"
                    "__expected__stg_orders AS (SELECT 10 AS customer_id, 5.0 AS amount),\n"
                    "__expected__customer_totals AS (\n"
                    "  SELECT 10 AS customer_id, 5.0 AS total_amount\n)"
                ),
            ),
        ),
        failure_case(
            name="sql-test-glue-inverted-window",
            expected_code="P001",
            expected_message=(
                "SQL test 'inverted_window' in 'tests/unit/test_customer_totals.sql': "
                "cursor_start '20' must be before the exclusive cursor_end '10' for model "
                "'windowed_orders'"
            ),
            files={
                _WINDOWED_PATH: _WINDOWED_MODEL,
                **_test(
                    header='name "inverted_window", cursor_start 20, cursor_end 10',
                    ctes=(
                        f"__source__raw_orders AS ({_ORDERS_FIXTURE}),\n"
                        "__expected__windowed_orders AS (SELECT 1 AS order_id, 5.0 AS amount)"
                    ),
                ),
            },
        ),
        failure_case(
            name="sql-test-glue-window-without-cursor-model",
            expected_code="P001",
            expected_message=(
                "SQL test 'plain_window' in 'tests/unit/test_customer_totals.sql' declares "
                "cursor_start or cursor_end, but no model it evaluates uses __cursor_start() or "
                "__cursor_end()"
            ),
            files=_test(
                header='name "plain_window", cursor_start 1',
                ctes=(
                    f"__source__raw_orders AS ({_ORDERS_FIXTURE}),\n"
                    f"__expected__stg_orders AS ({_ORDERS_FIXTURE})"
                ),
            ),
        ),
        failure_case(
            name="sql-test-glue-mock-reads-referencing-helper",
            expected_code="P013",
            expected_message=(
                "SQL test mock '__ref__stg_orders' reads helper CTE 'orders_feed', which calls "
                '__source("raw_orders"); mocks and fixtures are defined before the models the '
                "test runs, so the helper cannot be resolved for them"
            ),
            expected_location=(4, 31),
            files=_test(
                header='name "mock_reads_helper"',
                ctes=(
                    'orders_feed AS (SELECT * FROM __source("raw_orders")),\n'
                    "__ref__stg_orders AS (SELECT * FROM orders_feed),\n"
                    "__expected__customer_totals AS (\n"
                    "  SELECT 10 AS customer_id, 5.0 AS total_amount\n)"
                ),
            ),
        ),
    )
