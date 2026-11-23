"""Minimal projects whose audits, functions, sources, SQL tests or scenarios fail to attach."""

from scripts.compiler_differential._helpers.corpus.case_builder import config_files, failure_case
from scripts.compiler_differential.constants import (
    FAILURE_BASE_SOURCES,
    FAILURE_BASE_STAGING,
    FAILURE_SOURCES_PATH,
    FAILURE_STAGING_PATH,
)
from scripts.compiler_differential.models import FailureCase

_STAGING_HEADER: str = '"Staged orders",'
_FLOOR_AUDIT_PATH: str = "sql/models/staging/_sqlbuild/_audits/generic/floor.sql"
_FLOOR_AUDIT: str = 'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE amount < @minimum\n'
_FRESH_AUDIT: str = (
    'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE ordered_at >= __cursor_start()\n'
)
_ORDER_LABEL_PATH: str = "sql/functions/sql/order_label.sql"
_ORDER_FLAG_PATH: str = "sql/functions/python/order_flag.py"
_SCENARIO_PATH: str = "tests/scenarios/orders.sql"
_SCENARIO_HEADER: str = 'SCENARIO (\n  description "Order flow"\n);\n\n'
_SOURCE_FIXTURE: str = (
    "__source__raw_orders AS (\n"
    "  SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount, 'placed' AS status\n"
    ")"
)
_EXPECTED_STAGING: str = "__expected__stg_orders AS (\n  SELECT 1 AS order_id\n)"
_TEST_PATH: str = "tests/unit/test_stg_orders.sql"
_SCHEMA_PATH: str = "sql/schemas/orders.sql"


def _scenario(*ctes: str, tail: str = "\nSELECT 1\n") -> str:
    return _SCENARIO_HEADER + "WITH\n" + ",\n".join(ctes) + tail


def _staging_audits(audits: str) -> dict[str, str]:
    return {
        FAILURE_STAGING_PATH: FAILURE_BASE_STAGING.replace(
            _STAGING_HEADER, f"{_STAGING_HEADER}\n  audits [{audits}],"
        )
    }


def _order_flag(decorator_lines: str) -> dict[str, str]:
    return {
        _ORDER_FLAG_PATH: (
            "from sqlbuild.functions import udf\n\n\n@udf(\n"
            '    arguments={"order_status": "STRING"},\n    returns="BOOLEAN",\n'
            f"{decorator_lines})\n"
            "def main(order_status: str | None) -> bool:\n"
            '    """Flag completed orders."""\n    return order_status == "completed"\n'
        )
    }


def _order_label(header: str) -> dict[str, str]:
    return {_ORDER_LABEL_PATH: f"FUNCTION (\n{header});\n\nUPPER(p_status)\n"}


def _schema(*, name: str, extends: str | None, column: str) -> str:
    parent: str = "" if extends is None else f"extends {extends}, "
    return f"SCHEMA (name {name}, {parent}columns ({column} (type INTEGER)));\n"


