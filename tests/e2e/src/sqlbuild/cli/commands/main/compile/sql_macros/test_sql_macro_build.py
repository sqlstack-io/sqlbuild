"""E2E coverage for building, formatting and inspecting models that call SQL macros."""

from __future__ import annotations

import subprocess
from decimal import Decimal
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroBuildTestCase,
    SqlMacroCommandTestCase,
    SqlMacroEditTestCase,
    SqlMacroFormatTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    ALL_MACROS,
    AUDIT_COLUMNS,
    BRACKET,
    CENTS_TO_DOLLARS,
    CLEAN_ORDERS,
    DATABASE,
    IN_SUPPORTED_COUNTRY,
    LATEST_PER_KEY,
    inspected_text,
    orders_model,
    output,
    run_project,
    with_ignored_rules,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import query_duckdb, run_sqb

_UNION_COLUMNS: dict[str, str] = {
    "macros/sql/order_columns.sql": (
        'MACRO (description "Order columns in a fixed order", arguments (src RELATION '
        "(order_id BIGINT, status VARCHAR)));\n\nSELECT @src.order_id, @src.status\n"
    )
}
_HAS_TAG: dict[str, str] = {
    "macros/sql/has_tag.sql": (
        'MACRO (description "Whether the tags hold a tag", arguments (tags VARCHAR[], '
        "tag VARCHAR CONST), returns BOOLEAN);\n\nlist_contains(@tags, @tag)\n"
    )
}
_STATUS_LABEL: dict[str, str] = {
    "macros/sql/status_label.sql": (
        'MACRO (description "Status label", arguments (status VARCHAR CONST), returns VARCHAR);'
        "\n\n'status: ' || @status\n"
    )
}
_FIRST_ORDERS: dict[str, str] = {
    "macros/sql/first_orders.sql": (
        'MACRO (description "First orders", arguments (src RELATION (order_id BIGINT), '
        "n BIGINT CONST));\n\nWITH firsts AS (SELECT s.order_id FROM @src AS s "
        "ORDER BY s.order_id LIMIT @n)\n"
    )
}
_IS_STATUS: dict[str, str] = {
    "macros/sql/is_status.sql": (
        'MACRO (description "Whether the status column matches", arguments (status VARCHAR), '
        "returns BOOLEAN);\n\nstatus = @status\n"
    )
}
_SCOPED_DOUBLED: dict[str, str] = {
    "models/marts/_sqlbuild/_macros/sql/doubled.sql": (
        'MACRO (description "Doubled", arguments (v BIGINT), returns BIGINT);\n\n@v * 2\n'
    )
}

