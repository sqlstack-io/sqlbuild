"""Failure cases owned by the native semantic completion lane."""

from scripts.compiler_differential._helpers.corpus.case_builder import (
    config_files,
    failure_case,
    mart_body_files,
    staging_files,
)
from scripts.compiler_differential.constants import (
    FAILURE_BASE_SOURCES,
    FAILURE_BASE_STAGING,
    FAILURE_MART_PATH,
    FAILURE_SOURCES_PATH,
)
from scripts.compiler_differential.models import FailureCase

_ORDERS: str = '__ref("stg_orders")'
_TIMED_STAGING: str = FAILURE_BASE_STAGING.replace(
    "amount, status", "amount, status, CAST('2026-01-01' AS TIMESTAMP) AS ordered_at"
)

_INTERMEDIATE_PATH: str = "sql/models/intermediate/int_orders.sql"
_INTERMEDIATE: str = (
    'MODEL (\n  description "Doubled order amounts",\n);\n\n'
    "SELECT customer_id, amount * 2 AS doubled_amount\n"
    'FROM __ref("stg_orders")\n'
)

_FUNCTION_PATH: str = "sql/functions/sql/scaled_amount.sql"
_SCALED_AMOUNT: str = (
    "FUNCTION (\n"
    '  description "Scale an amount by an integer factor",\n'
    "  arguments (p_amount DOUBLE, p_factor INTEGER),\n"
    "  returns DOUBLE,\n);\n\n"
    "p_amount * p_factor\n"
)
_LOADED_SOURCE: str = (
    "  - name: raw_events\n    description: Loaded events.\n    managed: true\n"
    "    contract: enforced\n"
    "    columns:\n      - name: event_id\n        type: INTEGER\n"
    "      - name: load_seq\n        type: INTEGER\n"
)
_LOADER: str = (
    "from sqlbuild.loaders import loader\n\n\n"
    "@loader(write_strategy='append', cursor_column='loaded_at')\n"
    "def raw_events(ctx):\n    '''Load events.'''\n    return [{'event_id': 1, 'load_seq': 1}]\n"
)


def semantic_checks_failure_cases() -> tuple[FailureCase, ...]:
    """Return this lane's failure cases; the lane appends here without editing shared lists."""

    return (
        failure_case(
            name="semantic-recovered-downstream-uses",
            expected_code="B002",
            expected_message="Unknown column 'amonut' in raw_orders",
            expected_help="did you mean 'amount'?",
            expected_notes=(
                "raw_orders has: amount, status, customer_id, order_id",
                "1 downstream uses of stg_orders.amount were not checked because of this error",
            ),
            files={
                **staging_files(FAILURE_BASE_STAGING.replace("amount, status", "amonut, status")),
                **mart_body_files(
                    f"SELECT customer_id, SUM(amount) AS total_amount\nFROM {_ORDERS}\n"
                    "GROUP BY customer_id\n"
                ),
            },
        ),
        failure_case(
            name="semantic-recovered-uses-through-intermediate",
            expected_code="B002",
            expected_message="Unknown column 'amonut' in raw_orders",
            expected_help="did you mean 'amount'?",
            expected_notes=(
                "raw_orders has: amount, status, customer_id, order_id",
                "1 downstream uses of stg_orders.amount were not checked because of this error",
            ),
            files={
                **staging_files(FAILURE_BASE_STAGING.replace("amount, status", "amonut, status")),
                _INTERMEDIATE_PATH: _INTERMEDIATE,
                **mart_body_files(
                    "SELECT customer_id, SUM(doubled_amount) AS total_amount\n"
                    'FROM __ref("int_orders")\nGROUP BY customer_id\n'
                ),
            },
        ),
        failure_case(
            name="semantic-qualified-alias-unknown-column",
            expected_code="B002",
            expected_message="Unknown column 'amonut' in stg_orders (as o)",
            expected_help="did you mean 'amount'?",
            files=mart_body_files(
                f"SELECT o.customer_id, SUM(o.amonut) AS total_amount\nFROM {_ORDERS} AS o\n"
                "GROUP BY o.customer_id\n"
            ),
        ),
        failure_case(
            name="semantic-temporal-comparison-help",
            expected_code="B217",
            expected_help="compare with a timestamp, for example TIMESTAMP '2026-04-01'",
            expected_notes=("o.ordered_at is TIMESTAMP, 5 is INTEGER",),
            expected_location=(7, 7),
            files={
                **staging_files(_TIMED_STAGING),
                **mart_body_files(
                    f"SELECT o.customer_id, SUM(o.amount) AS total_amount\nFROM {_ORDERS} AS o\n"
                    "WHERE o.ordered_at > 5\nGROUP BY o.customer_id\n"
                ),
            },
        ),
        failure_case(
            name="semantic-type-error-unchecked-uses",
            expected_code="B212",
            expected_notes=(
                "1 downstream output uses were not type-checked because of this error",
                "status is TEXT, 1 is INTEGER",
            ),
            files={
                **staging_files(
                    FAILURE_BASE_STAGING.replace("amount, status", "status + 1 AS amount, status")
                ),
                **mart_body_files(
                    f"SELECT customer_id, SUM(amount) AS total_amount\nFROM {_ORDERS}\n"
                    "GROUP BY customer_id\n"
                ),
            },
        ),
        failure_case(
            name="semantic-rejected-opt-out-counts-findings",
            expected_code="P009",
            expected_message="`sql_analysis false` is not needed for model 'customer_totals'",
            expected_help="hiding: 2 unknown columns, 1 type mismatch (run `sqb compile`",
            files={
                **config_files("\n[settings]\nrequire_sql_analysis = true\n"),
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n'
                    "  sql_analysis false,\n);\n\n"
                    "SELECT customer_id, SUM(refund) AS total_amount, MAX(amonut) AS top_amount\n"
                    f"FROM {_ORDERS}\nWHERE status > 5\nGROUP BY customer_id\n"
                ),
            },
        ),
        *_metadata_cases(),
    )


def _metadata_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="semantic-udf-qualified-cast-argument-family",
            expected_code="B301",
            expected_message=(
                "Function 'scaled_amount' argument 'p_factor' expects INTEGER, received TEXT"
            ),
            files={
                _FUNCTION_PATH: _SCALED_AMOUNT,
                **mart_body_files(
                    "SELECT o.customer_id,\n"
                    '  __udf("scaled_amount")(o.amount, CAST(o.status AS VARCHAR))'
                    " AS total_amount\n"
                    f"FROM {_ORDERS} AS o\n"
                ),
            },
        ),
        failure_case(
            name="semantic-cursor-inputs-unknown-column",
            expected_code="B300",
            expected_message="cursor_inputs stg_orders references unknown column 'placed_at'",
            files={
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n'
                    "  materialized incremental,\n  incremental_strategy delete_insert,\n"
                    "  unique_key [customer_id],\n  cursor customer_id,\n"
                    "  cursor_type integer,\n"
                    "  cursor_inputs (\n    stg_orders placed_at,\n  ),\n);\n\n"
                    "SELECT customer_id, SUM(amount) AS total_amount\n"
                    f"FROM {_ORDERS}\nGROUP BY customer_id\n"
                )
            },
        ),
        failure_case(
            name="semantic-source-cursor-unknown-column",
            expected_code="B300",
            expected_message="cursor_column references unknown column 'loaded_at'",
            expected_location=(21, 20),
            files={
                FAILURE_SOURCES_PATH: FAILURE_BASE_SOURCES + _LOADED_SOURCE,
                "python/nodes/loaders/raw_events/loader.py": _LOADER,
            },
        ),
    )
