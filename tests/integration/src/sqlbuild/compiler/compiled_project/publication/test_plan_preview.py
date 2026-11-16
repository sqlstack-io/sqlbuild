"""A location plans from the server's synced copy and never opens the warehouse to do it."""

import json
import os
import sqlite3
import subprocess
import sys
import time
from contextlib import closing
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ChangedPlanPreviewTestCase,
    HeldPlanTestCase,
    PlanPreviewTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    request_plan,
    request_warehouse_sync,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    changed_build_and_plan_from_copy,
    changed_runs,
    live_plan,
    location_process,
    plan_from_copy,
    plans_from_copy,
    run_sqb,
    snapshot_api,
    wait_for_generation,
    wait_for_location_work,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_EXPRESSION: str = "SELECT * FROM (VALUES (1, 100)) AS refunds(refund_id, amount_cents)"
_SOURCES: str = dedent(
    """
    sources:
      - name: raw_refunds
        description: Refunds described from an inline query.
        expression: |
          {expression}
        type_enforcement: true
        freshness:
          column: refund_id
        columns:
          - name: refund_id
            type: INTEGER
          - name: amount_cents
            type: INTEGER
    """
).strip()
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n[connections.local]\ndatabase = "warehouse.db"\n'
        '[targets.development]\nschema = "main"\n'
    ),
    "seeds/raw_orders.csv": "id,ordered_at\n1,2026-01-01 00:00:00\n2,2026-01-01 01:00:00\n",
    "seeds/raw_orders.yml": dedent(
        """
        seeds:
          - name: raw_orders
            description: Raw orders.
            columns:
              - name: id
                type: INTEGER
              - name: ordered_at
                type: TIMESTAMP
        """
    ).strip()
    + "\n",
    "models/orders.sql": dedent(
        """
        MODEL (description "Orders by timestamp cursor.",
          materialized incremental,
          incremental_strategy append,
          cursor ordered_at,
          cursor_type timestamp,
          cursor_grain second,
        );

        SELECT id, ordered_at FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
    "models/daily_orders.sql": dedent(
        """
        MODEL (description "Daily orders rebuilt over a lookback.",
          materialized incremental,
          incremental_strategy delete_insert,
          cursor ordered_at,
          cursor_type timestamp,
          cursor_grain second,
          lookback 1d,
        );

        SELECT id, ordered_at FROM __seed("raw_orders")
        WHERE ordered_at >= __cursor_start() AND ordered_at < __cursor_end()
        """
    ).strip()
    + "\n",
    "models/order_count.sql": dedent(
        """
        MODEL (description "Order count without a cursor.", materialized table);

        SELECT COUNT(*) AS order_count FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
    "models/refunds.sql": dedent(
        """
        MODEL (description "Refunds.", materialized table);

        SELECT refund_id, amount_cents FROM __source("raw_refunds")
        """
    ).strip()
    + "\n",
    "sources/raw.yml": _SOURCES.format(expression=_EXPRESSION) + "\n",
}
_SELECTION: list[str] = [
    "model:daily_orders",
    "model:order_count",
    "model:orders",
    "model:refunds",
]
_UNKNOWN: list[str] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
]
_HELD: str = "Planning from the published compiled project."
_SYNCED_READING: str = "Reading the synced warehouse state..."
_REPLAYED: str = (
    "cursor window of daily_orders (the run starts it away from the last build's cursor)"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanPreviewTestCase(
            description="matching observations plan like the live warehouse",
            edited_files={},
            injected_sql=(),
            expected_window_models=("orders",),
            expected_unplanned_cursor_models=(),
            expected_added_unknowns=(_REPLAYED,),
        ),
        PlanPreviewTestCase(
            description="a cursor_max from no successful build is not a cursor position",
            edited_files={},
            injected_sql=(
                "INSERT INTO main._sqlbuild_observations VALUES ('injected-orders', 'cursor_max', "
                "'orders', 'timestamp', '2099-01-01 00:00:00', 'injected-run', "
                "'2099-01-01 00:00:00')",
            ),
            expected_window_models=(),
            expected_unplanned_cursor_models=("orders",),
            expected_added_unknowns=(
                "cursor position of orders (no successful build recorded it)",
                _REPLAYED,
            ),
        ),
        PlanPreviewTestCase(
            description="an edited expression plans its declared columns and says so",
            edited_files={
                "sources/raw.yml": _SOURCES.format(expression=f"{_EXPRESSION} WHERE true") + "\n"
            },
            injected_sql=(),
            expected_window_models=("orders",),
            expected_unplanned_cursor_models=(),
            expected_added_unknowns=(
                _REPLAYED,
                "query columns of expression source raw_refunds (planned as declared)",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_synced_location_when_a_plan_is_requested_then_it_plans_from_the_copy_without_the_warehouse(
    test_case: PlanPreviewTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=_PROJECT_FILES)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    with duckdb.connect(project / "warehouse.db") as warehouse:
        for statement in test_case.injected_sql:
            _ = warehouse.execute(statement)
    write_files(project_dir=project, files=test_case.edited_files)
    live_run: subprocess.CompletedProcess[str] = live_plan(project, _SELECTION)
    assert live_run.returncode == 0, live_run.stderr
    live: dict[str, Any] = json.loads(live_run.stdout)
    log: Path = tmp_path / "location.log"
    refused, preview = plan_from_copy(tmp_path, project, _SELECTION, {})

    assert refused.returncode != 0
    assert "synced_copy" in preview, log.read_text()
    synced: dict[str, Any] = preview.pop("synced_copy")
    assert synced["synced_at"] is not None
    assert synced["unknown_until_run"] == [*_UNKNOWN, *test_case.expected_added_unknowns]
    live_models: dict[str, dict[str, Any]] = {model["name"]: model for model in live.pop("models")}
    preview_models: dict[str, dict[str, Any]] = {
        model["name"]: model for model in preview.pop("models")
    }
    live_replayed: dict[str, Any] = live_models.pop("daily_orders")
    preview_replayed: dict[str, Any] = preview_models.pop("daily_orders")
    assert live_replayed["cursor"]["resolved_bounds"]["start"] is not None
    assert (preview_replayed["action"], preview_replayed["reason"]) == (
        live_replayed["action"],
        live_replayed["reason"],
    )
    live_freshness: dict[str, Any] = live["metadata"].pop("direct_source_freshness")
    preview_freshness: dict[str, Any] = preview["metadata"].pop("direct_source_freshness")
    assert live_freshness["observed_source_names"] == ["raw_refunds"]
    assert preview_freshness["unknown_source_details"] == [
        {
            "message": "a plan preview does not observe source freshness",
            "reason": "error",
            "source_name": "raw_refunds",
        }
    ]
    assert synced["cursor_window_starts"] == {
        name: live_models[name]["cursor_bounds"]["start"]
        for name in test_case.expected_window_models
    }
    for name in (*test_case.expected_window_models, *test_case.expected_unplanned_cursor_models):
        for model in (live_models[name], preview_models.pop(name)):
            assert model["action"] == "incremental_append"
            assert model["reason"] == "normal_incremental"
        _ = live_models.pop(name)
    assert preview_models == live_models
    assert preview == live


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedPlanPreviewTestCase(
            description="an edited model is the only change the copy names",
            edited_model="models/order_count.sql",
            edit=("AS order_count", "AS order_count, 1 AS batch"),
            expected_changed={
                "counts": ["definition"],
                "reach": "job",
                "runs": [
                    {
                        "node": "model:order_count",
                        "reason": "planner change (query_changed)",
                        "downstream_of": None,
                    }
                ],
                "held_back": [],
                "unchanged": ["model:daily_orders", "model:orders", "model:refunds"],
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_edited_model_when_a_changed_plan_is_requested_then_the_copy_names_what_changed(
    test_case: ChangedPlanPreviewTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=_PROJECT_FILES)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    edited: str = _PROJECT_FILES[test_case.edited_model].replace(*test_case.edit)
    write_files(project_dir=project, files={test_case.edited_model: edited})
    live_run: subprocess.CompletedProcess[str] = live_plan(project, _SELECTION, "--changed")
    assert live_run.returncode == 0, live_run.stderr

    refused, preview = plan_from_copy(tmp_path, project, _SELECTION, {"changed": True})

    assert refused.returncode != 0
    assert preview.get("changed") == test_case.expected_changed, (
        tmp_path / "location.log"
    ).read_text()
    assert preview["changed"] == json.loads(live_run.stdout)["changed"]


def test_given_an_edited_model_when_the_location_plans_what_changed_then_it_publishes_what_the_changed_plan_runs(
    tmp_path: Path,
) -> None:
    project: Path = tmp_path / "project"
    summary: str = (
        'MODEL (description "Order count summary.", materialized table);\n\n'
        'SELECT order_count FROM __ref("order_count")\n'
    )
    write_files(project_dir=project, files=_PROJECT_FILES | {"models/order_summary.sql": summary})
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    edited: str = _PROJECT_FILES["models/order_count.sql"].replace(
        "AS order_count", "AS order_count, 1 AS batch"
    )
    write_files(project_dir=project, files={"models/order_count.sql": edited})
    # The selection the server freezes `sqb build --changed` to: every node a build runs.
    selection: list[str] = sorted([*_SELECTION, "model:order_summary", "seed:raw_orders"])

    published, plan = changed_build_and_plan_from_copy(tmp_path, project, selection)

    runs: list[dict[str, Any]] = plan["changed"]["runs"]
    assert published == {
        "changed": [run["node"] for run in runs if run["downstream_of"] is None],
        "downstream": [run["node"] for run in runs if run["downstream_of"] is not None],
    }, (tmp_path / "location.log").read_text()
    assert published == {"changed": ["model:order_count"], "downstream": ["model:order_summary"]}


_SYNCED: str = "Warehouse history synchronization completed"
_PLANNED: str = "Planned what changed"


def _logged(log: Path, text: str, count: int, process: subprocess.Popen[bytes]) -> str:
    """Wait until `text` has been logged `count` times; returns the log."""
    deadline: float = time.monotonic() + 45
    while (logged := log.read_text()).count(text) < count:
        assert process.poll() is None, logged
        assert time.monotonic() < deadline, logged
        time.sleep(0.05)
    return logged


def test_given_a_served_location_when_syncs_complete_then_it_replans_only_after_publication_or_an_applied_change(
    tmp_path: Path,
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=_PROJECT_FILES)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    edited: str = _PROJECT_FILES["models/order_count.sql"].replace(
        "AS order_count", "AS order_count, 1 AS batch"
    )
    write_files(project_dir=project, files={"models/order_count.sql": edited})
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"

    def changed_build() -> dict[str, list[str]] | None:
        with closing(sqlite3.connect(control)) as database:
            row: tuple[str] | None = database.execute(
                "SELECT report_json FROM sqb_changed_builds"
            ).fetchone()
        return None if row is None else json.loads(row[0])

    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        _ = wait_for_generation(api, "development", 1, location, log)
        first: str = _logged(log, _PLANNED, 1, location)
        assert first.index(_SYNCED) < first.index(_PLANNED), (
            "a publication plans what changed once its sync completes, not from the old copy"
        )
        assert changed_build() == {"changed": ["model:order_count"], "downstream": []}

        _ = wait_for_location_work(api, request_warehouse_sync(control), location, log)
        _ = _logged(log, _SYNCED, 2, location)
        # A later plan request runs on a later turn, after any plan the completed sync started.
        _ = wait_for_location_work(api, request_plan(control, ["model:orders"]), location, log)
        assert log.read_text().count(_PLANNED) == 1, "a sync that applied nothing keeps the count"

        rebuilt: subprocess.CompletedProcess[str] = run_sqb(
            project, "build", "--target", "development"
        )
        assert rebuilt.returncode == 0, rebuilt.stderr
        _ = wait_for_location_work(api, request_warehouse_sync(control), location, log)
        _ = _logged(log, _PLANNED, 2, location)
        assert changed_build() == {"changed": [], "downstream": []}, (
            "a sync that mirrored the new build plans again"
        )


@pytest.mark.parametrize(
    "test_case",
    [
        HeldPlanTestCase(
            description=(
                "only the plan that sets variables compiles the project again; the other two "
                "and the changed-build count plan from the published project"
            ),
            settings=(
                {"changed": True},
                {"changed": True, "vars": {"batch": "2"}},
                {"changed": True},
            ),
            expected_runs=((), ("model:order_count",), ()),
            expected_compiles=1,
            expected_held_plans=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_requests_when_the_location_plans_them_then_only_variables_compile_the_project_again(
    test_case: HeldPlanTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    files: dict[str, str] = _PROJECT_FILES | {
        "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"] + '[vars]\nbatch = "1"\n',
        "models/order_count.sql": _PROJECT_FILES["models/order_count.sql"].replace(
            "AS order_count", "AS order_count, @@batch AS batch"
        ),
    }
    write_files(project_dir=project, files=files)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    requests: list[tuple[list[str], dict[str, object]]] = [
        (["model:order_count"], settings) for settings in test_case.settings
    ]

    plans: list[dict[str, Any]] = plans_from_copy(tmp_path, project, requests)

    log: str = (tmp_path / "location.log").read_text()
    assert tuple(map(changed_runs, plans)) == test_case.expected_runs, log
    assert log.count("Compiling project...") == test_case.expected_compiles, log
    assert log.count(_HELD) == test_case.expected_held_plans, log
    assert log.count(_SYNCED_READING) == test_case.expected_held_plans + 1, log
    assert "Warehouse connected" not in log, "a synced copy opens no warehouse connection"
