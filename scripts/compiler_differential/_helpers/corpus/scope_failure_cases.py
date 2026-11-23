"""Minimal projects that fail on declarations and scopes: files, references, index and grants."""

from scripts.compiler_differential.constants import (
    FAILURE_BASE_FILES,
    FAILURE_MART_PATH,
    FAILURE_STAGING_PATH,
)
from scripts.compiler_differential.models import FailureCase

_STAGING_MACRO_PATH: str = "sql/models/staging/_macros/python/amounts.py"
_STAGING_MACRO: str = "def staged_amount(value):\n    return f'{value} * 1'\n"
_STAGING_ENUM_PATH: str = "sql/models/staging/_enums/status.sql"
_STAGING_ENUM: str = "ENUM (name staged_status, members [PLACED]);\n"
_MART_HEADER: str = 'MODEL (\n  description "Order totals per customer",\n);\n\n'
_STAGING_HEADER: str = 'MODEL (\n  description "Staged orders",\n);\n\n'
_TEST_PATH: str = "tests/unit/test_customer_totals.sql"
_SCOPE_SCENARIO_PATH: str = "tests/scenarios/orders.sql"
_SCENARIO_HEADER: str = 'SCENARIO (\n  description "Order flow"\n);\n\n'


_GLOBAL_MACRO_PATH: str = "sql/macros/python/amounts.py"
_GLOBAL_MACRO: str = "def doubled(value):\n    return f'{value} * 2'\n"
_MART_WITH_MACRO: str = (
    _MART_HEADER + "SELECT customer_id, SUM(@py.doubled('amount')) AS total_amount\n"
    'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
)


def _case(
    *, name: str, expected_code: str, files: dict[str, str], expected_message: str | None = None
) -> FailureCase:
    return FailureCase(
        files={**FAILURE_BASE_FILES, **files},
        name=name,
        expected_code=expected_code,
        expected_message=expected_message,
    )


def _staging_after_macro(*, where: str, files: dict[str, str] | None = None) -> dict[str, str]:
    return {
        **(files or {}),
        _GLOBAL_MACRO_PATH: _GLOBAL_MACRO,
        FAILURE_MART_PATH: _MART_WITH_MACRO,
        FAILURE_STAGING_PATH: _STAGING_HEADER
        + "SELECT order_id, customer_id, amount, status\n"
        + f'FROM __source("raw_orders")\nWHERE {where}\n',
    }


