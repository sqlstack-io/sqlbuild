"""CLI e2e coverage for compatibility views that keep migrated models' old names working."""

from __future__ import annotations

import subprocess
import time
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.model_migrations._test_types import (
    OldNameClaimE2ETestCase,
    OldNameGatingE2ETestCase,
    OldNameReferenceE2ETestCase,
    OldNameUnknownDropE2ETestCase,
    OldNameViewAliasE2ETestCase,
    OldNameViewConfigE2ETestCase,
    OldNameViewJanitorE2ETestCase,
    OldNameViewMaterializationE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.model_migrations.helpers import (
    OLD_NAME_DESTINATION,
    OLD_NAME_MIGRATE_FROM,
    OLD_NAME_ORIGIN,
    OLD_NAME_SCHEMA,
    build_old_name_project,
    load_raw_orders,
    old_name_fact_values,
    old_name_facts,
    old_name_model_sql,
    origin_archive_count,
    plan_payload,
    prepare_old_name_rename,
    relation_rows,
    relation_type,
    sqb,
    write_old_name_project,
)

_ALL_FACTS: tuple[str, ...] = ("required", "origin_archived", "view_created")
_THREE_ORDERS: tuple[tuple[int, int], ...] = ((1, 101), (2, 102), (3, 103))
_REPORT_TASK: str = (
    "from sqlbuild.assets import asset\n\n\n"
    "@asset\n"
    "def report(ctx):\n"
    f'    """Test task report."""\n    ctx.query("SELECT count(*) FROM {OLD_NAME_SCHEMA}.{OLD_NAME_ORIGIN}").fetchall()\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameViewMaterializationE2ETestCase(
            description=materialized,
            materialized=materialized,
            expected_old_name_type="VIEW",
            expected_facts=_ALL_FACTS,
            expected_rows=_THREE_ORDERS,
        )
        for materialized in ("table", "view", "incremental", "snapshot")
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_model_when_building_then_old_name_reads_the_new_relation(
    tmp_path: Path, test_case: OldNameViewMaterializationE2ETestCase
) -> None:
    """Every materialization archives its old relation once and serves a view at the old name."""

    project_dir: Path = prepare_old_name_rename(
        tmp_path=tmp_path, materialized=test_case.materialized
    )
    planned: dict[str, Any] = plan_payload(project_dir=project_dir)

    _ = build_old_name_project(project_dir)
    load_raw_orders(project_dir=project_dir, last_day=4)
    _ = build_old_name_project(project_dir)
    rebuilt: dict[str, Any] = plan_payload(project_dir=project_dir)

    assert planned["old_names"][0]["action"] == "archive_and_view"
    assert relation_type(project_dir=project_dir, name=OLD_NAME_ORIGIN) == (
        test_case.expected_old_name_type
    )
    assert old_name_facts(project_dir=project_dir) == test_case.expected_facts
    assert origin_archive_count(project_dir=project_dir) == 1
    assert relation_rows(
        project_dir=project_dir, relation=f"{OLD_NAME_SCHEMA}.{OLD_NAME_ORIGIN}", columns="*"
    ) == relation_rows(
        project_dir=project_dir, relation=f"{OLD_NAME_SCHEMA}.{OLD_NAME_DESTINATION}", columns="*"
    )
    assert (
        relation_rows(
            project_dir=project_dir,
            relation=f"{OLD_NAME_SCHEMA}.{OLD_NAME_ORIGIN}",
            columns="order_id, amount_cents",
        )[:3]
        == test_case.expected_rows
    )
    assert rebuilt["migrations"][0]["old_name"]["action"] == "live"


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameViewAliasE2ETestCase(
            description="incremental column renamed in place",
            materialized="incremental",
            origin_columns="amount_cents",
            destination_columns="amount_cents AS revenue_cents",
            destination_schema="  columns (revenue_cents (migrate_from amount_cents)),\n",
            expected_old_columns=("order_id", "order_date", "amount_cents"),
            expected_aliases={"amount_cents": "revenue_cents"},
        ),
        OldNameViewAliasE2ETestCase(
            description="table column declared as an alias",
            materialized="table",
            origin_columns="amount_cents",
            destination_columns="amount_cents AS revenue_cents",
            destination_schema="  columns (revenue_cents (migrate_from amount_cents)),\n",
            expected_old_columns=("order_id", "order_date", "amount_cents"),
            expected_aliases={"amount_cents": "revenue_cents"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_renamed_column_when_building_then_old_name_view_uses_old_column_names(
    tmp_path: Path, test_case: OldNameViewAliasE2ETestCase
) -> None:
    """The compatibility view presents renamed columns under the names consumers used."""

    project_dir: Path = prepare_old_name_rename(
        tmp_path=tmp_path,
        materialized=test_case.materialized,
        origin_columns=test_case.origin_columns,
        destination_columns=test_case.destination_columns,
        destination_schema=test_case.destination_schema,
    )

    _ = build_old_name_project(project_dir)
    planned: dict[str, Any] = plan_payload(project_dir=project_dir)

    old_columns: tuple[str, ...] = tuple(
        str(row[0])
        for row in relation_rows(
            project_dir=project_dir,
            relation=(
                "(SELECT column_name, ordinal_position AS order_id FROM "
                f"information_schema.columns WHERE table_schema = '{OLD_NAME_SCHEMA}' "
                f"AND table_name = '{OLD_NAME_ORIGIN}')"
            ),
            columns="column_name",
        )
    )
    assert old_columns == test_case.expected_old_columns
    assert (
        relation_rows(
            project_dir=project_dir,
            relation=f"{OLD_NAME_SCHEMA}.{OLD_NAME_ORIGIN}",
            columns="order_id, amount_cents",
        )
        == _THREE_ORDERS
    )
    assert planned["old_names"][0]["column_aliases"] == test_case.expected_aliases


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameViewConfigE2ETestCase(
            description="project default keeps the view for thirty days",
            project_toml_extra="",
            extra_config="",
            expected_old_name_type="VIEW",
            expected_facts=_ALL_FACTS,
            expected_retention=("30d",),
            expected_plan_fragment=("└── old name  analytics.revenue\n        ├── view  until "),
        ),
        OldNameViewConfigE2ETestCase(
            description="model header overrides the project retention",
            project_toml_extra='\n[migrations]\nold_name_views = "90d"\n',
            extra_config="  old_name_view 7d,\n",
            expected_old_name_type="VIEW",
            expected_facts=_ALL_FACTS,
            expected_retention=("7d",),
            expected_plan_fragment=("└── old name  analytics.revenue\n        ├── view  until "),
        ),
        OldNameViewConfigE2ETestCase(
            description="project false keeps the old relation untouched",
            project_toml_extra="\n[migrations]\nold_name_views = false\n",
            extra_config="",
            expected_old_name_type="BASE TABLE",
            expected_facts=(),
            expected_retention=(),
            expected_plan_fragment=(
                "└── old name  analytics.revenue\n        └── left for janitor  old_name_view false"
            ),
        ),
        OldNameViewConfigE2ETestCase(
            description="model header false overrides the project default",
            project_toml_extra="",
            extra_config="  old_name_view false,\n",
            expected_old_name_type="BASE TABLE",
            expected_facts=(),
            expected_retention=(),
            expected_plan_fragment=(
                "└── old name  analytics.revenue\n        └── left for janitor  old_name_view false"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_old_name_view_config_when_building_then_retention_follows_config(
    tmp_path: Path, test_case: OldNameViewConfigE2ETestCase
) -> None:
    """Project and model retention decide whether and how long the old name is kept."""

    project_dir: Path = prepare_old_name_rename(
        tmp_path=tmp_path,
        materialized="table",
        extra_config=test_case.extra_config,
        project_toml_extra=test_case.project_toml_extra,
    )
    plan_text: str = sqb(project_dir=project_dir, args=("plan",)).stdout

    _ = build_old_name_project(project_dir)

    assert test_case.expected_plan_fragment in plan_text, plan_text
    assert relation_type(project_dir=project_dir, name=OLD_NAME_ORIGIN) == (
        test_case.expected_old_name_type
    )
    assert old_name_facts(project_dir=project_dir) == test_case.expected_facts
    assert (
        old_name_fact_values(
            project_dir=project_dir, event_type="required", column="view_retention"
        )
        == test_case.expected_retention
    )


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameGatingE2ETestCase(
            description="move recorded while views were off",
            expected_old_name_type="BASE TABLE",
            expected_facts=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_move_recorded_without_old_name_view_when_enabling_then_old_name_is_untouched(
    tmp_path: Path, test_case: OldNameGatingE2ETestCase
) -> None:
    """Moves recorded while views were off, as before the upgrade, never gain a view later."""

    project_dir: Path = prepare_old_name_rename(
        tmp_path=tmp_path,
        materialized="table",
        project_toml_extra="\n[migrations]\nold_name_views = false\n",
    )
    _ = build_old_name_project(project_dir)
    project_dir = write_old_name_project(
        tmp_path=tmp_path,
        models={
            OLD_NAME_DESTINATION: old_name_model_sql(
                materialized="table", extra_config=OLD_NAME_MIGRATE_FROM
            )
        },
    )

    _ = build_old_name_project(project_dir)

    assert relation_type(project_dir=project_dir, name=OLD_NAME_ORIGIN) == (
        test_case.expected_old_name_type
    )
    assert old_name_facts(project_dir=project_dir) == test_case.expected_facts
    assert plan_payload(project_dir=project_dir)["old_names"] == []


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameViewJanitorE2ETestCase(
            description="unexpired view is kept and listed",
            extra_config="",
            janitor_args=(),
            wait_seconds=0,
            expected_janitor_fragment=(
                "└── analytics.revenue  -> model:daily_revenue\n    └── expires  "
            ),
            expected_old_name_type="VIEW",
            expected_drop_reasons=(),
        ),
        OldNameViewJanitorE2ETestCase(
            description="expired view is dropped",
            extra_config="  old_name_view 1s,\n",
            janitor_args=(),
            wait_seconds=1.5,
            expected_janitor_fragment=(
                "└── analytics.revenue  -> model:daily_revenue\n    └── drop  now  (expired "
            ),
            expected_old_name_type=None,
            expected_drop_reasons=("expired",),
        ),
        OldNameViewJanitorE2ETestCase(
            description="requested view is dropped early",
            extra_config="",
            janitor_args=("--drop-old-name-view", "analytics.revenue"),
            wait_seconds=0,
            expected_janitor_fragment="    └── drop  now  (requested; would expire ",
            expected_old_name_type=None,
            expected_drop_reasons=("early",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_old_name_view_when_running_janitor_then_it_drops_only_expired_or_requested(
    tmp_path: Path, test_case: OldNameViewJanitorE2ETestCase
) -> None:
    """The janitor drops expired or requested views, records the drop, and spares live ones."""

    project_dir: Path = prepare_old_name_rename(
        tmp_path=tmp_path, materialized="table", extra_config=test_case.extra_config
    )
    _ = build_old_name_project(project_dir)
    time.sleep(test_case.wait_seconds)

    janitor: subprocess.CompletedProcess[str] = sqb(
        project_dir=project_dir, args=("janitor", "--auto-approve", *test_case.janitor_args)
    )

    assert janitor.returncode == 0, janitor.stdout + janitor.stderr
    assert test_case.expected_janitor_fragment in janitor.stdout, janitor.stdout
    assert relation_type(project_dir=project_dir, name=OLD_NAME_ORIGIN) == (
        test_case.expected_old_name_type
    )
    assert (
        old_name_fact_values(
            project_dir=project_dir, event_type="view_dropped", column="drop_reason"
        )
        == test_case.expected_drop_reasons
    )
    assert origin_archive_count(project_dir=project_dir) == 1


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameUnknownDropE2ETestCase(
            description="name with no recorded compatibility view",
            requested_name="analytics.orders",
            expected_code="C503",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_old_name_when_dropping_early_then_janitor_refuses(
    tmp_path: Path, test_case: OldNameUnknownDropE2ETestCase
) -> None:
    """An early drop must name a recorded compatibility view."""

    project_dir: Path = prepare_old_name_rename(tmp_path=tmp_path, materialized="table")
    _ = build_old_name_project(project_dir)

    janitor: subprocess.CompletedProcess[str] = sqb(
        project_dir=project_dir,
        args=("janitor", "--auto-approve", "--drop-old-name-view", test_case.requested_name),
    )

    assert janitor.returncode != 0
    assert test_case.expected_code in janitor.stdout + janitor.stderr
    assert relation_type(project_dir=project_dir, name=OLD_NAME_ORIGIN) == "VIEW"


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameClaimE2ETestCase(
            description="new model named like a live compatibility view",
            expected_error_fragments=(
                "M114",
                "would replace analytics.revenue, a compatibility view for model:daily_revenue until",
                "sqb janitor --drop-old-name-view analytics.revenue",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_live_compatibility_view_when_new_model_claims_its_name_then_build_is_refused(
    tmp_path: Path, test_case: OldNameClaimE2ETestCase
) -> None:
    """A model cannot silently replace a compatibility view; an early drop releases the name."""

    project_dir: Path = prepare_old_name_rename(tmp_path=tmp_path, materialized="table")
    _ = build_old_name_project(project_dir)
    project_dir = write_old_name_project(
        tmp_path=tmp_path,
        models={
            OLD_NAME_DESTINATION: old_name_model_sql(materialized="table"),
            OLD_NAME_ORIGIN: old_name_model_sql(materialized="table", columns="0 AS other"),
        },
    )

    refused: subprocess.CompletedProcess[str] = sqb(project_dir=project_dir, args=("build",))
    dropped: subprocess.CompletedProcess[str] = sqb(
        project_dir=project_dir,
        args=("janitor", "--auto-approve", "--drop-old-name-view", "analytics.revenue"),
    )
    _ = build_old_name_project(project_dir)

    assert refused.returncode != 0
    assert all(
        fragment in refused.stdout + refused.stderr
        for fragment in test_case.expected_error_fragments
    ), refused.stdout + refused.stderr
    assert dropped.returncode == 0, dropped.stdout + dropped.stderr
    assert relation_type(project_dir=project_dir, name=OLD_NAME_ORIGIN) == "BASE TABLE"


@pytest.mark.parametrize(
    "test_case",
    [
        OldNameReferenceE2ETestCase(
            description="declared rename is rejected at compile time",
            header=OLD_NAME_MIGRATE_FROM,
            command="compile",
            expected_fragment=(
                "error[P008]: asset:report names 'analytics.revenue' in SQL passed to ctx.query(); "
                "it is the old name of model:daily_revenue"
            ),
        ),
        OldNameReferenceE2ETestCase(
            description="discovered rename is rejected at plan time",
            header="",
            command="build",
            expected_fragment=(
                "asset:report names 'analytics.revenue' in SQL passed to ctx.query(); "
                "it is the old name of model:daily_revenue"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_sql_reading_old_name_when_checking_then_p008_rejects_it(
    tmp_path: Path, test_case: OldNameReferenceE2ETestCase
) -> None:
    """Project code must move to the new model; the old name is only for outside consumers."""

    project_dir: Path = write_old_name_project(
        tmp_path=tmp_path, models={OLD_NAME_ORIGIN: old_name_model_sql(materialized="table")}
    )
    load_raw_orders(project_dir=project_dir, last_day=3)
    _ = build_old_name_project(project_dir)
    project_dir = write_old_name_project(
        tmp_path=tmp_path,
        models={
            OLD_NAME_DESTINATION: old_name_model_sql(
                materialized="table", extra_config=test_case.header
            )
        },
        python_files={"report/asset.py": _REPORT_TASK},
    )

    result: subprocess.CompletedProcess[str] = sqb(
        project_dir=project_dir, args=(test_case.command,)
    )

    assert result.returncode != 0, result.stdout + result.stderr
    assert test_case.expected_fragment in result.stdout + result.stderr, (
        result.stdout + result.stderr
    )
    assert relation_type(project_dir=project_dir, name=OLD_NAME_ORIGIN) == "BASE TABLE"
