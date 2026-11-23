"""E2E coverage for slot reads across scopes: forwarding, subqueries, LATERAL and clashes."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    SLOT_MACRO,
    inspected_text,
    macro_file,
    output,
    run_project,
    scoped_model,
)

_COMPILED: tuple[str, ...] = ("target/compiled/models/marts/scoped.sql",)
_CAPTURED: str = (
    "error[P017]: slot weekly.keys column product_id would bind to refunds in this subquery; "
    "write alias.@weekly.keys"
)
_JOINED_KEYS: dict[str, str] = macro_file(
    "joined_keys",
    "r RELATION (keys COLUMNS, wk BIGINT), other RELATION (week BIGINT)",
    "WITH joined AS (SELECT * FROM @r JOIN @other USING (@r.keys, @r.wk))",
)
_JOINED_WEEKS: dict[str, str] = macro_file(
    "joined_weeks",
    "r RELATION (keys COLUMNS, week BIGINT), other RELATION (week BIGINT)",
    "WITH joined AS (SELECT * FROM @r JOIN @other USING (@r.keys, @r.week))",
)
_KEYED_COUNTS: dict[str, str] = macro_file(
    "keyed_counts",
    "r RELATION (keys COLUMNS)",
    "WITH counted AS (SELECT s.@r.keys, COUNT(*) AS n FROM @r AS s GROUP BY s.@r.keys),\n"
    "listed AS (SELECT @r.keys, n FROM counted)",
)
_BUSY_ARGUMENTS: str = "weekly RELATION (order_count BIGINT, week BIGINT)"
_BUSY_CALL: str = "@sql.busy_weeks(weekly, weekly.order_count := orders_placed)\nSELECT * FROM busy"
_SCOPED_ARGUMENTS: str = (
    "weekly RELATION (keys COLUMNS, week BIGINT), refunds RELATION (week BIGINT)"
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="forwarded_using_read_mapped_to_a_column_that_is_no_outer_slot",
            files=scoped_model(
                {
                    **_JOINED_KEYS,
                    **macro_file(
                        "outer_keys",
                        "w RELATION (dims COLUMNS), o RELATION (week BIGINT)",
                        "WITH @sql.joined_keys(r := @w, other := @o, r.keys := (dims), "
                        "r.wk := week)",
                    ),
                },
                "@sql.outer_keys(weekly, refunds, w.dims := (product_id))\nSELECT * FROM joined",
            ),
            expected_returncode=0,
            expected_fragments=("USING (product_id, week)",),
            unexpected_fragments=("weekly.week",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="forwarded_using_read_of_an_unmapped_slot",
            files=scoped_model(
                {
                    **_JOINED_WEEKS,
                    **macro_file(
                        "outer_weeks",
                        "w RELATION (dims COLUMNS), o RELATION (week BIGINT)",
                        "WITH @sql.joined_weeks(r := @w, other := @o, r.keys := (dims))",
                    ),
                },
                "@sql.outer_weeks(weekly, refunds, w.dims := (product_id))\nSELECT * FROM joined",
            ),
            expected_returncode=0,
            expected_fragments=("USING (product_id, week)",),
            unexpected_fragments=("weekly.week",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="forwarded_group_mapped_to_real_columns_keeps_its_kind",
            files=scoped_model(
                {
                    **_KEYED_COUNTS,
                    **macro_file(
                        "outer_counts",
                        "w RELATION (week BIGINT)",
                        "WITH @sql.keyed_counts(r := @w, r.keys := (product_id, region))",
                    ),
                },
                "@sql.outer_counts(weekly)\nSELECT * FROM listed",
            ),
            expected_returncode=0,
            expected_fragments=(
                "SELECT s.product_id, s.region, COUNT(*) AS n FROM weekly AS s "
                "GROUP BY s.product_id, s.region",
                "SELECT product_id, region, n FROM counted",
            ),
            unexpected_fragments=("weekly.product_id", "P022"),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="forwarded_aliased_read_of_a_slot_the_outer_does_not_declare",
            files=scoped_model(
                {
                    **macro_file(
                        "counted_weeks",
                        "r RELATION (order_count BIGINT, week BIGINT)",
                        "WITH counted AS (SELECT s.@r.week, s.@r.order_count AS orders "
                        "FROM @r AS s)",
                    ),
                    **macro_file(
                        "outer_counted",
                        "w RELATION (placed BIGINT)",
                        "WITH @sql.counted_weeks(r := @w, r.order_count := placed)",
                    ),
                },
                "@sql.outer_counted(weekly, w.placed := orders_placed)\nSELECT * FROM counted",
            ),
            expected_returncode=0,
            expected_fragments=("SELECT s.week, s.orders_placed AS orders FROM weekly AS s",),
            unexpected_fragments=("P022",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="forwarded_group_mapped_to_a_single_column_outer_slot",
            files=scoped_model(
                {
                    **_KEYED_COUNTS,
                    **macro_file(
                        "outer_counts",
                        "w RELATION (week BIGINT)",
                        "WITH @sql.keyed_counts(r := @w, r.keys := (week))",
                    ),
                },
                "@sql.outer_counts(weekly)\nSELECT * FROM listed",
            ),
            expected_returncode=1,
            expected_fragments=(
                "slot r.keys of macro keyed_counts() is a COLUMNS slot, but the call maps it to "
                "w.week, a single-column slot of the forwarded relation w",
            ),
        ),
        SqlMacroCompileTestCase(
            description="plain_read_of_an_undeclared_column_in_using",
            files=scoped_model(
                macro_file(
                    "joined_on_week",
                    "r RELATION (keys COLUMNS), other RELATION (refund_count BIGINT)",
                    "WITH joined AS (SELECT @r.keys FROM @r JOIN @other USING (@r.week))",
                ),
                "@sql.joined_on_week(weekly, refunds, r.keys := (region))\nSELECT * FROM joined",
            ),
            expected_returncode=0,
            expected_fragments=("USING (week)",),
            unexpected_fragments=("weekly.week",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="forwarded_slots_mapped_to_outer_slot_names",
            files=scoped_model(
                {
                    **SLOT_MACRO,
                    **macro_file(
                        "outer_rates",
                        "w RELATION (dims COLUMNS, placed BIGINT)",
                        "WITH @sql.weekly_rates(weekly := @w, weekly.keys := (dims), "
                        "weekly.order_count := placed)",
                    ),
                },
                "@sql.outer_rates(weekly, w.dims := (product_id), w.placed := orders_placed)\n"
                "SELECT * FROM rates_out",
            ),
            expected_returncode=0,
            expected_fragments=(
                "s.orders_placed AS orders",
                "PARTITION BY product_id ORDER BY s.week DESC",
                "GROUP BY product_id",
            ),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="mapped_slot_without_as_in_an_in_subquery",
            files=scoped_model(
                macro_file(
                    "busy_weeks",
                    _BUSY_ARGUMENTS,
                    "WITH busy AS (SELECT s.week AS week FROM @weekly AS s\n"
                    "WHERE s.@weekly.order_count IN (SELECT w.@weekly.order_count FROM @weekly AS w))",
                ),
                _BUSY_CALL,
            ),
            expected_returncode=0,
            expected_fragments=("IN (SELECT w.orders_placed FROM weekly AS w)",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="mapped_slot_without_as_in_an_exists_subquery",
            files=scoped_model(
                macro_file(
                    "busy_weeks",
                    _BUSY_ARGUMENTS,
                    "WITH busy AS (SELECT s.week AS week FROM @weekly AS s\n"
                    "WHERE EXISTS (SELECT w.@weekly.order_count FROM @weekly AS w "
                    "WHERE w.week = s.week))",
                ),
                _BUSY_CALL,
            ),
            expected_returncode=0,
            expected_fragments=("EXISTS (SELECT w.orders_placed FROM weekly AS w",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="mapped_slot_without_as_in_a_scalar_subquery",
            files=scoped_model(
                macro_file(
                    "busy_weeks",
                    _BUSY_ARGUMENTS,
                    "WITH busy AS (SELECT s.week AS week,\n"
                    "(SELECT MAX(w.@weekly.order_count) FROM @weekly AS w) AS most FROM @weekly AS s)",
                ),
                _BUSY_CALL,
            ),
            expected_returncode=0,
            expected_fragments=("(SELECT MAX(w.orders_placed) FROM weekly AS w) AS most",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="plain_group_read_in_a_comma_lateral_subquery_that_captures_it",
            files=scoped_model(
                macro_file(
                    "lateral_counts",
                    _SCOPED_ARGUMENTS,
                    "WITH counted AS (SELECT s.@weekly.keys, l.n FROM @weekly AS s,\n"
                    "LATERAL (SELECT COUNT(*) AS n FROM @refunds AS r WHERE r.week = s.week "
                    "GROUP BY @weekly.keys) AS l)",
                ),
                "@sql.lateral_counts(weekly, refunds, weekly.keys := (product_id))\n"
                "SELECT * FROM counted",
            ),
            expected_returncode=1,
            expected_fragments=(_CAPTURED, "--> sql/models/marts/scoped.sql:5:1"),
        ),
        SqlMacroCompileTestCase(
            description="plain_group_read_in_a_join_lateral_subquery_that_captures_it",
            files=scoped_model(
                macro_file(
                    "lateral_counts",
                    _SCOPED_ARGUMENTS,
                    "WITH counted AS (SELECT s.@weekly.keys, l.n FROM @weekly AS s\n"
                    "JOIN LATERAL (SELECT COUNT(*) AS n FROM @refunds AS r WHERE r.week = s.week "
                    "GROUP BY @weekly.keys) AS l ON TRUE)",
                ),
                "@sql.lateral_counts(weekly, refunds, weekly.keys := (product_id))\n"
                "SELECT * FROM counted",
            ),
            expected_returncode=1,
            expected_fragments=(_CAPTURED, "--> sql/models/marts/scoped.sql:5:1"),
        ),
        SqlMacroCompileTestCase(
            description="plain_group_read_in_a_subquery_over_the_passed_relation",
            files=scoped_model(
                macro_file(
                    "max_weeks",
                    _SCOPED_ARGUMENTS,
                    "WITH latest AS (SELECT s.@weekly.keys FROM @weekly AS s\n"
                    "WHERE s.week IN (SELECT MAX(week) FROM @weekly GROUP BY @weekly.keys))",
                ),
                "@sql.max_weeks(weekly, refunds, weekly.keys := (product_id))\n"
                "SELECT * FROM latest",
            ),
            expected_returncode=0,
            expected_fragments=("IN (SELECT MAX(week) FROM weekly GROUP BY product_id)",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="plain_group_read_in_a_subquery_over_the_passed_and_another_relation",
            files=scoped_model(
                macro_file(
                    "max_weeks",
                    _SCOPED_ARGUMENTS,
                    "WITH latest AS (SELECT s.@weekly.keys FROM @weekly AS s\n"
                    "WHERE s.week IN (SELECT MAX(r.week) FROM @weekly AS v JOIN @refunds AS r "
                    "ON v.week = r.week GROUP BY @weekly.keys))",
                ),
                "@sql.max_weeks(weekly, refunds, weekly.keys := (product_id))\n"
                "SELECT * FROM latest",
            ),
            expected_returncode=1,
            expected_fragments=(_CAPTURED, "--> sql/models/marts/scoped.sql:5:1"),
        ),
        SqlMacroCompileTestCase(
            description="aliased_group_read_in_a_column_list_macro",
            files=scoped_model(
                macro_file(
                    "order_columns",
                    "weekly RELATION (keys COLUMNS, order_count BIGINT)",
                    "SELECT o.@weekly.keys, o.@weekly.order_count AS orders",
                ),
                "counted AS (\n"
                "  SELECT @sql.order_columns(weekly, weekly.keys := (product_id, region), "
                "weekly.order_count := orders_placed)\n"
                "  FROM weekly AS o\n"
                ")\n"
                "SELECT * FROM counted",
            ),
            expected_returncode=0,
            expected_fragments=("o.product_id, o.region, o.orders_placed AS orders",),
            inspected_paths=_COMPILED,
        ),
        SqlMacroCompileTestCase(
            description="grouped_column_named_like_a_bare_column_of_the_same_select",
            files=scoped_model(
                macro_file(
                    "listed_weeks",
                    "weekly RELATION (keys COLUMNS)",
                    "WITH listed AS (SELECT s.week, s.@weekly.keys FROM @weekly AS s)",
                ),
                "@sql.listed_weeks(weekly, weekly.keys := (week, product_id))\n"
                "SELECT * FROM listed",
            ),
            expected_returncode=1,
            expected_fragments=(
                "error[P018]: column week of slot weekly.keys clashes with column week that "
                "macro listed_weeks() defines",
            ),
        ),
        SqlMacroCompileTestCase(
            description="relation_read_of_a_column_beside_a_group_holding_it",
            files=scoped_model(
                macro_file(
                    "listed_regions",
                    "weekly RELATION (keys COLUMNS)",
                    "WITH listed AS (SELECT @weekly.keys, @weekly.region FROM @weekly)",
                ),
                "@sql.listed_regions(weekly, weekly.keys := (product_id, region))\n"
                "SELECT * FROM listed",
            ),
            expected_returncode=1,
            expected_fragments=(
                "error[P018]: column region of slot weekly.keys clashes with column region that "
                "macro listed_regions() defines",
            ),
        ),
        SqlMacroCompileTestCase(
            description="mapped_slot_without_as_in_a_lateral_subquery",
            files=scoped_model(
                macro_file(
                    "lateral_orders",
                    "weekly RELATION (order_count BIGINT, week BIGINT), refunds RELATION (week BIGINT)",
                    "WITH latest AS (SELECT r.week, l.* FROM @refunds AS r,\n"
                    "LATERAL (SELECT w.@weekly.order_count FROM @weekly AS w "
                    "WHERE w.week = r.week) AS l)",
                ),
                "@sql.lateral_orders(weekly, refunds, weekly.order_count := orders_placed)\n"
                "SELECT * FROM latest",
            ),
            expected_returncode=1,
            expected_fragments=(
                "error[P017]: slot weekly.order_count of macro lateral_orders() is mapped to "
                "orders_placed, but the macro reads it in a select list without AS",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_slot_reads_across_scopes_when_compiling_then_each_resolves_or_fails_clearly(
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
    for fragment in test_case.unexpected_fragments:
        assert fragment not in printed, printed