def scope_failure_cases() -> tuple[FailureCase, ...]:
    """Return the declaration-scope failure cases in a stable order."""

    return (
        *_declaration_reference_cases(),
        _case(
            name="scope-duplicate-scoped-enum",
            expected_code="P001",
            files={
                _STAGING_ENUM_PATH: _STAGING_ENUM,
                "sql/enums/status.sql": "ENUM (name staged_status, members [SHIPPED]);\n",
            },
        ),
        _case(
            name="scope-inaccessible-local-macro",
            expected_code="P001",
            files={
                _STAGING_MACRO_PATH: _STAGING_MACRO,
                FAILURE_MART_PATH: _MART_HEADER
                + "SELECT customer_id, SUM(@py.staged_amount('amount')) AS total_amount\n"
                'FROM __ref("stg_orders")\nGROUP BY customer_id\n',
            },
        ),
        _case(
            name="scope-first-of-two-visibility-errors",
            expected_code="P001",
            files={
                _STAGING_ENUM_PATH: _STAGING_ENUM,
                "sql/models/marts/_macros/python/totals.py": (
                    "def mart_total(value):\n    return value\n"
                ),
                FAILURE_STAGING_PATH: _STAGING_HEADER
                + "SELECT order_id, customer_id, @py.mart_total('amount') AS amount, status\n"
                'FROM __source("raw_orders")\n',
                FAILURE_MART_PATH: _MART_HEADER
                + "SELECT customer_id, SUM(amount) AS total_amount\n"
                'FROM __ref("stg_orders")\n'
                'WHERE status = @enum("staged_status").PLACED\nGROUP BY customer_id\n',
            },
        ),
        _case(
            name="scope-test-outside-relationship-grant",
            expected_code="P001",
            files={
                _STAGING_ENUM_PATH: _STAGING_ENUM,
                _TEST_PATH: 'TEST (name "totals_by_customer");\n\nWITH\n'
                "__ref__stg_orders AS (\n  SELECT 10 AS customer_id, 5 AS amount\n),\n"
                "__expected__customer_totals AS (\n"
                '  SELECT 10 AS customer_id, @enum("staged_status").PLACED AS status\n)\n'
                "SELECT 1\n",
            },
        ),
        _case(
            name="scope-malformed-expected-relationship",
            expected_code="P001",
            files={
                _STAGING_ENUM_PATH: _STAGING_ENUM,
                _TEST_PATH: 'TEST (name "totals_by_customer");\n\nWITH\n'
                "__ref__stg_orders AS (\n  SELECT 10 AS customer_id\n),\n"
                "__expected__ AS (\n  SELECT 10 AS customer_id\n)\nSELECT 1\n",
            },
        ),
        _case(
            name="scope-scenario-expected-without-target",
            expected_code="P001",
            expected_message=(
                f"SQL scenario '{_SCOPE_SCENARIO_PATH}' must use __expected__<model> to identify "
                "a target"
            ),
            files={
                _STAGING_ENUM_PATH: _STAGING_ENUM,
                _SCOPE_SCENARIO_PATH: _SCENARIO_HEADER
                + "WITH\n__source__raw_orders AS (SELECT 1 AS order_id),\n"
                "__expected__ AS (SELECT 1)\n",
            },
        ),
        _case(
            name="scope-macro-test-cte-without-body",
            expected_code="P001",
            expected_message=f"SQL test '{_TEST_PATH}' CTE '__macro_actual__' must use AS (...)",
            files={
                _STAGING_ENUM_PATH: _STAGING_ENUM,
                "sql/macros/python/amounts.py": "def doubled(value):\n    return f'{value} * 2'\n",
                _TEST_PATH: 'TEST (mode macro, name "doubled");\n\nWITH\n'
                "__macro_actual__ AS SELECT @py.doubled('1') AS amount\n",
            },
        ),
    )


def _declaration_reference_cases() -> tuple[FailureCase, ...]:
    return (
        _case(
            name="declaration-unknown-enum-after-macro",
            expected_code="P001",
            expected_message="Unknown enum 'missing_status'",
            files=_staging_after_macro(where='status = @enum("missing_status").PLACED'),
        ),
        _case(
            name="declaration-first-of-two-reference-errors",
            expected_code="P001",
            expected_message="Unknown member 'HELD' for enum 'staged_status'",
            files=_staging_after_macro(
                where='status = @enum("staged_status").HELD OR amount > @const(cap)',
                files={_STAGING_ENUM_PATH: _STAGING_ENUM},
            ),
        ),
        _case(
            name="declaration-invalid-constant-reference",
            expected_code="P001",
            expected_message="Invalid constant reference",
            files=_staging_after_macro(where="amount > @const(cap)"),
        ),
        _case(
            name="declaration-unclosed-quote-before-reference",
            expected_code="P001",
            expected_message="Enum and constant expansion contains an unclosed quoted string",
            files=_staging_after_macro(where='status = \'placed OR amount > @const("cap")'),
        ),
        _case(
            name="declaration-inaccessible-local-constant",
            expected_code="P001",
            expected_message="Constant 'order_cap' is known but inaccessible",
            files=_staging_after_macro(
                where='amount > @const("order_cap")',
                files={
                    "sql/models/marts/_constants/limits.sql": "CONSTANT (name order_cap, value 3);\n"
                },
            ),
        ),
    )
