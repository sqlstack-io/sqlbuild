"""Failure cases owned by the native FAST lineage facts lane."""

from scripts.compiler_differential._helpers.corpus.case_builder import failure_case
from scripts.compiler_differential.constants import FAILURE_MART_PATH
from scripts.compiler_differential.models import FailureCase

_ORDERS: str = '__ref("stg_orders")'
_RAW: str = '__source("raw_orders")'
_OPTED_OUT_HEADER: str = (
    'MODEL (\n  description "Order totals per customer",\n  sql_analysis false,\n);\n\n'
)
_BROKEN_PATH: str = "sql/models/marts/broken_orders.sql"
_BROKEN_MODEL: str = (
    'MODEL (\n  description "Orders with an unknown qualifier",\n);\n\n'
    f"SELECT x.order_id\nFROM {_ORDERS} o\n"
)


def _opted_out_mart(body: str) -> dict[str, str]:
    """An opted-out mart whose lineage comes from the parse fallback, beside a failing model."""

    return {FAILURE_MART_PATH: _OPTED_OUT_HEADER + body, _BROKEN_PATH: _BROKEN_MODEL}


def lineage_failure_cases() -> tuple[FailureCase, ...]:
    """Return this lane's failure cases; the lane appends here without editing shared lists."""

    return (
        failure_case(
            name="lineage-opted-out-star",
            expected_code="B004",
            files=_opted_out_mart(f"SELECT o.order_id AS first_order, o.*\nFROM {_ORDERS} o\n"),
        ),
        failure_case(
            name="lineage-opted-out-union-of-stars",
            expected_code="B004",
            files=_opted_out_mart(
                f"SELECT *, 1 AS copy_number\nFROM {_ORDERS}\n"
                f'UNION ALL\nSELECT "R".*, 2 FROM {_RAW} AS "R" JOIN {_ORDERS} s ON TRUE\n'
            ),
        ),
        failure_case(
            name="lineage-opted-out-except-and-ctes",
            expected_code="B004",
            files=_opted_out_mart(
                f"WITH kept AS (SELECT customer_id FROM {_ORDERS}),\n"
                "again AS (SELECT * FROM kept)\n"
                "SELECT customer_id, COUNT(*) AS total_amount FROM again GROUP BY 1\n"
                f"EXCEPT\nSELECT customer_id, amount FROM {_ORDERS}\n"
            ),
        ),
        failure_case(
            name="lineage-opted-out-quoted-columns",
            expected_code="B004",
            files=_opted_out_mart(
                'SELECT "Customer_ID", SUM(O.Amount) AS "Total_Amount",\n'
                "  CAST(MAX(o.status) AS VARCHAR) AS last_status, 'fixed' AS label\n"
                f"FROM {_ORDERS} AS O\nGROUP BY 1\n"
            ),
        ),
    )
