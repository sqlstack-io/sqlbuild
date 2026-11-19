"""Integration coverage for fingerprint-based automatic migrations through the real CLI."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.cli.commands.main.model_migrations._test_types import (
    AutomaticMigrationTestCase,
    FingerprintHistoryTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.model_migrations.helpers import (
    CliRun,
    build,
    build_ok,
    build_original_order_models,
    daily_totals_sql,
    execute,
    fail_clone_into,
    fail_model_build,
    incremental_orders_sql,
    load_raw_orders,
    migration_events,
    model_entry,
    model_fingerprint_rows,
    order_ids,
    original_order_models,
    plan_json,
    planned_migrations,
    relation_names,
    rename_migration_old_names,
    rename_migrations,
    renamed_models,
    renamed_old_names,
    renamed_order_models,
    row_count,
    run_sqb,
    warning_messages,
    write_project,
)

_FIRST_RUN: str = "first_run"


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="incremental, view, and incremental renamed together",
            expected_migrations=(
                ("daily_order_totals", "customer_daily_order_totals", "automatic", "migrate"),
                ("orders_enriched", "customer_orders_enriched", "automatic", "renamed"),
                ("stg_orders", "stg_customer_orders", "automatic", "migrate"),
            ),
            expected_events=(
                ("daily_order_totals", "customer_daily_order_totals", "migrate"),
                ("orders_enriched", "customer_orders_enriched", "renamed"),
                ("stg_orders", "stg_customer_orders", "migrate"),
            ),
            # Only the view is built under its new name; the incremental models take their data
            # over, so their migrations are steps, not renames.
            expected_renames=(("orders_enriched", "customer_orders_enriched"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_coordinated_renames_when_building_then_history_moves_automatically(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    build_original_order_models(project_dir=tmp_path, capsys=capsys)
    load_raw_orders(project_dir=tmp_path, first_day=3, last_day=8)
    write_project(project_dir=tmp_path, models=renamed_order_models())

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    result: CliRun = build_ok(project_dir=tmp_path, capsys=capsys)
    rebuild_plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)

    assert planned_migrations(plan) == test_case.expected_migrations
    # Each renamed model's row names its old model, and its own migration is marked as the rename.
    assert renamed_models(plan) == test_case.expected_renames
    assert rename_migrations(plan) == test_case.expected_renames
    # What stays at the old name is on the renamed row too, as its migration has it.
    assert renamed_old_names(plan) == rename_migration_old_names(plan)
    assert all(old_name is not None for old_name in renamed_old_names(plan).values())
    assert model_entry(plan=plan, name="stg_customer_orders")["reason"] != _FIRST_RUN
    assert model_entry(plan=plan, name="customer_daily_order_totals")["reason"] != _FIRST_RUN
    assert "Migrated main.stg_orders -> main.stg_customer_orders" in result.output
    assert order_ids(project_dir=tmp_path, relation="main.stg_customer_orders") == tuple(
        range(1, 9)
    )
    assert row_count(project_dir=tmp_path, relation="main.customer_daily_order_totals") == 8
    assert tuple(sorted(migration_events(project_dir=tmp_path))) == test_case.expected_events
    assert planned_migrations(rebuild_plan) == ()


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="downstream rename matches after an upstream logic change",
            expected_migrations=(
                ("daily_order_totals", "customer_daily_order_totals", "automatic", "migrate"),
            ),
            expected_events=(("daily_order_totals", "customer_daily_order_totals", "migrate"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_upstream_logic_change_when_renaming_then_downstream_still_matches(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    build_original_order_models(project_dir=tmp_path, capsys=capsys)
    models: dict[str, str] = original_order_models()
    models["stg_orders"] = incremental_orders_sql(
        select_sql=(
            "SELECT order_id, order_date, amount_cents * 2 AS amount_cents "
            'FROM __source("raw_orders")'
        )
    )
    models["customer_daily_order_totals"] = daily_totals_sql(
        upstream="orders_enriched", cte="enriched"
    )
    del models["daily_order_totals"]
    write_project(project_dir=tmp_path, models=models)

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert planned_migrations(plan) == test_case.expected_migrations
    assert model_entry(plan=plan, name="customer_daily_order_totals")["reason"] != _FIRST_RUN
    assert migration_events(project_dir=tmp_path) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="renamed model with a new filter starts fresh",
            expected_reason="first_run",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_changed_logic_when_renaming_then_destination_is_first_run(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    build_original_order_models(project_dir=tmp_path, capsys=capsys)
    write_project(
        project_dir=tmp_path,
        models={
            "stg_customer_orders": incremental_orders_sql(
                select_sql=(
                    'SELECT order_id, order_date, amount_cents FROM __source("raw_orders") '
                    "WHERE amount_cents > 0"
                )
            )
        },
    )

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)

    assert planned_migrations(plan) == test_case.expected_migrations
    assert model_entry(plan=plan, name="stg_customer_orders")["reason"] == (
        test_case.expected_reason
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="two identical removed models make the match ambiguous",
            expected_reason="first_run",
            expected_warning=(
                "'stg_customer_orders' matches several earlier models (orders_copy, stg_orders); "
                "no automatic migration was inferred. Add migrate_from to choose one."
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_identical_orphans_when_renaming_then_warns_and_does_not_migrate(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_project(
        project_dir=tmp_path,
        models={"stg_orders": incremental_orders_sql(), "orders_copy": incremental_orders_sql()},
    )
    load_raw_orders(project_dir=tmp_path, first_day=1, last_day=3)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    write_project(project_dir=tmp_path, models={"stg_customer_orders": incremental_orders_sql()})

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)

    assert planned_migrations(plan) == test_case.expected_migrations
    assert test_case.expected_warning in warning_messages(plan)
    assert model_entry(plan=plan, name="stg_customer_orders")["reason"] == (
        test_case.expected_reason
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="dropped removed relation cannot migrate",
            expected_reason="first_run",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dropped_old_relation_when_renaming_then_destination_is_first_run(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_project(project_dir=tmp_path, models={"stg_orders": incremental_orders_sql()})
    load_raw_orders(project_dir=tmp_path, first_day=1, last_day=3)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    execute(project_dir=tmp_path, sql="DROP TABLE main.stg_orders")
    write_project(project_dir=tmp_path, models={"stg_customer_orders": incremental_orders_sql()})

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)

    assert planned_migrations(plan) == test_case.expected_migrations
    assert model_entry(plan=plan, name="stg_customer_orders")["reason"] == (
        test_case.expected_reason
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="migrate_from wins over a matching fingerprint",
            expected_migrations=(("legacy_orders", "stg_customer_orders", "manual", "migrate"),),
            expected_events=(("legacy_orders", "stg_customer_orders", "migrate"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_migrate_from_when_fingerprint_matches_another_then_manual_wins(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_project(
        project_dir=tmp_path,
        models={
            "stg_orders": incremental_orders_sql(),
            "legacy_orders": incremental_orders_sql(
                select_sql=(
                    'SELECT order_id, order_date, amount_cents FROM __source("raw_orders") '
                    "WHERE order_id <= 2"
                )
            ),
        },
    )
    load_raw_orders(project_dir=tmp_path, first_day=1, last_day=4)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    write_project(
        project_dir=tmp_path,
        models={"stg_customer_orders": incremental_orders_sql(migrate_from="legacy_orders")},
    )

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert planned_migrations(plan) == test_case.expected_migrations
    assert migration_events(project_dir=tmp_path) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="qualified migrate_from removes its origin and destination from matching",
            expected_migrations=((None, "stg_customer_orders", "manual", "migrate"),),
            expected_events=(("legacy_orders", "stg_customer_orders", "migrate"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_qualified_migrate_from_when_another_orphan_matches_then_only_declared_moves(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_project(
        project_dir=tmp_path,
        models={
            "stg_orders": incremental_orders_sql(),
            "legacy_orders": incremental_orders_sql(
                select_sql=(
                    'SELECT order_id, order_date, amount_cents FROM __source("raw_orders") '
                    "WHERE order_id <= 2"
                )
            ),
        },
    )
    load_raw_orders(project_dir=tmp_path, first_day=1, last_day=4)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    write_project(
        project_dir=tmp_path,
        models={"stg_customer_orders": incremental_orders_sql(migrate_from="main.legacy_orders")},
    )

    plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert planned_migrations(plan) == test_case.expected_migrations
    assert migration_events(project_dir=tmp_path) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="recorded upstream move resolves the downstream rename on retry",
            expected_migrations=(
                ("daily_order_totals", "customer_daily_order_totals", "automatic", "migrate"),
                ("orders_enriched", "customer_orders_enriched", "automatic", "renamed"),
                ("stg_orders", "stg_customer_orders", "automatic", "done"),
            ),
            expected_events=(
                ("daily_order_totals", "customer_daily_order_totals", "migrate"),
                ("orders_enriched", "customer_orders_enriched", "renamed"),
                ("stg_orders", "stg_customer_orders", "migrate"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_interrupted_coordinated_renames_when_rerunning_then_downstream_resumes(
    test_case: AutomaticMigrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    build_original_order_models(project_dir=tmp_path, capsys=capsys)
    load_raw_orders(project_dir=tmp_path, first_day=3, last_day=8)
    write_project(project_dir=tmp_path, models=renamed_order_models())

    with monkeypatch.context() as patch:
        fail_clone_into(monkeypatch=patch, origin="main.daily_order_totals")
        interrupted: CliRun = build(project_dir=tmp_path, capsys=capsys)
    events_after_crash: tuple[tuple[str, str, str], ...] = migration_events(project_dir=tmp_path)
    retry_plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert interrupted.exit_code == 1, interrupted.output
    assert events_after_crash == (("stg_orders", "stg_customer_orders", "migrate"),)
    assert planned_migrations(retry_plan) == test_case.expected_migrations
    assert model_entry(plan=retry_plan, name="stg_customer_orders")["reason"] != _FIRST_RUN
    assert order_ids(project_dir=tmp_path, relation="main.stg_customer_orders") == tuple(
        range(1, 9)
    )
    assert row_count(project_dir=tmp_path, relation="main.customer_daily_order_totals") == 8
    assert tuple(sorted(migration_events(project_dir=tmp_path))) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="selecting one of two new models still sees the whole-project ambiguity",
            expected_reason="first_run",
            expected_warning=(
                "'stg_customer_orders' matches earlier model 'stg_orders', which also matches "
                "'stg_web_orders'; no automatic migration was inferred. "
                "Add migrate_from to choose one."
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_one_orphan_matching_two_new_models_when_selecting_one_then_warns_ambiguous(
    test_case: AutomaticMigrationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_project(project_dir=tmp_path, models={"stg_orders": incremental_orders_sql()})
    load_raw_orders(project_dir=tmp_path, first_day=1, last_day=3)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    write_project(
        project_dir=tmp_path,
        models={
            "stg_customer_orders": incremental_orders_sql(),
            "stg_web_orders": incremental_orders_sql(),
        },
    )

    plan: dict[str, Any] = plan_json(
        project_dir=tmp_path, capsys=capsys, args=("--select", "stg_customer_orders")
    )
    built: CliRun = run_sqb(
        project_dir=tmp_path, args=("build", "--select", "stg_customer_orders"), capsys=capsys
    )

    assert planned_migrations(plan) == test_case.expected_migrations
    assert warning_messages(plan) == (test_case.expected_warning,)
    assert model_entry(plan=plan, name="stg_customer_orders")["reason"] == (
        test_case.expected_reason
    )
    assert built.exit_code == 0, built.output
    assert "_sqlbuild_migrations" not in relation_names(project_dir=tmp_path, schema="main")


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticMigrationTestCase(
            description="downstream failed after its clone while the upstream was rebuilt",
            expected_migrations=(
                ("daily_order_totals", "customer_daily_order_totals", "automatic", "done"),
            ),
            expected_events=(
                ("daily_order_totals", "customer_daily_order_totals", "migrate"),
                ("stg_orders", "stg_customer_orders", "migrate"),
            ),
            expected_reason="normal_incremental",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprinted_upstream_when_retrying_failed_downstream_then_history_is_kept(
    test_case: AutomaticMigrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    replay_full: str = "  replay_on_change full,\n"
    write_project(
        project_dir=tmp_path,
        models={
            "stg_orders": incremental_orders_sql(),
            "daily_order_totals": daily_totals_sql(
                upstream="stg_orders", cte="stg_orders", extra_config=replay_full
            ),
        },
    )
    load_raw_orders(project_dir=tmp_path, first_day=1, last_day=5)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    load_raw_orders(project_dir=tmp_path, first_day=3, last_day=8)
    write_project(
        project_dir=tmp_path,
        models={
            "stg_customer_orders": incremental_orders_sql(),
            "customer_daily_order_totals": daily_totals_sql(
                upstream="stg_customer_orders",
                cte="stg_customer_orders",
                extra_config=replay_full,
            ),
        },
    )

    with monkeypatch.context() as patch:
        fail_model_build(monkeypatch=patch, model_name="customer_daily_order_totals")
        interrupted: CliRun = build(project_dir=tmp_path, capsys=capsys)
    upstream_fingerprinted: dict[str, Any] = plan_json(
        project_dir=tmp_path, capsys=capsys, args=("--select", "stg_customer_orders")
    )
    retry_plan: dict[str, Any] = plan_json(project_dir=tmp_path, capsys=capsys)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert interrupted.exit_code == 1, interrupted.output
    assert planned_migrations(upstream_fingerprinted) == ()
    assert planned_migrations(retry_plan) == test_case.expected_migrations
    assert model_entry(plan=retry_plan, name="stg_customer_orders")["reason"] != _FIRST_RUN
    assert model_entry(plan=retry_plan, name="customer_daily_order_totals")["reason"] == (
        test_case.expected_reason
    )
    assert row_count(project_dir=tmp_path, relation="main.customer_daily_order_totals") == 8
    assert tuple(sorted(migration_events(project_dir=tmp_path))) == test_case.expected_events


@pytest.mark.parametrize(
    "test_case",
    [
        FingerprintHistoryTestCase(
            description="rename history keeps every rebuilt incremental model recording",
            expected_rows_without_history=0,
            expected_min_rows_after_history=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_migration_history_when_rebuilding_unchanged_models_then_every_build_records(
    test_case: FingerprintHistoryTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    build_original_order_models(project_dir=tmp_path, capsys=capsys)
    before_history: int = model_fingerprint_rows(project_dir=tmp_path)
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    unchanged_without_history: int = model_fingerprint_rows(project_dir=tmp_path)
    write_project(project_dir=tmp_path, models=renamed_order_models())
    _ = build_ok(project_dir=tmp_path, capsys=capsys)
    after_migration: int = model_fingerprint_rows(project_dir=tmp_path)

    _ = build_ok(project_dir=tmp_path, capsys=capsys)

    assert unchanged_without_history - before_history == test_case.expected_rows_without_history
    assert migration_events(project_dir=tmp_path) != ()
    assert model_fingerprint_rows(project_dir=tmp_path) - after_migration >= (
        test_case.expected_min_rows_after_history
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
