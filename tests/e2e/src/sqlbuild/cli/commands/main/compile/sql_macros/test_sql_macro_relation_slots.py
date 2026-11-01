"""E2E coverage for RELATION slots: COLUMNS groups and slots mapped at the call."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroCommandTestCase,
    SqlMacroCompileTestCase,
    SqlMacroEquivalenceTestCase,
    SqlMacroErrorTestCase,
    SqlMacroFormatPairTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    BASE_FILES,
    BY_HAND,
    BY_SLOT,
    DATABASE,
    LINEAGE_MODEL,
    ORDER_COLUMNS,
    PER_PRODUCT_MACRO,
    PER_PRODUCT_REGION_MACRO,
    REFUND_RATE_MACRO,
    REFUNDS,
    SLOT_MACRO,
    UNFORMATTED_SLOT_READS,
    UNNAMED_COUNT,
    WEEKLY_ORDERS,
    hand_written,
    inspected_text,
    output,
    rate_model,
    refunded_weeks,
    run_project,
    slot_model,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import query_duckdb, run_sqb

_RATES_QUERY: str = "SELECT product_id, region, refund_rate FROM {table} ORDER BY ALL"
_RATE_TYPE_MISMATCH: str = (
    "error[K002]: column 'refund_rate' inferred as DOUBLE but declared type is VARCHAR"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroEquivalenceTestCase(
            description="columns_slot_at_two_grains_matches_per_grain_macros",
            files={
                **WEEKLY_ORDERS,
                **SLOT_MACRO,
                **PER_PRODUCT_MACRO,
                **PER_PRODUCT_REGION_MACRO,
                **BY_SLOT,
                **BY_HAND,
            },
            compared_paths=(
                "target/compiled/models/marts/rates_by_slot.sql",
                "target/compiled/models/marts/rates_by_hand.sql",
            ),
            expected_fragment=(
                "ROW_NUMBER() OVER (PARTITION BY product_id, region ORDER BY s.week DESC)"
            ),
            compared_queries=(
                _RATES_QUERY.format(table="rates_by_slot"),
                _RATES_QUERY.format(table="rates_by_hand"),
            ),
            expected_row_count=6,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_columns_slot_at_two_grains_when_building_then_it_matches_per_grain_macros(
    test_case: SqlMacroEquivalenceTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(tmp_path=tmp_path, files=test_case.files, command=("build",))

    assert result.returncode == 0, output(result)
    by_slot: str = inspected_text(
        project_dir=project_dir, relative_paths=test_case.compared_paths[:1]
    )
    by_hand: str = inspected_text(
        project_dir=project_dir, relative_paths=test_case.compared_paths[1:]
    )
    assert by_slot == by_hand
    assert test_case.expected_fragment in by_slot
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / DATABASE, sql=test_case.compared_queries[0]
    )
    assert rows == query_duckdb(db_path=project_dir / DATABASE, sql=test_case.compared_queries[1])
    assert len(rows) == test_case.expected_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroErrorTestCase(
            description="single_column_slot_mapped_to_a_missing_column",
            files={
                **WEEKLY_ORDERS,
                **SLOT_MACRO,
                **slot_model("weekly.keys := (product_id), weekly.order_count := placed"),
            },
            expected_code="P017",
            expected_fragments=(
                "error[P017]: slot weekly.order_count needs a BIGINT column; weekly has no column placed",
                "--> models/marts/rates.sql:4:1",
                "--> macros/sql/weekly_rates.sql:3:14",
                "help: map weekly.order_count to columns of weekly",
            ),
        ),
        SqlMacroErrorTestCase(
            description="single_column_slot_mapped_to_an_incompatible_column",
            files={
                **WEEKLY_ORDERS,
                **SLOT_MACRO,
                **slot_model("weekly.keys := (product_id), weekly.order_count := region"),
            },
            expected_code="P017",
            expected_fragments=(
                "error[P017]: slot weekly.order_count needs a BIGINT column; weekly.region is VARCHAR",
            ),
        ),
        SqlMacroErrorTestCase(
            description="columns_slot_naming_a_missing_column",
            files={
                **WEEKLY_ORDERS,
                **SLOT_MACRO,
                **slot_model(
                    "weekly.keys := (product_id, channel), weekly.order_count := orders_placed"
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "error[P017]: slot weekly.keys groups column channel; weekly has no column channel",
            ),
        ),
        SqlMacroErrorTestCase(
            description="unmapped_single_column_slot_reads_its_own_name",
            files={**WEEKLY_ORDERS, **SLOT_MACRO, **slot_model("weekly.keys := (product_id)")},
            expected_code="P017",
            expected_fragments=(
                "error[P017]: argument weekly of macro weekly_rates() needs column order_count BIGINT; weekly has no column order_count",
            ),
        ),
        SqlMacroErrorTestCase(
            description="unmapped_columns_slot",
            files={
                **WEEKLY_ORDERS,
                **SLOT_MACRO,
                **slot_model("weekly.order_count := orders_placed"),
            },
            expected_code="P017",
            expected_fragments=(
                "error[P017]: call to macro weekly_rates() is missing slot weekly.keys COLUMNS",
                "help: map it to the passed relation's columns, such as weekly.keys := (product_id)",
            ),
        ),
        SqlMacroErrorTestCase(
            description="grouped_column_that_also_fills_a_single_column_slot",
            files={
                **WEEKLY_ORDERS,
                **SLOT_MACRO,
                **slot_model(
                    "weekly.keys := (product_id, orders_placed), "
                    "weekly.order_count := orders_placed"
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "error[P017]: column orders_placed is in slot weekly.keys and also fills single-column slot weekly.order_count of macro weekly_rates(); a grouped column may not fill a single-column slot",
            ),
        ),
        SqlMacroErrorTestCase(
            description="grouped_column_named_like_a_column_the_macro_defines",
            files={
                **WEEKLY_ORDERS,
                **SLOT_MACRO,
                **slot_model(
                    "weekly.keys := (product_id, refunds), weekly.order_count := orders_placed"
                ),
            },
            expected_code="P018",
            expected_fragments=(
                "error[P018]: column refunds of slot weekly.keys clashes with column refunds that macro weekly_rates() defines",
            ),
        ),
        SqlMacroErrorTestCase(
            description="mapped_single_column_slot_read_without_as",
            files={
                **WEEKLY_ORDERS,
                **UNNAMED_COUNT,
                "models/marts/rates.sql": (
                    'MODEL (description "Order counts");\n\n'
                    "SELECT @sql.order_columns(w, weekly.order_count := orders_placed)\n"
                    'FROM __seed("weekly_orders") AS w\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "error[P017]: slot weekly.order_count of macro order_columns() is mapped to orders_placed, but the macro reads it in a select list without AS, so the output column's name would depend on the mapping",
                "help: add AS <name> to @weekly.order_count in macros/sql/order_columns.sql",
            ),
        ),
        SqlMacroErrorTestCase(
            description="columns_slot_outside_a_list",
            files={
                **WEEKLY_ORDERS,
                "macros/sql/weekly_rates.sql": (
                    'MACRO (description "Rates", arguments (weekly RELATION (keys COLUMNS)));\n\n'
                    "WITH rated AS (SELECT * FROM @weekly WHERE @weekly.keys IS NOT NULL)\n"
                ),
                **slot_model("weekly.keys := (product_id)"),
            },
            expected_code="P015",
            expected_fragments=(
                "error[P015]: COLUMNS slot weekly.keys is read as @weekly.keys in macro weekly_rates() where a list item cannot stand",
                "--> macros/sql/weekly_rates.sql:3:44",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_wrong_slot_mappings_when_compiling_then_the_error_names_the_slot(
    test_case: SqlMacroErrorTestCase, tmp_path: Path
) -> None:
    _, result = run_project(tmp_path=tmp_path, files=test_case.files, command=("compile",))
    printed: str = output(result)

    assert result.returncode == 1, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCommandTestCase(
            description="a_mapped_slot_traces_to_the_mapped_column",
            files={**WEEKLY_ORDERS, **ORDER_COLUMNS, **LINEAGE_MODEL},
            command=("lineage", "order_counts.orders", "--direction", "upstream"),
            expected_returncode=0,
            expected_fragments=("weekly_orders.orders_placed",),
        ),
        SqlMacroCommandTestCase(
            description="a_grouped_column_traces_to_its_own_column",
            files={**WEEKLY_ORDERS, **ORDER_COLUMNS, **LINEAGE_MODEL},
            command=("lineage", "order_counts.region", "--direction", "upstream"),
            expected_returncode=0,
            expected_fragments=("weekly_orders.region",),
        ),
        SqlMacroCommandTestCase(
            description="a_rule_sees_through_a_slot_read",
            files={
                **WEEKLY_ORDERS,
                "sqlbuild_project.toml": BASE_FILES["sqlbuild_project.toml"]
                + '\n[rules]\nselect = ["SQLCLEAN001"]\n',
                "macros/sql/order_columns.sql": (
                    'MACRO (\n  description "Order count column",\n'
                    "  arguments (weekly RELATION (order_count BIGINT)),\n);\n\n"
                    "SELECT @weekly.order_count = NULL AS has_no_orders\n"
                ),
                "models/marts/order_counts.sql": (
                    'MODEL (description "Order counts");\n\n'
                    "SELECT @sql.order_columns(w, weekly.order_count := orders_placed)\n"
                    'FROM __seed("weekly_orders") AS w\n'
                ),
            },
            command=("compile",),
            expected_returncode=1,
            expected_fragments=(
                "error[SQLCLEAN001]: Comparison with NULL is never true (in SQL generated by "
                "@sql.order_columns(w, weekly.order_count := orders_placed))",
                "--> models/marts/order_counts.sql:3:8",
            ),
        ),
        SqlMacroCommandTestCase(
            description="an_aliased_slot_read_named_after_its_slot_is_no_self_alias",
            files={
                **WEEKLY_ORDERS,
                "sqlbuild_project.toml": BASE_FILES["sqlbuild_project.toml"]
                + '\n[rules]\nselect = ["SQLCLEAN016"]\n',
                "macros/sql/order_columns.sql": (
                    'MACRO (\n  description "Order count column",\n'
                    "  arguments (weekly RELATION (order_count BIGINT)),\n);\n\n"
                    "WITH counted AS (SELECT s.@weekly.order_count AS order_count FROM @weekly AS s),\n"
                    "named AS (SELECT @weekly.order_count AS order_count FROM @weekly)\n"
                ),
                "models/marts/order_counts.sql": (
                    'MODEL (description "Order counts");\n\n'
                    'WITH w AS (SELECT * FROM __seed("weekly_orders")),\n'
                    "@sql.order_columns(w, weekly.order_count := orders_placed)\n"
                    "SELECT counted.order_count FROM counted, named\n"
                ),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("Project compile  OK",),
        ),
        SqlMacroCommandTestCase(
            description="the_same_alias_on_a_hand_written_column_is_a_self_alias",
            files={
                **WEEKLY_ORDERS,
                "sqlbuild_project.toml": BASE_FILES["sqlbuild_project.toml"]
                + '\n[rules]\nselect = ["SQLCLEAN016"]\n',
                "macros/sql/order_columns.sql": (
                    'MACRO (\n  description "Order count column",\n'
                    "  arguments (weekly RELATION (order_count BIGINT)),\n);\n\n"
                    "WITH counted AS (SELECT s.order_count AS order_count FROM @weekly AS s)\n"
                ),
                "models/marts/order_counts.sql": (
                    'MODEL (description "Order counts");\n\n'
                    'WITH w AS (SELECT orders_placed AS order_count FROM __seed("weekly_orders")),\n'
                    "@sql.order_columns(w)\n"
                    "SELECT counted.order_count FROM counted\n"
                ),
            },
            command=("compile",),
            expected_returncode=1,
            expected_fragments=(
                "SQLCLEAN016]: Expression is aliased to its existing name",
                "--> macros/sql/order_columns.sql:6:",
            ),
        ),
        SqlMacroCommandTestCase(
            description="a_plain_slot_read_ambiguous_in_a_join_fails_like_hand_written_sql",
            files={
                **WEEKLY_ORDERS,
                "macros/sql/joined_keys.sql": (
                    'MACRO (\n  description "Keys of joined weeks",\n'
                    "  arguments (weekly RELATION (keys COLUMNS), other RELATION (week BIGINT)),\n);\n\n"
                    "WITH joined AS (\n"
                    "  SELECT @weekly.keys\n"
                    "  FROM @weekly AS w\n"
                    "  JOIN @other AS o ON w.week = o.week\n"
                    ")\n"
                ),
                "models/marts/joined.sql": (
                    'MODEL (description "Joined weeks");\n\n'
                    'WITH weekly AS (SELECT * FROM __seed("weekly_orders")),\n'
                    "@sql.joined_keys(weekly, weekly, weekly.keys := (product_id))\n"
                    "SELECT * FROM joined\n"
                ),
            },
            command=("compile",),
            expected_returncode=1,
            expected_fragments=(
                "error[B003]: Ambiguous unqualified column 'product_id' found in 2 referenced tables",
                "--> models/marts/joined.sql:4:1",
            ),
        ),
        SqlMacroCommandTestCase(
            description="the_same_bare_name_written_by_hand_in_a_join",
            files={
                **WEEKLY_ORDERS,
                "models/marts/joined.sql": (
                    'MODEL (description "Joined weeks");\n\n'
                    'WITH weekly AS (SELECT * FROM __seed("weekly_orders")),\n'
                    "joined AS (\n"
                    "  SELECT product_id\n"
                    "  FROM weekly AS w\n"
                    "  JOIN weekly AS o ON w.week = o.week\n"
                    ")\n"
                    "SELECT * FROM joined\n"
                ),
            },
            command=("compile",),
            expected_returncode=1,
            expected_fragments=(
                "error[B003]: Ambiguous unqualified column 'product_id' found in 2 referenced tables",
                "--> models/marts/joined.sql:5:10",
            ),
        ),
        SqlMacroCommandTestCase(
            description="a_plain_group_read_in_a_correlated_subquery_that_captures_it",
            files={
                **WEEKLY_ORDERS,
                **REFUNDS,
                **refunded_weeks(
                    "SELECT s.@weekly.keys FROM @weekly AS s\n"
                    "WHERE EXISTS (SELECT @weekly.keys FROM @refunds AS r WHERE r.week = s.week)"
                ),
            },
            command=("compile",),
            expected_returncode=1,
            expected_fragments=(
                "error[P017]: slot weekly.keys column product_id would bind to refunds in this "
                "subquery; write alias.@weekly.keys",
                "--> models/marts/refunded.sql:5:1",
            ),
        ),
        SqlMacroCommandTestCase(
            description="the_same_subquery_reading_the_group_through_its_alias",
            files={
                **WEEKLY_ORDERS,
                **REFUNDS,
                **refunded_weeks(
                    "SELECT s.@weekly.keys FROM @weekly AS s\n"
                    "WHERE EXISTS (SELECT s.@weekly.keys FROM @refunds AS r WHERE r.week = s.week)"
                ),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("Project compiled",),
        ),
        SqlMacroCommandTestCase(
            description="a_plain_group_read_in_a_cte_chain",
            files={
                **WEEKLY_ORDERS,
                **REFUNDS,
                **refunded_weeks(
                    "SELECT s.@weekly.keys FROM @weekly AS s),\n"
                    "grouped AS (SELECT @weekly.keys FROM refunded GROUP BY @weekly.keys"
                ),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("Project compiled",),
        ),
        SqlMacroCommandTestCase(
            description="a_plain_group_read_beside_an_uncorrelated_subquery_and_in_a_derived_table",
            files={
                **WEEKLY_ORDERS,
                **REFUNDS,
                **refunded_weeks(
                    "SELECT t.@weekly.keys FROM (SELECT @weekly.keys, week FROM @weekly) AS t\n"
                    "WHERE t.week IN (SELECT r.week FROM @refunds AS r)"
                ),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("Project compiled",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_slot_reads_when_inspecting_then_tools_see_the_mapped_columns(
    test_case: SqlMacroCommandTestCase, tmp_path: Path
) -> None:
    _, result = run_project(tmp_path=tmp_path, files=test_case.files, command=test_case.command)
    printed: str = output(result)

    assert result.returncode == test_case.expected_returncode, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="expression_macro_reading_mapped_slots",
            files={
                **WEEKLY_ORDERS,
                **REFUND_RATE_MACRO,
                **rate_model("rates", "@sql.refund_rate(w, weekly.order_count := orders_placed)"),
            },
            expected_returncode=1,
            expected_fragments=(
                _RATE_TYPE_MISMATCH,
                "w.refund_count / NULLIF(w.orders_placed, 0) AS refund_rate",
            ),
            inspected_paths=("target/compiled/models/marts/rates.sql",),
        ),
        SqlMacroCompileTestCase(
            description="the_same_expression_written_by_hand",
            files={
                **WEEKLY_ORDERS,
                **rate_model("rates", "w.refund_count / NULLIF(w.orders_placed, 0)"),
            },
            expected_returncode=1,
            expected_fragments=(
                _RATE_TYPE_MISMATCH,
                "w.refund_count / NULLIF(w.orders_placed, 0) AS refund_rate",
            ),
            inspected_paths=("target/compiled/models/marts/rates.sql",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_expression_macro_reading_slots_when_compiling_then_it_types_like_hand_sql(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile",)
    )
    printed: str = output(result) + inspected_text(
        project_dir=project_dir, relative_paths=test_case.inspected_paths
    )

    assert result.returncode == test_case.expected_returncode, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroFormatPairTestCase(
            description="slot_reads_and_columns_in_one_cte",
            slot_text=UNFORMATTED_SLOT_READS,
            hand_text=hand_written(UNFORMATTED_SLOT_READS),
            expected_check_returncode=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_slot_reads_when_formatting_then_they_lay_out_like_hand_written_columns(
    test_case: SqlMacroFormatPairTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(
        tmp_path=tmp_path,
        files={
            "macros/sql/slots.sql": test_case.slot_text,
            "macros/sql/by_hand.sql": test_case.hand_text,
        },
        command=("format",),
    )
    checked: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "format", "--check"), project_dir=project_dir
    )

    assert result.returncode == 0, output(result)
    formatted: str = (project_dir / "macros/sql/slots.sql").read_text(encoding="utf-8")
    assert formatted != test_case.slot_text
    assert hand_written(formatted) == (project_dir / "macros/sql/by_hand.sql").read_text(
        encoding="utf-8"
    )
    assert checked.returncode == test_case.expected_check_returncode, output(checked)
