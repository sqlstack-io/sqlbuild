"""E2E matrix of SQL macro calls typed by their returns in every position a call can stand."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroCompileTestCase,
    SqlMacroLineageTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    BRACKET,
    adapter_project,
    contract_model,
    inspected_text,
    lineage_edges,
    orders_model,
    output,
    run_project,
    text_macro,
    typed_position_files,
    untyped_text_macro,
    with_ignored_rules,
    with_rules,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_FROM: str = 'FROM __ref("stg_orders") AS o'
_COMPILED: str = "target/compiled/models/marts/orders.sql"
_CLEAN: tuple[str, ...] = ("0 errors, 0 warnings",)
_MACROS: dict[str, str] = {
    **text_macro("status_bar", "VARCHAR", "bar(LENGTH(@value), 0, 10)"),
    **text_macro("same", "VARCHAR", "@value", argument="ANY"),
    **text_macro("shouted", "VARCHAR", "UPPER(@value)"),
}
_UNKNOWN_CALL: str = "@sql.status_bar(o.status)"
_KNOWN_CALL: str = "@sql.same(o.order_id)"
_DISAGREEING_POSITIONS: tuple[tuple[str, str, str, str], ...] = (
    ("main_whole_item", "contracted", f"SELECT {{m}} AS x {_FROM}\n", "VARCHAR"),
    (
        "cte_item_read_by_name",
        "contracted",
        f"WITH c AS (SELECT {{m}} AS x {_FROM})\nSELECT x FROM c\n",
        "VARCHAR",
    ),
    (
        "star_from_a_cte",
        "contracted",
        f"WITH c AS (SELECT {{m}} AS x {_FROM})\nSELECT * FROM c\n",
        "VARCHAR",
    ),
    (
        "derived_table",
        "contracted",
        f"SELECT s.x FROM (SELECT {{m}} AS x {_FROM}) AS s\n",
        "VARCHAR",
    ),
    (
        "nested_with",
        "contracted",
        f"WITH c AS (WITH d AS (SELECT {{m}} AS x {_FROM}) SELECT x FROM d)\nSELECT x FROM c\n",
        "VARCHAR",
    ),
    (
        "union_both_branches",
        "contracted",
        f"SELECT {{m}} AS x {_FROM}\nUNION ALL\nSELECT {{m}} AS x {_FROM}\n",
        "VARCHAR",
    ),
    (
        "after_a_star_modifier",
        "downstream",
        "WITH c AS (SELECT o.order_id, o.status " + _FROM + ")\n"
        "SELECT * EXCLUDE (order_id), {m} AS x FROM c AS o\n",
        "VARCHAR",
    ),
)
_AGREEING_POSITIONS: tuple[tuple[str, str, str, str], ...] = (
    (
        "scalar_subquery",
        "contracted",
        f"SELECT (SELECT {{m}} {_FROM} LIMIT 1) AS x\n",
        "VARCHAR",
    ),
    (
        "union_right_branch",
        "contracted",
        f"SELECT o.status AS x {_FROM}\nUNION ALL\nSELECT {{m}} AS x {_FROM}\n",
        "VARCHAR",
    ),
    (
        "union_left_branch",
        "contracted",
        f"SELECT {{m}} AS x {_FROM}\nUNION ALL\nSELECT o.status AS x {_FROM}\n",
        "VARCHAR",
    ),
    (
        "parenthesised_branch",
        "contracted",
        f"SELECT CAST(NULL AS VARCHAR) AS x {_FROM}\nUNION ALL\n(SELECT {{m}} AS x {_FROM})\n",
        "VARCHAR",
    ),
    ("arithmetic", "contracted", f"SELECT LENGTH({{m}}) + 1 AS x {_FROM}\n", "BIGINT"),
    ("coalesce", "contracted", f"SELECT COALESCE({{m}}, {{m}}) AS x {_FROM}\n", "VARCHAR"),
    (
        "case",
        "contracted",
        f"SELECT CASE WHEN o.order_id > 1 THEN {{m}} ELSE NULL END AS x {_FROM}\n",
        "VARCHAR",
    ),
    ("function_argument", "contracted", f"SELECT LENGTH({{m}}) AS x {_FROM}\n", "BIGINT"),
    ("aggregate_over_the_call", "contracted", f"SELECT MAX({{m}}) AS x {_FROM}\n", "VARCHAR"),
    (
        "window",
        "contracted",
        f"SELECT MAX({{m}}) OVER (ORDER BY o.order_id) AS x {_FROM}\n",
        "VARCHAR",
    ),
    ("unaliased_call", "uncontracted", f"SELECT {{m}} {_FROM}\n", "VARCHAR"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description=description,
            files={
                **_MACROS,
                **typed_position_files(shape, sql.replace("{m}", _UNKNOWN_CALL), declared),
                **with_ignored_rules(
                    "SQLCLEAN004",
                    "SQLCLEAN039",
                    reason="The matrix covers calls in derived tables and scalar subqueries.",
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        )
        for description, shape, sql, declared in _DISAGREEING_POSITIONS + _AGREEING_POSITIONS
    ],
    ids=lambda case: case.description,
)
def test_given_an_unknown_body_when_compiling_then_the_call_takes_its_returns_type(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result), output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description=description,
            files={
                **_MACROS,
                **typed_position_files(shape, sql.replace("{m}", _KNOWN_CALL), declared),
            },
            expected_returncode=1,
            expected_fragments=("error[K002]: column 'x' inferred as",),
        )
        for description, shape, sql, declared in _DISAGREEING_POSITIONS
    ],
    ids=lambda case: case.description,
)
def test_given_a_known_different_body_when_compiling_then_the_inferred_type_stands(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result), output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="aggregate_inside_the_body",
            files={
                **text_macro("summed", "BIGINT", "SUM(bar(LENGTH(@value), 0, 10))"),
                **contract_model(
                    "orders",
                    "status (type VARCHAR), x (type BIGINT)",
                    f"SELECT o.status, @sql.summed(o.status) AS x {_FROM}\nGROUP BY o.status\n",
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCompileTestCase(
            description="aggregate_inside_the_body_still_needs_group_by",
            files={
                **text_macro("summed", "BIGINT", "SUM(bar(LENGTH(@value), 0, 10))"),
                **orders_model(f"SELECT o.status, @sql.summed(o.status) AS x {_FROM}\n"),
            },
            expected_returncode=1,
            expected_fragments=("error[B230]",),
        ),
        SqlMacroCompileTestCase(
            description="a_nested_call_types_the_macro_calling_it",
            files={
                **_MACROS,
                **untyped_text_macro("outer_bar", "@sql.status_bar(@value)"),
                **contract_model(
                    "orders", "x (type VARCHAR)", f"SELECT @sql.outer_bar(o.status) AS x {_FROM}\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCompileTestCase(
            description="an_any_argument_of_unknown_type_takes_returns",
            files={
                **_MACROS,
                **contract_model(
                    "orders",
                    "x (type VARCHAR)",
                    f"SELECT @sql.same(bar(o.order_id, 0, 10)) AS x {_FROM}\n",
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCompileTestCase(
            description="a_known_identical_body",
            files={
                **_MACROS,
                **contract_model(
                    "orders", "x (type VARCHAR)", f"SELECT @sql.shouted(o.status) AS x {_FROM}\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCompileTestCase(
            description="a_call_inside_a_python_macro_argument_is_inferred_only",
            files={
                **_MACROS,
                **BRACKET,
                **contract_model(
                    "orders",
                    "x (type VARCHAR)",
                    f"SELECT @py.bracket(@sql.status_bar(o.status)) AS x {_FROM}\n",
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCompileTestCase(
            description="an_outer_cast_wins_and_satisfies_the_cast_rule",
            files={
                **with_rules("SQBRCONTRACT105"),
                **_MACROS,
                **contract_model(
                    "orders",
                    "x (type BIGINT)",
                    f"SELECT CAST(@sql.status_bar(o.status) AS BIGINT) AS x {_FROM}\n",
                ),
            },
            expected_returncode=0,
            expected_fragments=("0 errors, 0 warnings",),
        ),
        SqlMacroCompileTestCase(
            description="a_call_inside_arithmetic_needs_a_cast_for_the_cast_rule",
            files={
                **with_rules("SQBRCONTRACT105"),
                **_MACROS,
                **contract_model(
                    "orders",
                    "x (type BIGINT)",
                    f"SELECT LENGTH(@sql.status_bar(o.status)) + 1 AS x {_FROM}\n",
                ),
            },
            expected_returncode=1,
            expected_fragments=(
                'error[SQBRCONTRACT105]: output "x" is calculated without an outer explicit cast',
                "Wrap the complete expression in CAST(... AS BIGINT)",
            ),
        ),
        SqlMacroCompileTestCase(
            description="a_binding_error_after_a_call_is_located_where_it_was_written",
            files={
                **_MACROS,
                **orders_model(f"SELECT @sql.status_bar(o.status) AS x, o.missing {_FROM}\n"),
            },
            expected_returncode=1,
            expected_fragments=("--> models/marts/orders.sql:3:42",),
        ),
        SqlMacroCompileTestCase(
            description="the_reserved_analysis_function_is_rejected_in_a_model",
            files={**orders_model(f"SELECT __polyglot_typed(o.status, 'VARCHAR') AS x {_FROM}\n")},
            expected_returncode=1,
            expected_fragments=("uses __polyglot_typed, a name reserved for SQL analysis",),
        ),
        SqlMacroCompileTestCase(
            description="a_downstream_model_sees_the_returns_type",
            files={
                **_MACROS,
                **contract_model(
                    "orders", "x (type VARCHAR)", f"SELECT @sql.status_bar(o.status) AS x {_FROM}\n"
                ),
                **contract_model(
                    "order_labels", "x (type BIGINT)", 'SELECT b.x FROM __ref("orders") AS b\n'
                ),
            },
            expected_returncode=1,
            expected_fragments=("error[K002]: column 'x' inferred as",),
        ),
        SqlMacroCompileTestCase(
            description="a_grouping_key_written_through_a_call",
            files={
                **text_macro("plus_one", "BIGINT", "@value + 1", argument="BIGINT"),
                **orders_model(
                    f"SELECT o.amount_cents + 1 AS k, COUNT(*) AS n {_FROM} GROUP BY @sql.plus_one(o.amount_cents)\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_in_select_and_grouping_key",
            files={
                **text_macro("plus_one", "BIGINT", "@value + 1", argument="BIGINT"),
                **orders_model(
                    f"SELECT @sql.plus_one(o.amount_cents) AS k, COUNT(*) AS n {_FROM} GROUP BY @sql.plus_one(o.amount_cents)\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_selected_and_grouped_written_out",
            files={
                **text_macro("plus_one", "BIGINT", "@value + 1", argument="BIGINT"),
                **orders_model(
                    f"SELECT @sql.plus_one(o.amount_cents) AS k, COUNT(*) AS n {_FROM} GROUP BY o.amount_cents + 1\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_in_a_rollup",
            files={
                **text_macro("plus_one", "BIGINT", "@value + 1", argument="BIGINT"),
                **orders_model(
                    f"SELECT o.amount_cents + 1 AS k, COUNT(*) AS n {_FROM} GROUP BY ROLLUP (@sql.plus_one(o.amount_cents))\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        ),
        SqlMacroCompileTestCase(
            description="a_having_over_a_called_grouping_key",
            files={
                **text_macro("plus_one", "BIGINT", "@value + 1", argument="BIGINT"),
                **orders_model(
                    f"SELECT COUNT(*) AS n {_FROM} GROUP BY @sql.plus_one(o.amount_cents) HAVING o.amount_cents + 1 > 0\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        ),
        SqlMacroCompileTestCase(
            description="a_distinct_ordered_by_a_call",
            files={
                **text_macro("plus_one", "BIGINT", "@value + 1", argument="BIGINT"),
                **orders_model(
                    f"SELECT DISTINCT o.amount_cents + 1 AS k {_FROM} ORDER BY @sql.plus_one(o.amount_cents)\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        ),
        SqlMacroCompileTestCase(
            description="a_call_as_a_snowflake_dateadd_part",
            files={
                **adapter_project("snowflake"),
                **text_macro("p", "VARCHAR", "day"),
                **orders_model(f"SELECT DATEADD(@sql.p(o.status), 1, o.created_at) AS y {_FROM}\n"),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("__polyglot", "Semantic checks were partial"),
        ),
        SqlMacroCompileTestCase(
            description="a_call_as_a_snowflake_datediff_part",
            files={
                **adapter_project("snowflake"),
                **text_macro("p", "VARCHAR", "day"),
                **orders_model(
                    f"SELECT DATEDIFF(@sql.p(o.status), o.created_at, o.updated_at) AS y {_FROM}\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("__polyglot", "Semantic checks were partial"),
        ),
        SqlMacroCompileTestCase(
            description="a_call_as_an_interval_unit",
            files={
                **text_macro("p", "VARCHAR", "DAY"),
                **orders_model(f"SELECT o.created_at + INTERVAL 1 @sql.p(o.status) AS y {_FROM}\n"),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("__polyglot", "Semantic checks were partial"),
        ),
        SqlMacroCompileTestCase(
            description="a_call_as_a_bare_cast_type",
            files={
                **text_macro("t", "VARCHAR", "BIGINT"),
                **orders_model(f"SELECT CAST(o.order_id AS @sql.t(o.status)) AS y {_FROM}\n"),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("__polyglot", "Semantic checks were partial"),
        ),
        SqlMacroCompileTestCase(
            description="a_call_as_a_parameterised_cast_type",
            files={
                **text_macro("t", "VARCHAR", "DECIMAL(18, 2)"),
                **orders_model(f"SELECT CAST(o.order_id AS @sql.t(o.status)) AS y {_FROM}\n"),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("__polyglot", "Semantic checks were partial"),
        ),
        SqlMacroCompileTestCase(
            description="the_compiled_sql_has_no_marker",
            files={
                **_MACROS,
                **contract_model(
                    "orders", "x (type VARCHAR)", f"SELECT @sql.status_bar(o.status) AS x {_FROM}\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("bar(LENGTH(o.status), 0, 10) AS x",),
            unexpected_fragments=("__polyglot",),
            inspected_paths=(_COMPILED,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_typed_call_case_when_compiling_then_it_reports_as_expected(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )
    printed: str = output(result) + inspected_text(
        project_dir=project_dir, relative_paths=test_case.inspected_paths
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed
    for fragment in test_case.unexpected_fragments:
        assert fragment not in printed, printed


@pytest.mark.xfail(
    strict=True,
    reason="native DuckDB analysis reports EXTRACT, a DuckDB parser special form, as an unknown function",
)
@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="a_call_in_a_keyword_slot_keeps_the_analysis_whole",
            files={
                **text_macro("part", "VARCHAR", "YEAR"),
                **contract_model(
                    "orders",
                    "y (type BIGINT)",
                    f"SELECT EXTRACT(@sql.part(o.status) FROM o.created_at) AS y {_FROM}\n",
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("Semantic checks were partial",),
        ),
        SqlMacroCompileTestCase(
            description="an_extract_without_a_macro_compiles_clean",
            files=contract_model(
                "orders",
                "y (type BIGINT)",
                f"SELECT EXTRACT(YEAR FROM o.created_at) AS y {_FROM}\n",
            ),
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("Semantic checks were partial",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_extract_on_duckdb_when_compiling_then_it_compiles_clean(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result), output(result)
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output(result), output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="a_date_part_call_compiles_clean",
            files=contract_model(
                "orders",
                "y (type BIGINT)",
                f"SELECT date_part('year', o.created_at) AS y {_FROM}\n",
            ),
            expected_returncode=0,
            expected_fragments=_CLEAN,
            unexpected_fragments=("Semantic checks were partial",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_date_part_on_duckdb_when_compiling_then_it_compiles_clean(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result), output(result)
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output(result), output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="a_call_in_a_star_replace",
            files={
                **_MACROS,
                **contract_model(
                    "orders",
                    "order_id (type VARCHAR)",
                    "WITH c AS (SELECT o.order_id " + _FROM + ")\n"
                    "SELECT * REPLACE (@sql.status_bar(CAST(order_id AS VARCHAR)) AS order_id) "
                    "FROM c\n",
                ),
            },
            expected_returncode=0,
            expected_fragments=_CLEAN,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_call_in_a_star_replace_when_compiling_then_it_takes_its_returns_type(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result), output(result)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroLineageTestCase(
            description="an_identity_macro",
            macro_files={
                **text_macro("ident", "BIGINT", "@value", argument="BIGINT"),
                **orders_model(f"SELECT @sql.ident(o.order_id) AS x {_FROM}\n"),
            },
            inline_files=orders_model(f"SELECT o.order_id AS x {_FROM}\n"),
            column="orders.x",
            expected_edge="stg_orders.order_id -> orders.x",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_call_when_reading_lineage_then_it_matches_the_inline_sql(
    test_case: SqlMacroLineageTestCase, tmp_path: Path
) -> None:
    macro_dir, _ = run_project(
        tmp_path=tmp_path / "macro", files=test_case.macro_files, command=("compile", "--no-cache")
    )
    inline_dir, _ = run_project(
        tmp_path=tmp_path / "inline",
        files=test_case.inline_files,
        command=("compile", "--no-cache"),
    )
    lineage: tuple[str, ...] = ("--no-color", "lineage", test_case.column, "--format", "list")
    through_macro: subprocess.CompletedProcess[str] = run_sqb(
        command=lineage, project_dir=macro_dir
    )
    inline: subprocess.CompletedProcess[str] = run_sqb(command=lineage, project_dir=inline_dir)

    assert through_macro.returncode == 0, output(through_macro)
    assert test_case.expected_edge in through_macro.stdout, output(through_macro)
    assert lineage_edges(through_macro.stdout) == lineage_edges(inline.stdout), output(inline)
