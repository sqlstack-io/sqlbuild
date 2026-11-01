"""Failure cases owned by the native model analysis session lane."""

from scripts.compiler_differential._helpers.corpus.case_builder import failure_case
from scripts.compiler_differential.constants import (
    FAILURE_BASE_CONFIG,
    FAILURE_CONFIG_PATH,
    FAILURE_MART_PATH,
    FAILURE_STAGING_PATH,
)
from scripts.compiler_differential.models import FailureCase

_ORDERS: str = '__ref("stg_orders")'
_MART_HEADER: str = 'MODEL (\n  description "Order totals per customer",\n);\n\n'
_STAGING_HEADER: str = 'MODEL (\n  description "Staged orders",\n);\n\n'
_LAYERED_PATH: str = "models/marts/order_layers.sql"
_LAYERED_MODEL: str = (
    'MODEL (\n  description "Orders layered over the staged star",\n);\n\n'
    f"SELECT s.*, s.amount * 2 AS doubled_amount\nFROM {_ORDERS} s\n"
)

_PIVOT_PATH: str = "models/marts/status_amounts.sql"


def _pivot_model(*, pivot_column: str, body: str) -> str:
    return (
        'MODEL (\n  description "Order amounts pivoted by status",\n  contract enforced,\n'
        "  materialized table,\n  columns (customer_id (type INTEGER)),\n"
        f"  dynamic_columns (\n    status_amounts (\n      pivot_column {pivot_column},\n"
        "      value_column amount,\n      aggregate MAX,\n      type DOUBLE\n    )\n  ),\n);\n\n"
        f"{body}"
    )


def _contract_header(columns: str) -> str:
    return (
        'MODEL (\n  description "Order totals per customer",\n  contract enforced,\n'
        f"  columns (\n{columns}  ),\n);\n\n"
    )


def analysis_session_failure_cases() -> tuple[FailureCase, ...]:
    """Return this lane's failure cases; the lane appends here without editing shared lists."""

    return (
        failure_case(
            name="analysis-session-star-published-unknown-column",
            expected_code="B002",
            files={
                FAILURE_STAGING_PATH: _STAGING_HEADER + 'SELECT *\nFROM __source("raw_orders")\n',
                _LAYERED_PATH: _LAYERED_MODEL,
                FAILURE_MART_PATH: _MART_HEADER
                + "SELECT customer_id, SUM(missing_amount) AS total_amount\n"
                'FROM __ref("order_layers")\nGROUP BY customer_id\n',
            },
        ),
        failure_case(
            name="analysis-session-set-operation-unknown-column",
            expected_code="B002",
            files={
                FAILURE_MART_PATH: _MART_HEADER
                + f"SELECT customer_id, amount AS total_amount FROM {_ORDERS}\n"
                f"UNION ALL\nSELECT customer_id, refund_amount FROM {_ORDERS}\n",
            },
        ),
        failure_case(
            name="analysis-session-cte-chain-unknown-column",
            expected_code="B002",
            files={
                FAILURE_MART_PATH: _MART_HEADER + f"WITH base AS (SELECT * FROM {_ORDERS}),\n"
                "totals AS (SELECT customer_id, SUM(amount) AS total_amount FROM base GROUP BY 1)\n"
                "SELECT customer_id, total_amount, order_count FROM totals\n",
            },
        ),
        failure_case(
            name="analysis-session-enriched-subquery-type-mismatch",
            expected_code="K002",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + '\n[[rules.rule_ignores]]\nrules = ["SQLCLEAN039"]\n'
                f'paths = ["{FAILURE_MART_PATH}"]\n'
                'reason = "The case checks type facts through a derived table."\n',
                FAILURE_MART_PATH: 'MODEL (\n  description "Order totals per customer",\n'
                "  contract enforced,\n"
                "  columns (\n    customer_id (type INTEGER),\n    amount (type VARCHAR),\n  ),\n"
                ");\n\n"
                f"SELECT customer_id, amount\nFROM (SELECT * FROM {_ORDERS}) staged\n",
            },
        ),
        failure_case(
            name="analysis-session-cte-fact-type-mismatch",
            expected_code="K002",
            files={
                FAILURE_MART_PATH: _contract_header(
                    "    customer_id (type INTEGER),\n    total_amount (type VARCHAR),\n"
                )
                + "WITH totals AS (\n  SELECT customer_id, CAST(amount AS DOUBLE) AS total_amount\n"
                f"  FROM {_ORDERS}\n)\n"
                "SELECT customer_id + 0 AS customer_id, total_amount FROM totals\n",
            },
        ),
        failure_case(
            name="analysis-session-cte-fact-nullability",
            expected_code="K004",
            files={
                FAILURE_MART_PATH: _contract_header(
                    "    customer_id (type INTEGER),\n    note (type VARCHAR, nullable false),\n"
                )
                + "WITH base AS (\n  SELECT customer_id, CAST(NULL AS VARCHAR) AS note\n"
                f"  FROM {_ORDERS}\n)\nSELECT customer_id + 0 AS customer_id, note FROM base\n",
            },
        ),
        failure_case(
            name="analysis-session-pivot-values-on-clause",
            expected_code="K011",
            expected_message="exactly one pivot column",
            files={
                _PIVOT_PATH: _pivot_model(
                    pivot_column="status",
                    body="PIVOT __source(\"raw_orders\")\nON status IN ('placed')\n"
                    "USING MAX(amount)\nGROUP BY customer_id\n",
                ),
            },
        ),
        failure_case(
            name="analysis-session-pivot-family-column-mismatch",
            expected_code="K011",
            expected_message="declares pivot_column 'region'",
            files={
                _PIVOT_PATH: _pivot_model(
                    pivot_column="region",
                    body='PIVOT __source("raw_orders")\nON status\nUSING MAX(amount)\n'
                    "GROUP BY customer_id\n",
                ),
            },
        ),
    )