def attachment_failure_cases() -> tuple[FailureCase, ...]:
    """Return audit, function, source, SQL test and scenario failures the native stage reports."""

    return (
        failure_case(
            name="model-schema-extends-unknown",
            expected_code="P001",
            expected_message=(
                "Schema 'order_row' in sql/schemas/orders.sql extends unknown schema 'base_row'"
            ),
            files={_SCHEMA_PATH: _schema(name="order_row", extends="base_row", column="id")},
        ),
        failure_case(
            name="model-schema-inheritance-cycle",
            expected_code="P001",
            expected_message="Model schema inheritance cycle: order_row -> base_row -> order_row",
            files={
                _SCHEMA_PATH: _schema(name="order_row", extends="base_row", column="id")
                + _schema(name="base_row", extends="order_row", column="amount")
            },
        ),
        failure_case(
            name="model-schema-redeclared-column",
            expected_code="P001",
            expected_message=(
                "Schema 'order_row' in sql/schemas/orders.sql redeclares inherited column 'ID' from "
                "sql/schemas/orders.sql:1; column overrides are not supported"
            ),
            files={
                _SCHEMA_PATH: _schema(name="base_row", extends=None, column="id")
                + _schema(name="order_row", extends="base_row", column="ID")
            },
        ),
        failure_case(
            name="generic-audit-unknown-run-scope",
            expected_code="P001",
            expected_message="unknown audit run_scope 'always'",
            files={
                _FLOOR_AUDIT_PATH: _FLOOR_AUDIT,
                **_staging_audits('floor (minimum 0, run_scope "always")'),
            },
        ),
        failure_case(
            name="generic-audit-overrides-implicit-model",
            expected_code="P001",
            expected_message="must not override implicit model from attached context",
            files={
                _FLOOR_AUDIT_PATH: _FLOOR_AUDIT,
                **_staging_audits('floor (minimum 0, model "customer_totals")'),
            },
        ),
        failure_case(
            name="generic-audit-cursor-intrinsic",
            expected_code="P001",
            expected_message="Audit 'fresh' uses cursor intrinsics",
            files={
                "sql/models/staging/_sqlbuild/_audits/generic/fresh.sql": _FRESH_AUDIT,
                **_staging_audits("fresh"),
            },
        ),
        failure_case(
            name="audit-unknown-project-variable",
            expected_code="P001",
            expected_message="unknown project variable '@@floor_amount'",
            files={
                **config_files('\n[vars]\nregion = "north"\n'),
                _FLOOR_AUDIT_PATH: _FLOOR_AUDIT.replace("@minimum", "@@floor_amount"),
                **_staging_audits("floor"),
            },
        ),
        failure_case(
            name="audit-variable-in-doubled-backticks",
            expected_code="P001",
            expected_message="unknown project variable '@@floor_amount'",
            files={
                **config_files('\n[vars]\nregion = "north"\n'),
                _FLOOR_AUDIT_PATH: _FLOOR_AUDIT.replace("@minimum", "`@@floor_amount``"),
                **_staging_audits("floor"),
            },
        ),
        failure_case(
            name="sql-function-missing-returns",
            expected_code="P001",
            expected_message="sql/functions/sql/order_label.sql must declare returns",
            files=_order_label('  description "Order label",\n  arguments (p_status VARCHAR),\n'),
        ),
        failure_case(
            name="sql-function-template-namespace",
            expected_code="P001",
            expected_message="references unsupported template namespace 'VAR'",
            files=_order_label(
                '  description "Order label",\n  arguments (p_status VARCHAR),\n'
                '  returns "${VAR:label_type}",\n'
            ),
        ),
        failure_case(
            name="sql-function-invalid-return-type",
            expected_code="P001",
            expected_message=(
                "type 'DECIMAL(10,' is not valid for adapter 'duckdb' SQL analysis dialect "
                "'duckdb': Parse error at line 1, column 12: Expected number"
            ),
            files=_order_label(
                '  description "Order label",\n  arguments (p_status VARCHAR),\n'
                '  returns "DECIMAL(10,",\n'
            ),
        ),
        failure_case(
            name="sql-function-arguments-not-a-map",
            expected_code="P001",
            expected_message="sql/functions/sql/order_label.sql arguments must be a map",
            files=_order_label(
                '  description "Order label",\n  arguments [p_status],\n  returns VARCHAR\n'
            ),
        ),
        failure_case(
            name="sql-function-blank-tag",
            expected_code="P001",
            expected_message="order_label.sql tags entries must be non-empty strings",
            files=_order_label(
                '  description "Order label",\n  arguments (p_status VARCHAR),\n'
                '  returns VARCHAR,\n  tags [""]\n'
            ),
        ),
        failure_case(
            name="sql-function-returns-list",
            expected_code="P001",
            expected_message="returns must be a type string or table column declaration",
            files=_order_label(
                '  description "Order label",\n  arguments (p_status VARCHAR),\n'
                "  returns [VARCHAR]\n"
            ),
        ),
        failure_case(
            name="python-function-missing-runtime-version",
            expected_code="P001",
            expected_message="sql/functions/python/order_flag.py must declare runtime_version",
            files=_order_flag(""),
        ),
        failure_case(
            name="python-function-packages-not-a-list",
            expected_code="P001",
            expected_message="sql/functions/python/order_flag.py packages must be a list",
            files=_order_flag('    runtime_version="3.11",\n    packages="numpy",\n'),
        ),
        failure_case(
            name="source-description-unknown-variable",
            expected_code="P001",
            expected_message="references unknown variable 'channel'",
            files={
                FAILURE_SOURCES_PATH: FAILURE_BASE_SOURCES.replace(
                    "description: Orders feed.", 'description: "Orders from ${channel}"'
                )
            },
        ),
        failure_case(
            name="scenario-fixture-without-target",
            expected_code="P001",
            expected_message=(
                f"SQL scenario '{_SCENARIO_PATH}' must use __seed__<seed> to identify a target"
            ),
            files={_SCENARIO_PATH: _scenario("__seed__ AS (SELECT 1 AS id)", _EXPECTED_STAGING)},
        ),
        failure_case(
            name="scenario-macro-mock",
            expected_code="P001",
            expected_message="does not support macro mock CTE '__macro__tidy'",
            files={
                _SCENARIO_PATH: _scenario(
                    _SOURCE_FIXTURE, "__macro__tidy AS (SELECT 'x')", _EXPECTED_STAGING
                )
            },
        ),
        failure_case(
            name="scenario-without-checks",
            expected_code="P001",
            expected_message=(
                f"SQL scenario '{_SCENARIO_PATH}' must define at least one __expected__<model> "
                "or __assert__<assertion> CTE"
            ),
            files={_SCENARIO_PATH: _scenario(_SOURCE_FIXTURE)},
        ),
        failure_case(
            name="scenario-statement-after-ctes",
            expected_code="P001",
            expected_message="must end after its CTEs",
            files={
                _SCENARIO_PATH: _scenario(
                    _SOURCE_FIXTURE, _EXPECTED_STAGING, tail="\nSELECT order_id FROM orders\n"
                )
            },
        ),
        failure_case(
            name="scenario-check-reads-source",
            expected_code="P001",
            expected_message="CTE '__expected__stg_orders' must not reference project source",
            files={
                _SCENARIO_PATH: _scenario(
                    _SOURCE_FIXTURE,
                    '__expected__stg_orders AS (\n  SELECT order_id FROM __source("raw_orders")\n)',
                )
            },
        ),
        failure_case(
            name="scenario-dependent-checks",
            expected_code="P001",
            expected_message=(
                "check CTE '__expected__stg_orders' must not depend on '__assert__no_rows'"
            ),
            files={
                _SCENARIO_PATH: _scenario(
                    _SOURCE_FIXTURE,
                    "__assert__no_rows AS (\n  SELECT 1 FROM __source__raw_orders WHERE false\n)",
                    "__expected__stg_orders AS (\n  SELECT 1 AS order_id FROM __assert__no_rows\n)",
                )
            },
        ),
        failure_case(
            name="scenario-first-of-two-errors",
            expected_code="P001",
            expected_message=(
                "SQL scenario 'tests/scenarios/a_orders.sql' must use __seed__<seed> to identify "
                "a target"
            ),
            files={
                "tests/scenarios/a_orders.sql": _scenario(
                    "__seed__ AS (SELECT 1 AS id)", _EXPECTED_STAGING
                ),
                "tests/scenarios/b_orders.sql": _scenario(_SOURCE_FIXTURE),
            },
        ),
        failure_case(
            name="test-mocks-unknown-source",
            expected_code="P001",
            expected_message="mocks unknown source 'raw_payments'",
            files={
                _TEST_PATH: "TEST();\n\nWITH\n__source__raw_payments AS (\n  SELECT 1 AS id\n),\n"
                f"{_EXPECTED_STAGING}\nSELECT 1\n"
            },
        ),
        failure_case(
            name="scenario-quoted-cte-name",
            expected_code="P001",
            expected_message=(
                f"SQL scenario '{_SCENARIO_PATH}' CTE name \"__source__raw_orders\" must be an "
                "unquoted identifier of ASCII letters, digits and underscores"
            ),
            expected_help=(
                "rename the CTE, for example __source__raw_orders; quoted CTE names and names "
                "with $ or non-ASCII characters are not supported"
            ),
            files={
                _SCENARIO_PATH: _scenario(
                    '"__source__raw_orders" AS (\n  SELECT 1 AS order_id\n)', _EXPECTED_STAGING
                )
            },
        ),
        failure_case(
            name="scenario-materialized-cte",
            expected_code="P001",
            expected_message=(
                f"SQL scenario '{_SCENARIO_PATH}' CTE '__source__raw_orders' must not use AS "
                "MATERIALIZED; materialization hints are not supported in SQL scenario CTEs"
            ),
            expected_help="remove MATERIALIZED and write __source__raw_orders AS (...)",
            files={
                _SCENARIO_PATH: _scenario(
                    "__source__raw_orders AS MATERIALIZED (\n  SELECT 1 AS order_id\n)",
                    _EXPECTED_STAGING,
                )
            },
        ),
        failure_case(
            name="macro-test-quoted-cte-name",
            expected_code="P001",
            expected_message=(
                f"SQL test '{_TEST_PATH}' CTE name \"__macro_actual__\" must be an unquoted "
                "identifier of ASCII letters, digits and underscores"
            ),
            expected_help=(
                "rename the CTE, for example __macro_actual__; quoted CTE names and names with "
                "$ or non-ASCII characters are not supported"
            ),
            files={
                "sql/macros/python/amounts.py": "def doubled(value):\n    return f'{value} * 2'\n",
                _TEST_PATH: 'TEST (mode macro, name "doubled");\n\nWITH\n'
                "\"__macro_actual__\" AS (\n  SELECT @py.doubled('1') AS amount\n),\n"
                "__macro_expected__ AS (\n  SELECT 2 AS amount\n)\nSELECT 1\n",
            },
        ),
    )
