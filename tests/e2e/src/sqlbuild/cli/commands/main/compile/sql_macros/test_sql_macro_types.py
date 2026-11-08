"""E2E coverage for ANY SQL macro arguments and the declared returns type of a call."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroBuildTestCase,
    SqlMacroCommandTestCase,
    SqlMacroCompileTestCase,
    SqlMacroErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    DATABASE,
    contract_model,
    orders_model,
    output,
    run_project,
    text_macro,
    with_rules,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import query_duckdb, run_sqb

_ORDER_KEY: dict[str, str] = {
    "macros/sql/order_key.sql": (
        'MACRO (\n  description "Order key from the country and the order",\n'
        "  arguments (country_code VARCHAR, order_ref ANY),\n"
        "  returns VARCHAR,\n);\n\n"
        "@country_code || '::' || CAST(@order_ref AS VARCHAR)\n"
    )
}

_UNPROVEN: tuple[str, ...] = ("K003", "SQBRCONTRACT105")
_TYPED_COLUMNS: str = "order_id (type BIGINT), shouted (type VARCHAR), status_bar (type VARCHAR)"
_TYPED_RULES_AND_MACROS: dict[str, str] = {
    **with_rules("SQBRCONTRACT105"),
    **text_macro("shouted", "VARCHAR", "UPPER(@value)::VARCHAR"),
    **text_macro("status_bar", "VARCHAR", "CAST(bar(LENGTH(@value), 0, 10) AS VARCHAR)"),
}
_TYPED_CALLS: dict[str, str] = {
    **_TYPED_RULES_AND_MACROS,
    **contract_model(
        "orders",
        _TYPED_COLUMNS,
        "SELECT\n  o.order_id,\n  @sql.shouted(o.status) AS shouted,\n"
        "  @sql.status_bar(o.status) AS status_bar\n"
        'FROM __ref("stg_orders") AS o\n',
    ),
    **contract_model(
        "order_bars",
        "status_bar (type VARCHAR)",
        'SELECT b.status_bar FROM __ref("orders") AS b\n',
    ),
}

_UNION_CALLS: dict[str, str] = {
    **_TYPED_RULES_AND_MACROS,
    **contract_model(
        "orders",
        "order_id (type BIGINT), status_bar (type VARCHAR)",
        "WITH final AS (\n"
        "  SELECT CAST(o.order_id AS BIGINT) AS order_id,\n"
        "    @sql.status_bar(o.status) AS status_bar\n"
        '  FROM __ref("stg_orders") AS o\n'
        "  UNION ALL\n"
        "  SELECT CAST(o.order_id AS BIGINT) AS order_id,\n"
        "    CAST(NULL AS VARCHAR) AS status_bar\n"
        '  FROM __ref("stg_orders") AS o\n'
        ")\nSELECT f.order_id, f.status_bar FROM final AS f\n",
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroBuildTestCase(
            description="any_argument_takes_a_number_a_string_and_an_expression",
            files={
                **_ORDER_KEY,
                **orders_model(
                    "SELECT order_id,\n"
                    "  @sql.order_key(country_code, order_id) AS by_number,\n"
                    "  @sql.order_key(country_code, status) AS by_text,\n"
                    "  @sql.order_key(country_code, amount_cents / 100) AS by_expression\n"
                    'FROM __ref("stg_orders")\nWHERE order_id = 2\n'
                ),
            },
            query="SELECT by_number, by_text, by_expression FROM orders",
            expected_rows=[("FR::2", "FR::closed", "FR::5.0")],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_any_argument_when_building_then_callers_pass_values_of_any_type(
    test_case: SqlMacroBuildTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(tmp_path=tmp_path, files=test_case.files, command=("build",))

    assert result.returncode == 0, output(result)
    assert "P017" not in output(result)
    assert (
        query_duckdb(db_path=project_dir / DATABASE, sql=test_case.query) == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroErrorTestCase(
            description="a_list_passed_to_an_any_argument",
            files={
                **_ORDER_KEY,
                **orders_model(
                    'SELECT @sql.order_key(country_code, [1, 2]) AS k FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument order_ref of macro order_key() expects ANY: a scalar SQL expression",
            ),
        ),
        SqlMacroErrorTestCase(
            description="any_with_const",
            files={
                "macros/sql/order_key.sql": (
                    'MACRO (description "Key", arguments (order_ref ANY CONST), '
                    "returns VARCHAR);\n\nCAST(@order_ref AS VARCHAR)\n"
                ),
                **orders_model('SELECT @sql.order_key(1) AS k FROM __ref("stg_orders")\n'),
            },
            expected_code="P015",
            expected_fragments=(
                "argument order_ref ANY CONST: a CONST argument needs a type its values are "
                "checked against",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_any_argument_misused_when_compiling_then_it_fails(
    test_case: SqlMacroErrorTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == 1, output(result)
    assert f"error[{test_case.expected_code}]" in output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="a_main_query_call_and_a_downstream_passthrough",
            files=_TYPED_CALLS,
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
            unexpected_fragments=_UNPROVEN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_in_a_final_cte_read_with_a_star",
            files={
                **_TYPED_RULES_AND_MACROS,
                **contract_model(
                    "orders",
                    _TYPED_COLUMNS,
                    "WITH final AS (\n  SELECT\n    o.order_id,\n"
                    "    @sql.shouted(o.status) AS shouted,\n"
                    "    @sql.status_bar(o.status) AS status_bar\n"
                    '  FROM __ref("stg_orders") AS o\n)\nSELECT * FROM final\n',
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
            unexpected_fragments=_UNPROVEN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_through_a_two_cte_chain_and_a_join",
            files={
                **_TYPED_RULES_AND_MACROS,
                **contract_model(
                    "orders",
                    _TYPED_COLUMNS,
                    "WITH typed AS (\n  SELECT\n    o.order_id,\n"
                    "    @sql.shouted(o.status) AS shouted,\n"
                    "    @sql.status_bar(o.status) AS status_bar\n"
                    '  FROM __ref("stg_orders") AS o\n),\n'
                    "final AS (\n  SELECT t.order_id, t.shouted, b.status_bar\n"
                    "  FROM typed AS t JOIN typed AS b ON b.order_id = t.order_id\n)\n"
                    "SELECT * FROM final\n",
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
            unexpected_fragments=_UNPROVEN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_inside_a_larger_expression_is_typed_but_needs_a_cast_for_the_rule",
            files={
                **_TYPED_RULES_AND_MACROS,
                **contract_model(
                    "orders",
                    "order_id (type BIGINT), status_bar (type VARCHAR)",
                    "WITH final AS (\n  SELECT\n    o.order_id,\n"
                    "    COALESCE(@sql.status_bar(o.status), @sql.status_bar(o.status)) "
                    "AS status_bar\n"
                    '  FROM __ref("stg_orders") AS o\n)\nSELECT * FROM final\n',
                ),
            },
            expected_returncode=1,
            expected_fragments=(
                'error[SQBRCONTRACT105]: output "status_bar" is calculated without an outer '
                "explicit cast",
            ),
            unexpected_fragments=("K003",),
        ),
        SqlMacroCompileTestCase(
            description="a_time_body_disagrees_with_returns_at_the_macro_file",
            files={
                **text_macro("m", "VARCHAR", "CAST('10:00:00' AS TIME)"),
                **contract_model(
                    "orders",
                    "v (type VARCHAR)",
                    'SELECT @sql.m(o.status) AS v FROM __ref("stg_orders") AS o\n',
                ),
            },
            expected_returncode=1,
            expected_fragments=(
                "error[P024]: macro m() declares returns VARCHAR, but its body is TIME",
            ),
        ),
        SqlMacroCompileTestCase(
            description="a_longer_text_body_keeps_its_inferred_type",
            files={
                **text_macro("m", "VARCHAR(5)", "CAST(@value AS VARCHAR(100))"),
                **contract_model(
                    "orders",
                    "v (type VARCHAR(5))",
                    'SELECT @sql.m(o.status) AS v FROM __ref("stg_orders") AS o\n',
                ),
            },
            expected_returncode=1,
            expected_fragments=("error[K002]: column 'v' inferred as",),
        ),
        SqlMacroCompileTestCase(
            description="a_wider_decimal_body_keeps_its_inferred_type",
            files={
                **text_macro("m", "DECIMAL(4, 1)", "CAST(LENGTH(@value) AS DECIMAL(18, 6))"),
                **contract_model(
                    "orders",
                    "v (type DECIMAL(4, 1))",
                    'SELECT @sql.m(o.status) AS v FROM __ref("stg_orders") AS o\n',
                ),
            },
            expected_returncode=1,
            expected_fragments=("error[K002]: column 'v' inferred as",),
        ),
        SqlMacroCompileTestCase(
            description="a_timestamptz_body_keeps_its_inferred_type",
            files={
                **text_macro("m", "TIMESTAMP", "CAST(@value AS TIMESTAMPTZ)"),
                **contract_model(
                    "orders",
                    "v (type TIMESTAMP)",
                    'SELECT @sql.m(o.status) AS v FROM __ref("stg_orders") AS o\n',
                ),
            },
            expected_returncode=1,
            expected_fragments=("error[K002]: column 'v' inferred as",),
        ),
        SqlMacroCompileTestCase(
            description="an_unusual_outer_query_star_replace_still_compiles",
            files={
                **_TYPED_RULES_AND_MACROS,
                **orders_model(
                    "WITH final AS (\n"
                    "  SELECT o.order_id, @sql.shouted(o.status) AS shouted\n"
                    '  FROM __ref("stg_orders") AS o\n'
                    ")\n"
                    "SELECT * REPLACE (UPPER(shouted) AS shouted) FROM final\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCompileTestCase(
            description="an_unusual_outer_query_star_exclude_still_compiles",
            files={
                **_TYPED_RULES_AND_MACROS,
                **orders_model(
                    "WITH final AS (\n"
                    "  SELECT o.order_id, @sql.shouted(o.status) AS shouted\n"
                    '  FROM __ref("stg_orders") AS o\n'
                    ")\n"
                    "SELECT * EXCLUDE (order_id) FROM final\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCompileTestCase(
            description="an_unusual_outer_query_columns_expression_still_compiles",
            files={
                **_TYPED_RULES_AND_MACROS,
                **orders_model(
                    "WITH final AS (\n"
                    "  SELECT o.order_id, @sql.shouted(o.status) AS shouted\n"
                    '  FROM __ref("stg_orders") AS o\n'
                    ")\n"
                    "SELECT COLUMNS('shout.*') FROM final\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCompileTestCase(
            description="an_unusual_outer_query_star_rename_still_compiles",
            files={
                **_TYPED_RULES_AND_MACROS,
                **orders_model(
                    "WITH final AS (\n"
                    "  SELECT o.order_id, @sql.shouted(o.status) AS shouted\n"
                    '  FROM __ref("stg_orders") AS o\n'
                    ")\n"
                    "SELECT * RENAME (shouted AS loud) FROM final\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCompileTestCase(
            description="an_unusual_outer_query_full_join_using_still_compiles",
            files={
                **_TYPED_RULES_AND_MACROS,
                **orders_model(
                    "WITH final AS (\n"
                    "  SELECT o.order_id, @sql.shouted(o.status) AS shouted\n"
                    '  FROM __ref("stg_orders") AS o\n'
                    ")\n"
                    "SELECT f.* FROM final AS f FULL JOIN final AS g USING (order_id)\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCompileTestCase(
            description="a_call_in_a_union_branch_of_a_cte_types_the_union",
            files=_UNION_CALLS,
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
            unexpected_fragments=_UNPROVEN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_in_a_later_union_branch_of_the_main_query_types_the_union",
            files={
                **_TYPED_RULES_AND_MACROS,
                **contract_model(
                    "orders",
                    "status_bar (type VARCHAR)",
                    'SELECT CAST(NULL AS VARCHAR) AS status_bar FROM __ref("stg_orders") AS o\n'
                    "UNION ALL\n"
                    'SELECT @sql.status_bar(o.status) AS status_bar FROM __ref("stg_orders") AS o\n',
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
            unexpected_fragments=_UNPROVEN,
        ),
        SqlMacroCompileTestCase(
            description="a_union_sibling_of_another_known_type_still_reports",
            files={
                **_TYPED_RULES_AND_MACROS,
                **contract_model(
                    "orders",
                    "status_bar (type VARCHAR)",
                    "WITH final AS (\n"
                    '  SELECT @sql.status_bar(o.status) AS status_bar FROM __ref("stg_orders") AS o\n'
                    "  UNION ALL\n"
                    "  SELECT CAST(LENGTH(o.status) AS BIGINT) AS status_bar\n"
                    '  FROM __ref("stg_orders") AS o\n'
                    ")\nSELECT f.status_bar FROM final AS f\n",
                ),
            },
            expected_returncode=1,
            expected_fragments=('error[SQBRCONTRACT105]: output "status_bar"',),
        ),
        SqlMacroCompileTestCase(
            description="an_untyped_upstream_column_in_a_cte_takes_returns",
            files={
                **with_rules("SQBRCONTRACT105"),
                **text_macro("m", "BIGINT", "@value", argument="ANY"),
                "models/marts/up.sql": (
                    'MODEL (description "Up");\n\nSELECT NULL AS s FROM __ref("stg_orders") AS o\n'
                ),
                **contract_model(
                    "orders",
                    "v (type BIGINT)",
                    'WITH c AS (SELECT @sql.m(u.s) AS v FROM __ref("up") AS u)\nSELECT v FROM c\n',
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
            unexpected_fragments=_UNPROVEN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_in_a_later_main_query_branch_matching_its_sibling_is_typed",
            files={
                **with_rules("SQBRCONTRACT105"),
                **text_macro("m", "BIGINT", "CAST(NULL AS BIGINT)"),
                **contract_model(
                    "orders",
                    "v (type BIGINT)",
                    'SELECT CAST(NULL AS BIGINT) AS v FROM __ref("stg_orders") AS o\n'
                    "UNION ALL\n"
                    'SELECT @sql.m(o.status) AS v FROM __ref("stg_orders") AS o\n',
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
            unexpected_fragments=_UNPROVEN,
        ),
        SqlMacroCompileTestCase(
            description="an_inferred_type_of_another_family_disagrees_at_the_macro_file",
            files={
                **text_macro("shouted", "BIGINT", "UPPER(@value)"),
                **contract_model(
                    "orders",
                    "shouted (type BIGINT)",
                    "WITH final AS (\n"
                    '  SELECT @sql.shouted(o.status) AS shouted FROM __ref("stg_orders") AS o\n'
                    ")\nSELECT * FROM final\n",
                ),
            },
            expected_returncode=1,
            expected_fragments=(
                "error[P024]: macro shouted() declares returns BIGINT, but its body is VARCHAR",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_returns_type_when_checking_contracts_then_the_call_has_that_type(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result)
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output(result)


@pytest.mark.xfail(
    strict=True,
    reason="native DuckDB analysis does not type list literals yet",
)
@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="an_array_body_disagrees_with_returns_at_the_macro_file",
            files={
                **text_macro("m", "VARCHAR", "[@value]"),
                **contract_model(
                    "orders",
                    "v (type VARCHAR)",
                    'SELECT @sql.m(o.status) AS v FROM __ref("stg_orders") AS o\n',
                ),
            },
            expected_returncode=1,
            expected_fragments=(
                "error[P024]: macro m() declares returns VARCHAR, but its body is",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_list_body_when_checking_returns_then_the_mismatch_is_reported(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result)
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCommandTestCase(
            description="a_warm_compile_types_the_call_the_same_way",
            files=_TYPED_CALLS,
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCommandTestCase(
            description="a_warm_compile_types_a_union_branch_call_the_same_way",
            files=_UNION_CALLS,
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_returns_type_when_compiling_warm_then_the_call_keeps_that_type(
    test_case: SqlMacroCommandTestCase, tmp_path: Path
) -> None:
    project_dir, cold = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )
    warm: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project_dir
    )

    assert cold.returncode == 0, output(cold)
    assert warm.returncode == test_case.expected_returncode, output(warm)
    for fragment in test_case.expected_fragments:
        assert fragment in output(warm)
