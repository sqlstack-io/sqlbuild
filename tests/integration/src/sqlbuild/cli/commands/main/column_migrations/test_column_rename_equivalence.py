"""Integration coverage that only provably pure renames move history or skip replay."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.cli.commands.main.column_migrations._test_types import (
    CollidingAliasRenameTestCase,
    EquivalentRenameTestCase,
    RebindingRenameTestCase,
    UnprovenRenameReplayTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.column_migrations.helpers import (
    REPLAY_FULL,
    CliRun,
    build,
    build_initial,
    build_ok,
    change_historical_amount,
    column_names,
    column_values,
    incremental_sql,
    load_descending_amounts,
    migrate_columns,
    model_plan,
    orders_sql,
    plan_json,
    planned_column_migrations,
    state_table_names,
    write_model,
)
from tests.integration.src.sqlbuild.cli.commands.main.model_migrations.helpers import PROJECT_TOML

_ORDERS: str = 'FROM __source("raw_orders")'
_REBOUND_BODY: str = (
    f"WITH changed AS (SELECT order_id, order_date, tax AS amount {_ORDERS}) "
    "SELECT order_id, order_date, amount AS revenue FROM changed"
)
_QUALIFY_BEFORE: str = (
    f"SELECT order_id, order_date, amount {_ORDERS} QUALIFY row_number() OVER (ORDER BY amount) = 1"
)
_QUALIFY_AFTER: str = (
    f"SELECT order_id, order_date, amount AS tax {_ORDERS} "
    "QUALIFY row_number() OVER (ORDER BY tax) = 1"
)
# The QUALIFY window ranks by a non-unique column on purpose, which SQLCLEAN018 reports.
_QUALIFY_PROJECT_TOML: str = PROJECT_TOML + '\n[rules]\nignore = ["SQLCLEAN018"]\n'
_FILTERED_BODY: str = f"SELECT order_id, order_date, amount AS revenue {_ORDERS} WHERE amount > 0"
_REPLAY_BOUNDED: str = "  replay_on_change bounded-2d,\n"
_HISTORY: tuple[tuple[int, Any], ...] = ((1, 101), (2, 102), (3, 103), (4, 104), (5, 105))


@pytest.mark.parametrize(
    "test_case",
    [
        RebindingRenameTestCase(
            description="on_schema_change fail rejects a rebound column instead of renaming it",
            rebound_sql=incremental_sql(
                body=_REBOUND_BODY, extra_config="  on_schema_change fail,\n"
            ),
            expected_build_succeeds=False,
            expected_columns=("order_id", "order_date", "amount"),
            expected_hint=(
                "similar to amount; if this is a rename, add revenue (migrate_from amount)"
            ),
        ),
        RebindingRenameTestCase(
            description="append_new_columns keeps amount history and adds the rebound column",
            rebound_sql=incremental_sql(body=_REBOUND_BODY),
            expected_build_succeeds=True,
            expected_columns=("order_id", "order_date", "amount", "revenue"),
            expected_hint=(
                "similar to amount; if this is a rename, add revenue (migrate_from amount)"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_expression_rebound_by_a_changed_cte_when_building_then_nothing_is_renamed(
    test_case: RebindingRenameTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    build_initial(project_dir=tmp_path, capsys=capsys, sql=orders_sql())
    write_model(project_dir=tmp_path, sql=test_case.rebound_sql)

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    result: CliRun = build(project_dir=tmp_path, capsys=capsys)

    assert planned_column_migrations(plan) == ()
    assert model_plan(plan)["column_rename_hints"][0]["hint"] == test_case.expected_hint
    assert (result.exit_code == 0) is test_case.expected_build_succeeds, result.output
    assert column_names(project_dir=tmp_path) == test_case.expected_columns
    assert column_values(project_dir=tmp_path, column="amount")[:2] == _HISTORY[:2]
    assert "_sqlbuild_column_migrations" not in state_table_names(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        EquivalentRenameTestCase(
            description="automatic rename with an ORDER BY on the new alias is not replayed",
            initial_sql=incremental_sql(
                body=f"SELECT order_id, order_date, amount {_ORDERS} ORDER BY amount",
                extra_config=REPLAY_FULL,
            ),
            renamed_sql=incremental_sql(
                body=f"SELECT order_id, order_date, amount AS revenue {_ORDERS} ORDER BY revenue",
                extra_config=REPLAY_FULL,
            ),
            expected_planned=(("amount", "revenue", "automatic", "rename"),),
            expected_backfill="forward",
            expected_columns=("order_id", "order_date", "revenue"),
            expected_values=_HISTORY,
        ),
        EquivalentRenameTestCase(
            description="automatic rename that also reorders projections is not replayed",
            initial_sql=orders_sql(extra_config=REPLAY_FULL),
            renamed_sql=incremental_sql(
                body=f"SELECT amount AS revenue, order_id, order_date {_ORDERS}",
                extra_config=REPLAY_FULL,
            ),
            expected_planned=(("amount", "revenue", "automatic", "rename"),),
            expected_backfill="forward",
            expected_columns=("order_id", "order_date", "revenue"),
            expected_values=_HISTORY,
        ),
        EquivalentRenameTestCase(
            description="declared rename with an ORDER BY on the new alias is not replayed",
            initial_sql=incremental_sql(
                body=f"SELECT order_id, order_date, amount {_ORDERS} ORDER BY amount",
                extra_config=REPLAY_FULL,
            ),
            renamed_sql=incremental_sql(
                body=f"SELECT order_id, order_date, amount AS revenue {_ORDERS} ORDER BY revenue",
                extra_config=REPLAY_FULL + migrate_columns("revenue (migrate_from amount)"),
            ),
            expected_planned=(("amount", "revenue", "manual", "rename"),),
            expected_backfill="forward",
            expected_columns=("order_id", "order_date", "revenue"),
            expected_values=_HISTORY,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rename_equivalent_modulo_aliases_when_building_then_history_is_kept(
    test_case: EquivalentRenameTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    build_initial(project_dir=tmp_path, capsys=capsys, sql=test_case.initial_sql)
    write_model(project_dir=tmp_path, sql=test_case.renamed_sql)
    change_historical_amount(project_dir=tmp_path)

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert planned_column_migrations(plan) == test_case.expected_planned
    assert model_plan(plan)["backfill"]["action"] == test_case.expected_backfill
    assert column_names(project_dir=tmp_path) == test_case.expected_columns
    assert column_values(project_dir=tmp_path, column="revenue") == test_case.expected_values


@pytest.mark.parametrize(
    "test_case",
    [
        UnprovenRenameReplayTestCase(
            description="a full replay policy rebuilds exactly as it would without the rename",
            undeclared_sql=incremental_sql(body=_FILTERED_BODY, extra_config=REPLAY_FULL),
            declared_sql=incremental_sql(
                body=_FILTERED_BODY,
                extra_config=REPLAY_FULL + migrate_columns("revenue (migrate_from amount)"),
            ),
            expected_backfill={"action": "full", "duration": None},
            expected_values=((1, 999), *_HISTORY[1:]),
        ),
        UnprovenRenameReplayTestCase(
            description="a bounded replay policy keeps renamed history outside its window",
            undeclared_sql=incremental_sql(body=_FILTERED_BODY, extra_config=_REPLAY_BOUNDED),
            declared_sql=incremental_sql(
                body=_FILTERED_BODY,
                extra_config=_REPLAY_BOUNDED + migrate_columns("revenue (migrate_from amount)"),
            ),
            expected_backfill={"action": "bounded", "duration": "2d"},
            expected_values=_HISTORY,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rename_with_other_query_changes_when_building_then_replay_matches_no_rename(
    test_case: UnprovenRenameReplayTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    build_initial(project_dir=tmp_path, capsys=capsys, sql=orders_sql())
    change_historical_amount(project_dir=tmp_path)
    write_model(project_dir=tmp_path, sql=test_case.undeclared_sql)
    undeclared: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    write_model(project_dir=tmp_path, sql=test_case.declared_sql)
    declared: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert planned_column_migrations(undeclared) == ()
    assert planned_column_migrations(declared) == (("amount", "revenue", "manual", "rename"),)
    assert model_plan(undeclared)["backfill"] == test_case.expected_backfill
    assert model_plan(declared)["backfill"] == test_case.expected_backfill
    assert column_values(project_dir=tmp_path, column="revenue") == test_case.expected_values


@pytest.mark.parametrize(
    "test_case",
    [
        CollidingAliasRenameTestCase(
            description="QUALIFY on a new name that is an input column replays as configured",
            initial_sql=incremental_sql(body=_QUALIFY_BEFORE, extra_config=REPLAY_FULL),
            renamed_sql=incremental_sql(body=_QUALIFY_AFTER, extra_config=REPLAY_FULL),
            expected_planned=(),
            expected_backfill={"action": "full", "duration": None},
            expected_values=((1, 99),),
        ),
        CollidingAliasRenameTestCase(
            description="a declared rename with a colliding QUALIFY name still replays fully",
            initial_sql=incremental_sql(body=_QUALIFY_BEFORE, extra_config=REPLAY_FULL),
            renamed_sql=incremental_sql(
                body=_QUALIFY_AFTER,
                extra_config=REPLAY_FULL + migrate_columns("tax (migrate_from amount)"),
            ),
            expected_planned=(("amount", "tax", "manual", "rename"),),
            expected_backfill={"action": "full", "duration": None},
            expected_values=((1, 99),),
        ),
        CollidingAliasRenameTestCase(
            description="ORDER BY on a new name that is an input column replays as configured",
            initial_sql=incremental_sql(
                body=f"SELECT order_id, order_date, amount {_ORDERS} ORDER BY amount LIMIT 1",
                extra_config=REPLAY_FULL,
            ),
            renamed_sql=incremental_sql(
                body=f"SELECT order_id, order_date, amount AS tax {_ORDERS} ORDER BY tax LIMIT 1",
                extra_config=REPLAY_FULL,
            ),
            expected_planned=(),
            expected_backfill={"action": "full", "duration": None},
            expected_values=((5, 95),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_alias_colliding_with_an_input_column_when_building_then_it_replays(
    test_case: CollidingAliasRenameTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_model(project_dir=tmp_path, sql=test_case.initial_sql, project_toml=_QUALIFY_PROJECT_TOML)
    load_descending_amounts(project_dir=tmp_path, last_day=3)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    load_descending_amounts(project_dir=tmp_path, last_day=5)
    write_model(project_dir=tmp_path, sql=test_case.renamed_sql, project_toml=_QUALIFY_PROJECT_TOML)

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert planned_column_migrations(plan) == test_case.expected_planned
    assert model_plan(plan)["backfill"] == test_case.expected_backfill
    assert column_values(project_dir=tmp_path, column="tax") == test_case.expected_values


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
