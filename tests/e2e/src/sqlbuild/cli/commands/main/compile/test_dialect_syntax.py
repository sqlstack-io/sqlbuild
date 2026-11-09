"""E2E coverage of dialect syntax that compile analysis parses, types and expands."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DialectSyntaxCompileE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    compile_dialect_syntax_project,
    dialect_syntax_model,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DialectSyntaxCompileE2ETestCase(
            description="stars over derived tables expand to their columns",
            adapter="duckdb",
            models={
                "unaliased": dialect_syntax_model(
                    "Unaliased.", "SELECT * FROM (SELECT 1 AS order_id, 'a' AS status)"
                ),
                "set_operation": dialect_syntax_model(
                    "Set operation.",
                    "SELECT * FROM (SELECT 1 AS order_id UNION ALL SELECT 2) AS s",
                ),
                "renamed": dialect_syntax_model(
                    "Renamed.", "SELECT * FROM (SELECT 1 AS order_id, 'a' AS status) AS s(a, b)"
                ),
            },
            expected_exit_code=0,
            expected_codes=(),
            expected_column_counts={"unaliased": 2, "set_operation": 1, "renamed": 2},
            ignored_rules=("SQLCLEAN039",),
        ),
        DialectSyntaxCompileE2ETestCase(
            description="star EXCLUDE drops a column before set operation arms are compared",
            adapter="duckdb",
            models={
                "orders": dialect_syntax_model(
                    "Orders.",
                    "SELECT * EXCLUDE (status) FROM (SELECT 1 AS order_id, 'a' AS status) AS t "
                    "UNION ALL SELECT 2 AS order_id",
                ),
            },
            expected_exit_code=0,
            expected_codes=(),
            expected_column_counts={"orders": 1},
            ignored_rules=("SQLCLEAN007", "SQLCLEAN039"),
        ),
        DialectSyntaxCompileE2ETestCase(
            description="column alias lists name table function and table columns",
            adapter="duckdb",
            models={
                "series": dialect_syntax_model(
                    "Series.", "SELECT g.n FROM generate_series(1, 3) AS g(n)"
                ),
                "orders": dialect_syntax_model("Orders.", "SELECT 1 AS id, 'a' AS items"),
                "renamed_orders": dialect_syntax_model(
                    "Renamed orders.", 'SELECT o.a, o.b FROM __ref("orders") AS o(a, b)'
                ),
            },
            expected_exit_code=0,
            expected_codes=(),
            expected_column_counts={"series": 1, "renamed_orders": 2},
        ),
        DialectSyntaxCompileE2ETestCase(
            description="Snowflake typed lambdas and subscripts before a colon path parse",
            adapter="snowflake",
            models={
                "totals": dialect_syntax_model(
                    "Totals.",
                    "SELECT REDUCE(ARRAY_CONSTRUCT(1, 2), 0::DOUBLE, "
                    "(total DOUBLE, item VARIANT) -> total + item::DOUBLE) AS total",
                ),
                "names": dialect_syntax_model(
                    "Names.",
                    "SELECT o.f[0]:name::VARCHAR AS name "
                    "FROM (SELECT PARSE_JSON('[{\"name\": 1}]') AS f) AS o",
                ),
            },
            expected_exit_code=0,
            expected_codes=(),
            expected_column_counts={"totals": 1, "names": 1},
            ignored_rules=("SQLCLEAN039",),
        ),
        DialectSyntaxCompileE2ETestCase(
            description="Snowflake rejects Oracle's CONNECT_BY_ISLEAF pseudo-column",
            adapter="snowflake",
            models={
                "tree": dialect_syntax_model(
                    "Tree.",
                    "SELECT e.id, CONNECT_BY_ISLEAF AS leaf, LEVEL AS depth "
                    "FROM (SELECT 1 AS id, NULL AS parent_id) AS e "
                    "START WITH e.parent_id IS NULL CONNECT BY PRIOR e.id = e.parent_id",
                ),
            },
            expected_exit_code=1,
            expected_codes=("B002",),
            expected_column_counts={},
        ),
        DialectSyntaxCompileE2ETestCase(
            description="Snowflake accepts VALUES in FROM inside a CTE",
            adapter="snowflake",
            models={
                "codes": dialect_syntax_model(
                    "Codes.",
                    "WITH v AS (SELECT t.code FROM (VALUES (1), (2)) AS t(code)) "
                    "SELECT v.code FROM v",
                ),
            },
            expected_exit_code=0,
            expected_codes=(),
            expected_column_counts={"codes": 1},
            ignored_rules=("SQLCLEAN039",),
        ),
        DialectSyntaxCompileE2ETestCase(
            description="a ClickHouse DATE_TRUNC timestamp cursor keeps its timestamp type",
            adapter="clickhouse",
            models={
                "events": dialect_syntax_model("Events.", "SELECT CAST(now() AS DateTime) AS ts"),
                "hourly_events": dialect_syntax_model(
                    "Hourly events.",
                    "SELECT DATE_TRUNC('hour', e.ts) AS hour_ts FROM __ref(\"events\") AS e",
                    ", materialized incremental, incremental_strategy append, cursor hour_ts, "
                    "cursor_type timestamp, cursor_grain hour",
                ),
            },
            expected_exit_code=0,
            expected_codes=(),
            expected_column_counts={"hourly_events": 1},
        ),
        DialectSyntaxCompileE2ETestCase(
            description="BigQuery RANGE types parse in casts",
            adapter="bigquery",
            models={
                "periods": dialect_syntax_model(
                    "Periods.",
                    "SELECT CAST(NULL AS RANGE<DATE>) AS period, "
                    "CAST(NULL AS RANGE<TIMESTAMP>) AS active_window",
                ),
            },
            expected_exit_code=0,
            expected_codes=(),
            expected_column_counts={"periods": 2},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_syntax_when_compiling_then_analysis_parses_and_types_it(
    test_case: DialectSyntaxCompileE2ETestCase, tmp_path: Path
) -> None:
    compiled, codes, column_counts = compile_dialect_syntax_project(test_case, tmp_path)

    assert compiled.returncode == test_case.expected_exit_code, compiled.stdout + compiled.stderr
    assert codes == test_case.expected_codes
    assert {
        name: column_counts[name] for name in test_case.expected_column_counts
    } == test_case.expected_column_counts, test_case.description


@pytest.mark.xfail(
    strict=True,
    reason="Native Snowflake parse errors carry no syntax flag, so a bare VALUES CTE reports P026",
)
@pytest.mark.parametrize(
    "test_case",
    [
        DialectSyntaxCompileE2ETestCase(
            description="Snowflake rejects a CTE whose body is a bare VALUES list",
            adapter="snowflake",
            models={
                "codes": dialect_syntax_model(
                    "Codes.", "WITH v AS (VALUES (1), (2)) SELECT v.column1 AS code FROM v"
                ),
            },
            expected_exit_code=1,
            expected_codes=("P001",),
            expected_column_counts={},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_syntax_error_when_compiling_then_it_is_reported_as_p001(
    test_case: DialectSyntaxCompileE2ETestCase, tmp_path: Path
) -> None:
    compiled, codes, column_counts = compile_dialect_syntax_project(test_case, tmp_path)

    assert compiled.returncode == test_case.expected_exit_code, compiled.stdout + compiled.stderr
    assert codes == test_case.expected_codes
    assert {
        name: column_counts[name] for name in test_case.expected_column_counts
    } == test_case.expected_column_counts, test_case.description