_RAW: str = 'WITH orders_raw AS (SELECT * FROM __ref("stg_orders"))'
_CLEAN: str = "@sql.clean_orders(orders_raw)"
_FIRST: str = "@sql.first_orders(latest, 2)"
_FINAL: str = "final AS (SELECT order_id FROM firsts)"
_CTE_LIST_SPELLINGS: tuple[tuple[str, str], ...] = (
    ("no_commas", f"{_RAW}\n{_CLEAN}\n{_FIRST}\n{_FINAL}\nSELECT * FROM final\n"),
    ("commas_after", f"{_RAW},\n{_CLEAN},\n{_FIRST},\n{_FINAL}\nSELECT * FROM final\n"),
    ("commas_before", f"{_RAW}\n, {_CLEAN}\n, {_FIRST}\n, {_FINAL}\nSELECT * FROM final\n"),
    ("commas_on_both_sides", f"{_RAW},\n{_CLEAN}, {_FIRST},\n{_FINAL}\nSELECT * FROM final\n"),
    (
        "last_call_before_the_query_with_a_comma",
        f"{_RAW},\n{_CLEAN},\n{_FIRST},\nSELECT order_id FROM firsts\n",
    ),
    (
        "last_call_before_the_query_without_a_comma",
        f"{_RAW},\n{_CLEAN}\n{_FIRST}\nSELECT order_id FROM firsts\n",
    ),
)
_REFUNDS: dict[str, str] = {
    "seeds/raw_refunds.csv": "refund_id,status\n1,250\n",
    "seeds/raw_refunds.yml": (
        "seeds:\n  - name: raw_refunds\n    description: Raw refunds.\n    columns:\n"
        "      - name: refund_id\n        type: BIGINT\n"
        "      - name: status\n        type: BIGINT\n"
    ),
    "models/staging/stg_refunds.sql": (
        'MODEL (description "Staged refunds");\n\nSELECT * FROM __seed("raw_refunds")\n'
    ),
}
_PLUS_ONE_HOUR: dict[str, str] = {
    "macros/sql/plus_one_hour.sql": (
        'MACRO (description "One hour later", arguments (ts TIMESTAMP), returns TIMESTAMP);'
        "\n\n@ts + INTERVAL 1 HOUR\n"
    )
}
_BODY_SCOPED: dict[str, str] = {
    "models/marts/_sqlbuild/_constants/fee_divisor.sql": "CONSTANT (name fee_divisor, value 100);\n",
    "models/marts/_sqlbuild/macros/sql/fee_dollars.sql": (
        'MACRO (description "Fee in dollars", arguments (cents BIGINT), returns DOUBLE);'
        '\n\n@cents / @const("fee_divisor")\n'
    ),
    "models/marts/daily/orders.sql": (
        'MODEL (description "Orders");\n\nSELECT DISTINCT order_id, @sql.fee_dollars(fee_cents) '
        'AS fee FROM __ref("stg_orders") WHERE order_id = 2\n'
    ),
}
_NESTED_LOCAL: dict[str, str] = {
    "models/marts/_sqlbuild/macros/sql/doubled_fee.sql": (
        'MACRO (description "Fee doubled", arguments (cents BIGINT), returns BIGINT);'
        "\n\n@sql.doubled_inner(@cents)\n"
    ),
    "models/marts/_sqlbuild/_macros/sql/doubled_inner.sql": (
        'MACRO (description "Doubled", arguments (v BIGINT), returns BIGINT);\n\n@v * 2\n'
    ),
    "models/marts/daily/orders.sql": (
        'MODEL (description "Orders");\n\nSELECT DISTINCT order_id, @sql.doubled_fee(fee_cents) '
        'AS fee FROM __ref("stg_orders") WHERE order_id = 2\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroBuildTestCase(
            description="expression_macros_with_const_defaults_nesting_and_python_composition",
            files={
                **CENTS_TO_DOLLARS,
                **ALL_MACROS,
                **orders_model(
                    "SELECT\n  o.order_id,\n"
                    "  @sql.cents_to_dollars(o.amount_cents) AS amount,\n"
                    "  @sql.cents_to_dollars(o.fee_cents + o.tax_cents, places := 0) AS fees,\n"
                    "  @sql.net_amount(o.amount_cents, o.fee_cents) AS net,\n"
                    "  @py.bracket(@sql.cents_to_dollars(o.amount_cents)) AS label\n"
                    'FROM __ref("stg_orders") AS o\nWHERE o.order_id = 2\n'
                ),
            },
            query="SELECT order_id, amount, fees, net, label FROM orders",
            expected_rows=[(2, Decimal("5.00"), Decimal("1.00"), Decimal("4.50"), "[5.00]")],
        ),
        SqlMacroBuildTestCase(
            description="cte_list_and_column_list_macros_with_enum_and_list_constant_arguments",
            files={
                **ALL_MACROS,
                **orders_model(
                    'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\n'
                    "@sql.clean_orders(orders_raw)\n"
                    "final AS (\n  SELECT o.order_id, @sql.audit_columns(o)\n  FROM latest AS o\n"
                    "  WHERE @sql.is_open_status(o.status)\n"
                    '    AND @sql.in_supported_country(o.country_code, countries := @const("supported_countries"))\n'
                    ")\nSELECT * FROM final\n"
                ),
            },
            query="SELECT order_id, CAST(updated_at AS VARCHAR), was_updated FROM orders",
            expected_rows=[(1, "2026-01-03 00:00:00", True)],
        ),
        SqlMacroBuildTestCase(
            description="columns_slot_and_list_default",
            files={
                **LATEST_PER_KEY,
                **IN_SUPPORTED_COUNTRY,
                **orders_model(
                    'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\n'
                    "@sql.latest_per_key(orders_raw, src.keys := (status)),\n"
                    "final AS (SELECT status, order_id FROM latest_rows "
                    "WHERE @sql.in_supported_country(country_code))\n"
                    "SELECT * FROM final\n"
                ),
            },
            query="SELECT status, order_id FROM orders ORDER BY status",
            expected_rows=[("closed", 2)],
        ),
        SqlMacroBuildTestCase(
            description="column_list_repeated_across_union_all_branches",
            files={
                **_UNION_COLUMNS,
                **orders_model(
                    'WITH o AS (SELECT * FROM __ref("stg_orders"))\n'
                    "SELECT @sql.order_columns(o) FROM o WHERE o.status = 'open'\n"
                    "UNION ALL\n"
                    "SELECT @sql.order_columns(o) FROM o WHERE o.status = 'closed'\n"
                ),
            },
            query="SELECT order_id, status FROM orders ORDER BY order_id, status",
            expected_rows=[(1, "open"), (1, "open"), (2, "closed"), (3, "open")],
        ),
        SqlMacroBuildTestCase(
            description="runtime_array_parameter_and_const_scalar",
            files={
                **_HAS_TAG,
                **orders_model(
                    "WITH tagged AS (SELECT 1 AS order_id, ['vip', 'gift'] AS tags "
                    "UNION ALL SELECT 2, ['gift'])\n"
                    "SELECT order_id, @sql.has_tag(tags, 'vip') AS is_vip FROM tagged\n"
                ),
            },
            query="SELECT order_id, is_vip FROM orders ORDER BY order_id",
            expected_rows=[(1, True), (2, False)],
        ),
        SqlMacroBuildTestCase(
            description="const_argument_takes_an_enum_value",
            files={
                **_STATUS_LABEL,
                **orders_model('SELECT @sql.status_label(@enum("order_status").OPEN) AS label\n'),
            },
            query="SELECT label FROM orders",
            expected_rows=[("status: open",)],
        ),
        SqlMacroBuildTestCase(
            description="const_parameter_as_limit_and_aliased_relation",
            files={
                **_FIRST_ORDERS,
                **orders_model(
                    'WITH o AS (SELECT DISTINCT order_id FROM __ref("stg_orders")),\n'
                    "@sql.first_orders(o, 2)\nSELECT order_id FROM firsts\n"
                ),
            },
            query="SELECT order_id FROM orders ORDER BY order_id",
            expected_rows=[(1,), (2,)],
        ),
        SqlMacroBuildTestCase(
            description="column_and_parameter_sharing_a_name",
            files={
                **_IS_STATUS,
                **orders_model(
                    "SELECT DISTINCT order_id, @sql.is_status('open') AS is_open "
                    'FROM __ref("stg_orders")\n'
                ),
            },
            query="SELECT order_id, is_open FROM orders ORDER BY order_id",
            expected_rows=[(1, True), (2, False), (3, True)],
        ),
        SqlMacroBuildTestCase(
            description="macro_scoped_to_a_model_folder",
            files={
                **_SCOPED_DOUBLED,
                **orders_model(
                    'SELECT DISTINCT order_id, @sql.doubled(order_id) AS doubled FROM __ref("stg_orders")\n'
                ),
            },
            query="SELECT order_id, doubled FROM orders ORDER BY order_id",
            expected_rows=[(1, 2), (2, 4), (3, 6)],
        ),
        SqlMacroBuildTestCase(
            description="column_list_call_without_a_comma_before_it",
            files={
                **AUDIT_COLUMNS,
                **orders_model(
                    'SELECT DISTINCT o.order_id @sql.audit_columns(o) FROM __ref("stg_orders") AS o '
                    "WHERE o.order_id = 2\n"
                ),
            },
            query="SELECT order_id, was_updated FROM orders",
            expected_rows=[(2, False)],
        ),
        SqlMacroBuildTestCase(
            description="column_list_call_without_a_comma_after_it",
            files={
                **AUDIT_COLUMNS,
                **orders_model(
                    'SELECT DISTINCT @sql.audit_columns(o) o.order_id FROM __ref("stg_orders") AS o '
                    "WHERE o.order_id = 2\n"
                ),
            },
            query="SELECT order_id, was_updated FROM orders",
            expected_rows=[(2, False)],
        ),
        SqlMacroBuildTestCase(
            description="expression_call_before_at_time_zone_is_parenthesised",
            files={
                **_PLUS_ONE_HOUR,
                **orders_model(
                    "SELECT DISTINCT order_id, CAST(@sql.plus_one_hour(created_at) AT TIME ZONE 'UTC' "
                    'AS VARCHAR) AS later FROM __ref("stg_orders") WHERE order_id = 2\n'
                ),
            },
            query="SELECT order_id, later LIKE '2026-01-01 01:00:00%' FROM orders",
            expected_rows=[(2, True)],
        ),
        SqlMacroBuildTestCase(
            description="relation_argument_with_a_star_over_unnest_is_not_checked",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    "WITH orders_raw AS (\n"
                    '  SELECT o.order_id, o.updated_at, t.* FROM __ref("stg_orders") AS o\n'
                    "  CROSS JOIN unnest([false]) AS t(is_deleted)\n),\n"
                    "@sql.clean_orders(orders_raw)\nSELECT order_id FROM latest\n"
                ),
            },
            query="SELECT order_id FROM orders ORDER BY order_id",
            expected_rows=[(1,), (2,), (3,)],
        ),
        SqlMacroBuildTestCase(
            description="relation_argument_with_a_star_over_a_table_function_is_not_checked",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    "WITH orders_raw AS (\n"
                    '  SELECT o.order_id, o.updated_at, t.* FROM __ref("stg_orders") AS o\n'
                    "  CROSS JOIN range(1) AS t(is_deleted)\n),\n"
                    "@sql.clean_orders(orders_raw)\nSELECT order_id FROM latest\n"
                ),
            },
            query="SELECT order_id FROM orders ORDER BY order_id",
            expected_rows=[(1,), (2,), (3,)],
        ),
        SqlMacroBuildTestCase(
            description="macro_body_constant_resolves_in_the_macro_folder_for_a_subfolder_caller",
            files=_BODY_SCOPED,
            query="SELECT fee FROM orders",
            expected_rows=[(0.5,)],
        ),
        SqlMacroBuildTestCase(
            description="nested_local_macro_visible_to_the_outer_macro_only",
            files=_NESTED_LOCAL,
            query="SELECT fee FROM orders",
            expected_rows=[(100,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_model_calling_sql_macros_when_building_then_the_expanded_model_holds_the_rows(
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
        SqlMacroBuildTestCase(
            description=f"cte_list_calls_{spelling}",
            files={**CLEAN_ORDERS, **_FIRST_ORDERS, **orders_model(sql)},
            query="SELECT order_id FROM orders ORDER BY order_id",
            expected_rows=[(1,), (2,)],
        )
        for spelling, sql in _CTE_LIST_SPELLINGS
    ],
    ids=lambda case: case.description,
)
def test_given_any_spelling_of_list_calls_when_building_then_the_model_holds_the_rows(
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
        SqlMacroCommandTestCase(
            description="python_macros_in_their_folder_render_as_before",
            files={
                **BRACKET,
                **orders_model(
                    'SELECT @py.bracket("order_id") AS label FROM __ref("stg_orders")\n'
                ),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("'[' || CAST(order_id AS VARCHAR) || ']' AS label",),
            inspected_paths=("target/compiled/models/marts/orders.sql",),
        ),
        SqlMacroCommandTestCase(
            description="typed_argument_in_a_later_union_branch_is_not_checked",
            files={
                **CENTS_TO_DOLLARS,
                **_REFUNDS,
                **orders_model(
                    'SELECT @sql.cents_to_dollars(amount_cents) AS d FROM __ref("stg_orders")\n'
                    "UNION ALL\n"
                    'SELECT @sql.cents_to_dollars(status) AS d FROM __ref("stg_refunds")\n'
                ),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCommandTestCase(
            description="typed_argument_in_a_scalar_subquery_is_not_checked",
            files={
                **CENTS_TO_DOLLARS,
                **_REFUNDS,
                **orders_model(
                    "SELECT o.order_id,\n"
                    '  (SELECT MAX(@sql.cents_to_dollars(status)) FROM __ref("stg_refunds")) AS m\n'
                    'FROM __ref("stg_orders") AS o\n'
                ),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCommandTestCase(
            description="relation_argument_alias_shadowing_a_cte_reads_the_alias",
            files={
                **AUDIT_COLUMNS,
                **orders_model(
                    'WITH o AS (SELECT order_id FROM __ref("stg_orders"))\n'
                    'SELECT o.order_id, @sql.audit_columns(o) FROM __ref("stg_orders") AS o\n'
                ),
                **with_ignored_rules("SQLCLEAN005", reason="The shadowed CTE is never read."),
            },
            command=("compile",),
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCommandTestCase(
            description="lineage_reads_the_arguments_of_a_sql_macro",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    "SELECT o.order_id, @sql.cents_to_dollars(o.fee_cents + o.tax_cents, "
                    'places := 0) AS fees FROM __ref("stg_orders") AS o\n'
                ),
            },
            command=("lineage", "orders.fees", "--format", "list"),
            expected_returncode=0,
            expected_fragments=(
                "stg_orders.fee_cents -> orders.fees",
                "stg_orders.tax_cents -> orders.fees",
            ),
        ),
        SqlMacroCommandTestCase(
            description="rules_see_through_a_cte_macro_call",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\n'
                    "@sql.clean_orders(orders_raw)\nfinal AS (SELECT order_id FROM latest)\n"
                    "SELECT order_id FROM final\n"
                ),
            },
            command=("rules", "run", "SQBRMODEL102"),
            expected_returncode=1,
            expected_fragments=("models/marts/orders.sql", "[SQBRMODEL102]"),
        ),
        SqlMacroCommandTestCase(
            description="scope_names_the_invalid_sql_macro_file_and_keeps_the_valid_ones",
            files={
                **CENTS_TO_DOLLARS,
                "macros/sql/zz_invalid.sql": (
                    'MACRO (description "Invalid", returns BIGINT);\n\nWITH a AS (SELECT 1)\n'
                ),
                **orders_model(
                    'SELECT @sql.cents_to_dollars(amount_cents) AS d FROM __ref("stg_orders")\n'
                ),
            },
            command=("scope", "macro:cents_to_dollars", "--no-cache"),
            expected_returncode=1,
            expected_fragments=(
                "Resource: macro:cents_to_dollars",
                "ERROR S021 macros/sql/zz_invalid.sql: CTE-list macro zz_invalid() cannot declare returns",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_project_with_macros_when_running_a_command_then_its_output_reflects_them(
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


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroEditTestCase(
            description="edited_expression_macro",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    "SELECT DISTINCT order_id, @sql.cents_to_dollars(fee_cents) AS fee "
                    'FROM __ref("stg_orders")\nWHERE order_id = 2\n'
                ),
            },
            edited_path="macros/sql/cents_to_dollars.sql",
            replaced_text=("/ 100", "/ 10"),
            query="SELECT fee FROM orders",
            expected_rows=[(5.0,)],
        ),
        SqlMacroEditTestCase(
            description="sql_macro_called_only_from_a_hook",
            files={
                **CENTS_TO_DOLLARS,
                "models/marts/orders.sql": (
                    'MODEL (description "Orders", post_hooks [inline_sql("CREATE OR REPLACE TABLE '
                    'hook_fee AS SELECT @sql.cents_to_dollars(50) AS fee")]);\n\n'
                    'SELECT DISTINCT order_id FROM __ref("stg_orders")\n'
                ),
            },
            edited_path="macros/sql/cents_to_dollars.sql",
            replaced_text=("/ 100", "/ 10"),
            query="SELECT fee FROM hook_fee",
            expected_rows=[(5.0,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_edited_sql_macro_when_rebuilding_then_its_callers_use_the_new_body(
    test_case: SqlMacroEditTestCase, tmp_path: Path
) -> None:
    project_dir, first = run_project(tmp_path=tmp_path, files=test_case.files, command=("build",))
    assert first.returncode == 0, output(first)
    macro: Path = project_dir / test_case.edited_path
    macro.write_text(macro.read_text().replace(*test_case.replaced_text), encoding="utf-8")

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert second.returncode == 0, output(second)
    assert (
        query_duckdb(db_path=project_dir / DATABASE, sql=test_case.query) == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroFormatTestCase(
            description="cte_list_macro_and_its_call",
            files={
                "macros/sql/pair.sql": (
                    'MACRO (description "Pair", arguments (src RELATION (order_id BIGINT)));\n\n'
                    "with a as (select * from @src), b as (select order_id from a)\n"
                ),
                **orders_model(
                    'WITH o AS (SELECT * FROM __ref("stg_orders")),\n@sql.pair(o)\n'
                    "final AS (SELECT order_id FROM b)\nSELECT * FROM final\n"
                ),
            },
            formatted_path="macros/sql/pair.sql",
            expected_ending=(
                "WITH a AS (\n  SELECT *\n  FROM @src\n),\n\nb AS (\n  SELECT order_id\n"
                "  FROM a\n)\n"
            ),
        ),
        SqlMacroFormatTestCase(
            description="expression_macro_body_and_one_line_header",
            files={
                "macros/sql/net.sql": (
                    'MACRO (description "Net",   arguments (gross BIGINT,refund BIGINT), '
                    "returns DOUBLE);\n\nround(cast((@gross - coalesce(@refund, 0)) as double) "
                    "/ 100, 2)\n"
                ),
                **orders_model(
                    'SELECT DISTINCT order_id, @sql.net(amount_cents, fee_cents) AS net FROM __ref("stg_orders")\n'
                ),
            },
            formatted_path="macros/sql/net.sql",
            expected_start=(
                'MACRO (description "Net", arguments (gross BIGINT, refund BIGINT), '
                "returns DOUBLE);\n\n"
            ),
            expected_ending="ROUND(CAST((@gross - COALESCE(@refund, 0)) AS double) / 100, 2)\n",
        ),
        SqlMacroFormatTestCase(
            description="column_list_macro_body_and_multi_line_header",
            files={
                "macros/sql/stamps.sql": (
                    'MACRO (description "Stamps",\n   arguments (src RELATION (created_at TIMESTAMP, '
                    "updated_at TIMESTAMP)));\n\nselect @src.created_at,@src.updated_at\n"
                ),
                **orders_model('SELECT o.order_id, @sql.stamps(o) FROM __ref("stg_orders") AS o\n'),
            },
            formatted_path="macros/sql/stamps.sql",
            expected_start=(
                'MACRO (\n  description "Stamps",\n  arguments (src RELATION (created_at TIMESTAMP, '
                "updated_at TIMESTAMP)),\n);\n\n"
            ),
            expected_ending="SELECT\n  @src.created_at,\n  @src.updated_at\n",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_macro_files_when_formatting_then_bodies_and_calls_are_normalised(
    test_case: SqlMacroFormatTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(tmp_path=tmp_path, files=test_case.files, command=("format",))

    assert result.returncode == 0, output(result)
    formatted: str = (project_dir / test_case.formatted_path).read_text(encoding="utf-8")
    assert formatted.startswith(test_case.expected_start), formatted
    assert formatted.endswith(test_case.expected_ending), formatted
    check: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "format", "--check"), project_dir=project_dir
    )
    assert check.returncode == 0, output(check)
    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    assert compiled.returncode == 0, output(compiled)
