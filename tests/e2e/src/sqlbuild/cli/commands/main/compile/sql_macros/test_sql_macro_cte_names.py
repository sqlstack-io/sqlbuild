"""E2E coverage for the `__cte` argument naming the CTEs a CTE-list SQL macro call inserts."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroBuildTestCase,
    SqlMacroCommandTestCase,
    SqlMacroErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    ALL_MACROS,
    CLEAN_ORDERS,
    DATABASE,
    contract_model,
    inspected_text,
    orders_model,
    output,
    picked_orders,
    run_project,
    text_macro,
    with_ignored_rules,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import query_duckdb

_TWO_CALLS: str = (
    "WITH gb_raw AS (SELECT * FROM __ref(\"stg_orders\") WHERE country_code = 'GB'),\n"
    "fr_raw AS (SELECT * FROM __ref(\"stg_orders\") WHERE country_code = 'FR'),\n"
    "@sql.clean_orders(gb_raw, __cte := gb_orders)\n"
    "@sql.clean_orders(fr_raw, __cte := fr_orders)\n"
    "final AS (\n"
    "  SELECT order_id, amount_cents FROM gb_orders\n"
    "  UNION ALL SELECT order_id, amount_cents FROM fr_orders\n"
    ")\nSELECT * FROM final\n"
)
_INTERMEDIATE: str = (
    'WITH raw AS (SELECT * FROM __ref("stg_orders")),\n'
    "@sql.clean_orders(raw, __cte := cleaned)\n"
    "final AS (SELECT COUNT(*) AS live_rows FROM cleaned__live)\nSELECT * FROM final\n"
)
_CTE_LIST_TEST: str = (
    'TEST (mode macro, name "clean_orders_named_twice");\n\nWITH\n'
    "input_orders AS (\n"
    "  SELECT 1 AS order_id, TIMESTAMP '2026-01-01' AS updated_at, FALSE AS is_deleted\n"
    "  UNION ALL SELECT 1, TIMESTAMP '2026-01-02', FALSE\n),\n"
    "@sql.clean_orders(input_orders, __cte := first_pass)\n"
    "@sql.clean_orders(first_pass, __cte := second_pass)\n"
    "__macro_actual__ AS (SELECT order_id, updated_at FROM second_pass),\n"
    "__macro_expected__ AS (SELECT 1 AS order_id, TIMESTAMP '2026-01-02' AS updated_at)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroBuildTestCase(
            description="the_same_cte_list_macro_called_twice_with_cte_names",
            files={**CLEAN_ORDERS, **orders_model(_TWO_CALLS)},
            query="SELECT order_id, amount_cents FROM orders ORDER BY order_id",
            expected_rows=[(1, 1300), (2, 500)],
        ),
        SqlMacroBuildTestCase(
            description="a_caller_reads_a_prefixed_intermediate_cte",
            files={
                **CLEAN_ORDERS,
                **orders_model(_INTERMEDIATE),
                **with_ignored_rules(
                    "SQLCLEAN005", reason="The caller reads one intermediate CTE of the macro."
                ),
            },
            query="SELECT live_rows FROM orders",
            expected_rows=[(3,)],
        ),
        SqlMacroBuildTestCase(
            description="a_cte_read_under_its_own_name_as_alias",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src),\nlatest AS (SELECT live.order_id FROM live AS live)"
            ),
            query="SELECT COUNT(*) FROM orders",
            expected_rows=[(4,)],
        ),
        SqlMacroBuildTestCase(
            description="an_alias_named_like_a_cte_elsewhere",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src WHERE order_id = 1),\nlatest AS (SELECT live.order_id FROM live JOIN @src AS other ON live.order_id = other.order_id),\nfinal AS (SELECT o.order_id FROM latest AS o JOIN @src AS live ON live.order_id = o.order_id)"
            ),
            query="SELECT COUNT(*) FROM orders",
            expected_rows=[(8,)],
        ),
        SqlMacroBuildTestCase(
            description="a_nested_with_shadowing_a_cte",
            files={
                **picked_orders(
                    "WITH live AS (SELECT * FROM @src),\nlatest AS (WITH live AS (SELECT 42 AS order_id) SELECT order_id FROM live)"
                ),
                **with_ignored_rules("SQLCLEAN005", reason="The shadowed outer CTE is never read."),
            },
            query="SELECT order_id FROM orders",
            expected_rows=[(42,)],
        ),
        SqlMacroBuildTestCase(
            description="a_cte_read_in_a_derived_table",
            files={
                **picked_orders(
                    "WITH live AS (SELECT * FROM @src WHERE order_id = 2),\nlatest AS (SELECT s.order_id FROM (SELECT live.order_id FROM live) AS s)"
                ),
                **with_ignored_rules(
                    "SQLCLEAN039", reason="The case reads a CTE in a derived table."
                ),
            },
            query="SELECT order_id FROM orders",
            expected_rows=[(2,)],
        ),
        SqlMacroBuildTestCase(
            description="a_cte_read_in_a_set_operation",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src WHERE order_id = 2),\nlatest AS (SELECT order_id FROM live UNION ALL SELECT live.order_id FROM live)"
            ),
            query="SELECT order_id FROM orders",
            expected_rows=[(2,), (2,)],
        ),
        SqlMacroBuildTestCase(
            description="a_cte_read_in_a_join",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src WHERE order_id = 2),\nlatest AS (SELECT live.order_id FROM @src AS r JOIN live ON live.order_id = r.order_id)"
            ),
            query="SELECT order_id FROM orders",
            expected_rows=[(2,)],
        ),
        SqlMacroBuildTestCase(
            description="a_cte_read_in_a_correlated_subquery",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src WHERE order_id = 2),\nlatest AS (SELECT r.order_id FROM @src AS r WHERE EXISTS (SELECT 1 FROM live WHERE live.order_id = r.order_id))"
            ),
            query="SELECT order_id FROM orders",
            expected_rows=[(2,)],
        ),
        SqlMacroBuildTestCase(
            description="a_cte_read_in_a_parenthesised_join",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src WHERE order_id = 2),\nlatest AS (SELECT live.order_id FROM (live JOIN @src AS s ON live.order_id = s.order_id))"
            ),
            query="SELECT order_id FROM orders",
            expected_rows=[(2,)],
        ),
        SqlMacroBuildTestCase(
            description="a_cte_read_in_a_nested_parenthesised_join",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src WHERE order_id = 2),\nlatest AS (SELECT live.order_id FROM ((live JOIN @src AS s ON live.order_id = s.order_id) JOIN @src AS t ON t.order_id = live.order_id))"
            ),
            query="SELECT order_id FROM orders",
            expected_rows=[(2,)],
        ),
        SqlMacroBuildTestCase(
            description="a_nested_cte_reading_the_cte_it_shadows",
            files=picked_orders(
                "WITH live AS (SELECT * FROM @src WHERE order_id = 2),\nlatest AS (WITH live AS (SELECT * FROM live WHERE order_id > 0) SELECT order_id FROM live)"
            ),
            query="SELECT order_id FROM orders",
            expected_rows=[(2,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cte_names_when_building_then_each_call_inserts_its_own_ctes(
    test_case: SqlMacroBuildTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(tmp_path=tmp_path, files=test_case.files, command=("build",))

    assert result.returncode == 0, output(result)
    assert (
        query_duckdb(db_path=project_dir / DATABASE, sql=test_case.query) == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroErrorTestCase(
            description="cte_on_an_expression_macro",
            files={
                **text_macro("m", "VARCHAR", "UPPER(@value)"),
                **orders_model('SELECT @sql.m(status, __cte := x) AS v FROM __ref("stg_orders")\n'),
            },
            expected_code="P017",
            expected_fragments=(
                "__cte names the CTEs a CTE-list macro inserts; m() is an expression macro",
            ),
        ),
        SqlMacroErrorTestCase(
            description="cte_that_is_not_a_bare_name",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH raw AS (SELECT * FROM __ref("stg_orders")),\n'
                    '@sql.clean_orders(raw, __cte := "gb orders")\nSELECT * FROM raw\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                '__cte of macro clean_orders() must be a bare CTE name; got "gb orders"',
            ),
        ),
        SqlMacroErrorTestCase(
            description="the_same_macro_twice_without_cte_suggests_it",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH raw AS (SELECT * FROM __ref("stg_orders")),\n'
                    "@sql.clean_orders(raw)\n@sql.clean_orders(raw)\nSELECT * FROM latest\n"
                ),
            },
            expected_code="P018",
            expected_fragments=(
                'CTE "live" from macro clean_orders() clashes with a CTE from macro clean_orders()',
                "pass __cte := <name> to give the CTEs one call inserts their own names",
            ),
        ),
        SqlMacroErrorTestCase(
            description="a_cte_name_the_caller_already_defines",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH raw AS (SELECT * FROM __ref("stg_orders")),\n'
                    "picked AS (SELECT 1 AS x),\n"
                    "@sql.clean_orders(raw, __cte := picked)\nSELECT * FROM picked\n"
                ),
            },
            expected_code="P018",
            expected_fragments=(
                'CTE "picked" from macro clean_orders() clashes with a CTE already defined',
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_invalid_cte_name_when_compiling_then_the_error_names_the_problem(
    test_case: SqlMacroErrorTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == 1, output(result)
    assert f"error[{test_case.expected_code}]" in output(result), output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result), output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCommandTestCase(
            description="macro_mode_test_with_cte_names",
            files={
                **ALL_MACROS,
                **orders_model(_TWO_CALLS),
                "tests/unit/test_clean_orders.sql": _CTE_LIST_TEST,
            },
            command=("test",),
            expected_returncode=0,
            expected_fragments=("PASS=1", "clean_orders_named_twice"),
        ),
        SqlMacroCommandTestCase(
            description="lineage_reads_through_renamed_ctes",
            files={**CLEAN_ORDERS, **orders_model(_TWO_CALLS)},
            command=("lineage", "orders.amount_cents", "--format", "list"),
            expected_returncode=0,
            expected_fragments=("stg_orders.amount_cents -> orders.amount_cents",),
        ),
        SqlMacroCommandTestCase(
            description="contracts_type_columns_through_renamed_ctes",
            files={
                **CLEAN_ORDERS,
                **contract_model(
                    "orders", "order_id (type BIGINT), amount_cents (type BIGINT)", _TWO_CALLS
                ),
            },
            command=("compile", "--no-cache"),
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCommandTestCase(
            description="format_keeps_the_cte_argument",
            files={**CLEAN_ORDERS, **orders_model(_TWO_CALLS)},
            command=("format",),
            expected_returncode=0,
            expected_fragments=(
                "@sql.clean_orders(gb_raw, __cte := gb_orders)",
                "@sql.clean_orders(fr_raw, __cte := fr_orders)",
            ),
            inspected_paths=("models/marts/orders.sql",),
        ),
        SqlMacroCommandTestCase(
            description="lint_sees_renamed_ctes_as_hand_written",
            files={**CLEAN_ORDERS, **orders_model(_TWO_CALLS)},
            command=("check",),
            expected_returncode=0,
            expected_fragments=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cte_names_when_running_a_command_then_renamed_ctes_act_as_written(
    test_case: SqlMacroCommandTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=test_case.command
    )
    printed: str = output(result) + inspected_text(
        project_dir=project_dir, relative_paths=test_case.inspected_paths
    )

    assert result.returncode == test_case.expected_returncode, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed
