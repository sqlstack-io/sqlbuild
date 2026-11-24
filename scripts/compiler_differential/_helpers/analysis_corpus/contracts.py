"""Failure cases owned by the native contracts and promotion conflicts lane."""

from scripts.compiler_differential._helpers.corpus.case_builder import (
    failure_case,
    mart_body_files,
    staging_files,
)
from scripts.compiler_differential.constants import (
    FAILURE_BASE_CONFIG,
    FAILURE_CONFIG_PATH,
    FAILURE_MART_PATH,
)
from scripts.compiler_differential.models import FailureCase

_SOURCE_QUERY: str = 'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
_ENFORCED_HEADER: str = 'MODEL (\n  description "Staged orders",\n  contract enforced,\n'
_TYPED_COLUMNS: str = (
    "  columns (\n    order_id (type INTEGER),\n    customer_id (type INTEGER),\n"
    "    amount (type DOUBLE),\n    status (type VARCHAR),\n  ),\n"
)
_NAMED_SCHEMA_PATH: str = "sql/models/staging/_sqlbuild/_schemas/staged_order.sql"
_NAMED_SCHEMA: str = (
    'SCHEMA (\n  name staged_order,\n  description "Canonical staged order shape",\n'
    "  columns (\n    order_id (type INTEGER),\n    customer_id (type INTEGER),\n"
    "    amount (type DOUBLE),\n    shipped_at (type TIMESTAMP),\n  ),\n);\n"
)
_PIVOT_MART: str = (
    'MODEL (\n  description "Order amounts per status",\n  materialized table,\n'
    "  columns (customer_id (type INTEGER)),\n"
    "  dynamic_columns (\n    status_amounts (\n      pivot_column status,\n"
    "      value_column amount,\n      aggregate MAX,\n      type {family_type}\n    )\n  ),\n"
    ');\n\nPIVOT __ref("stg_orders")\nON status\nUSING MAX(amount)\nGROUP BY customer_id\n'
)
_IMMEDIATE_SETTING: str = '\n[settings]\ntable_promotion_mode = "immediate"\n'


def contracts_failure_cases() -> tuple[FailureCase, ...]:
    """Return this lane's failure cases; the lane appends here without editing shared lists."""

    return (
        failure_case(
            name="contract-order-every-column-family",
            expected_code="K005",
            expected_message="column 'status' is not declared in enforced contract for model "
            "'stg_orders'",
            files=staging_files(
                f"{_ENFORCED_HEADER}  columns (\n    order_id (type VARCHAR),\n"
                "    customer_id (type INTEGER, nullable false),\n"
                "    shipped_at (type TIMESTAMP),\n    amount (type DOUBLE),\n  ),\n);\n\n"
                "SELECT order_id, CAST(NULL AS INTEGER) AS customer_id, amount, status\n"
                'FROM __source("raw_orders")\n'
            ),
        ),
        failure_case(
            name="contract-implicit-missing-column",
            expected_code="K001",
            expected_message="declared column 'order_count' was not found in statically inferred "
            "output for model 'customer_totals'",
            files={
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n  columns (\n'
                    "    customer_id (type INTEGER),\n    total_amount (type DOUBLE),\n"
                    "    order_count (type BIGINT),\n  ),\n);\n\n"
                    "SELECT customer_id, SUM(amount) AS total_amount\n"
                    'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
                )
            },
        ),
        failure_case(
            name="contract-errors-across-models",
            expected_code="K002",
            expected_message="column 'total_amount' inferred as DOUBLE but declared type is BIGINT",
            files={
                **staging_files(f"{_ENFORCED_HEADER});\n\n{_SOURCE_QUERY}"),
                **mart_body_files(
                    "SELECT customer_id, CAST(SUM(amount) AS VARCHAR) AS total_amount\n"
                    'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
                ),
                "sql/models/marts/typed_totals.sql": (
                    'MODEL (\n  description "Typed totals",\n'
                    "  columns (total_amount (type BIGINT)),\n);\n\n"
                    "SELECT CAST(1.5 AS DOUBLE) AS total_amount\n"
                ),
            },
        ),
        failure_case(
            name="contract-named-schema-columns",
            expected_code="K005",
            expected_help="add the column to the named SCHEMA or remove it from the SELECT list",
            files={
                _NAMED_SCHEMA_PATH: _NAMED_SCHEMA,
                **staging_files(
                    f"{_ENFORCED_HEADER}  model_schema staged_order,\n);\n\n{_SOURCE_QUERY}"
                ),
            },
        ),
        failure_case(
            name="contract-local-immediate-promotion",
            expected_code="K011",
            expected_message="contract enforced requires staged table promotion",
            files={
                "sqlbuild_local.toml": _IMMEDIATE_SETTING.lstrip("\n"),
                **staging_files(
                    f"{_ENFORCED_HEADER}  materialized table,\n{_TYPED_COLUMNS});\n\n"
                    f"{_SOURCE_QUERY}"
                ),
            },
        ),
        failure_case(
            name="contract-immediate-promotion-several-models",
            expected_code="K011",
            expected_message="model 'customer_totals': contract enforced requires staged table "
            "promotion",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + '\n[defaults]\ncontract = "enforced"\nmaterialized = "table"\n'
                + _IMMEDIATE_SETTING,
                **staging_files(
                    'MODEL (\n  description "Staged orders",\n  materialized view,\n'
                    f"{_TYPED_COLUMNS});\n\n{_SOURCE_QUERY}"
                ),
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n  columns (\n'
                    "    customer_id (type INTEGER),\n    total_amount (type DOUBLE),\n  ),\n"
                    ");\n\nSELECT customer_id, SUM(amount) AS total_amount\n"
                    'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
                ),
            },
        ),
        failure_case(
            name="contract-dynamic-family-type-mismatch",
            expected_code="K002",
            expected_message="dynamic column family 'status_amounts'",
            files={
                **staging_files(
                    f"{_ENFORCED_HEADER}  materialized table,\n{_TYPED_COLUMNS});\n\n"
                    "SELECT CAST(order_id AS INTEGER) AS order_id,\n"
                    "  CAST(customer_id AS INTEGER) AS customer_id,\n"
                    "  CAST(amount AS DOUBLE) AS amount, CAST(status AS VARCHAR) AS status\n"
                    'FROM __source("raw_orders")\n'
                ),
                FAILURE_MART_PATH: _PIVOT_MART.format(family_type="VARCHAR"),
            },
        ),
    )
