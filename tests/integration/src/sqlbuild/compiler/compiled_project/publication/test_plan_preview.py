"""A location plans from the server's synced copy and never opens the warehouse to do it."""

import json
import os
import subprocess
import sys
import time
from datetime import UTC, datetime
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    PlanPreviewTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    request_plan,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    snapshot_api,
    wait_for_generation,
    wait_for_location_work,
    wait_for_warehouse_sync_health,
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
    "orchestration/orders.py": "",
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
_SELECTION: list[str] = ["model:order_count", "model:orders", "model:refunds"]
_UNKNOWN: list[str] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
]


def _sqb(project: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(Path(sys.executable).with_name("sqb")), "--project-dir", str(project), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )


def _live_plan(project: Path) -> subprocess.CompletedProcess[str]:
    selected: list[str] = [part for key in _SELECTION for part in ("--select", key.split(":")[1])]
    return _sqb(project, "plan", "--target", "development", "--json", "--no-color", *selected)


def _locked(database: Path) -> duckdb.DuckDBPyConnection:
    """Hold the warehouse's write lock, so any process that opens it fails."""
    deadline: float = time.monotonic() + 15
    while True:
        try:
            return duckdb.connect(database)
        except duckdb.IOException:
            assert time.monotonic() < deadline
            time.sleep(0.05)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanPreviewTestCase(
            description="matching observations plan like the live warehouse",
            edited_files={},
            injected_sql=(),
            window_models=("orders",),
            unplanned_cursor_models=(),
            added_unknowns=(),
        ),
        PlanPreviewTestCase(
            description="a cursor_max from no successful build is not a cursor position",
            edited_files={},
            injected_sql=(
                "INSERT INTO main._sqlbuild_observations VALUES ('injected-orders', 'cursor_max', "
                "'orders', 'timestamp', '2099-01-01 00:00:00', 'injected-run', "
                "'2099-01-01 00:00:00')",
            ),
            window_models=(),
            unplanned_cursor_models=("orders",),
            added_unknowns=("cursor position of orders (no successful build recorded it)",),
        ),
        PlanPreviewTestCase(
            description="an edited expression plans its declared columns and says so",
            edited_files={
                "sources/raw.yml": _SOURCES.format(expression=f"{_EXPRESSION} WHERE true") + "\n"
            },
            injected_sql=(),
            window_models=("orders",),
            unplanned_cursor_models=(),
            added_unknowns=(
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
    built: subprocess.CompletedProcess[str] = _sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    with duckdb.connect(project / "warehouse.db") as warehouse:
        for statement in test_case.injected_sql:
            _ = warehouse.execute(statement)
    write_files(project_dir=project, files=test_case.edited_files)
    live_run: subprocess.CompletedProcess[str] = _live_plan(project)
    assert live_run.returncode == 0, live_run.stderr
    live: dict[str, Any] = json.loads(live_run.stdout)
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"
    started: datetime = datetime.now(UTC)
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
        _ = wait_for_warehouse_sync_health(
            api,
            {"failure": None, "tables": ["catalog", "consumption", "fingerprints", "observations"]},
            location,
            log,
            after=started,
        )
        lock: duckdb.DuckDBPyConnection = _locked(project / "warehouse.db")
        try:
            refused: subprocess.CompletedProcess[str] = _live_plan(project)
            request_id: str = request_plan(control, _SELECTION)
            result: dict[str, Any] = wait_for_location_work(api, request_id, location, log)
        finally:
            lock.close()

    assert refused.returncode != 0
    outcome: dict[str, Any] = result["result_json"]["outcome"]
    assert outcome["kind"] == "plan", outcome
    preview: dict[str, Any] = outcome["document"]
    assert "synced_copy" in preview, log.read_text()
    synced: dict[str, Any] = preview.pop("synced_copy")
    assert synced["synced_at"] is not None
    assert synced["unknown_until_run"] == [*_UNKNOWN, *test_case.added_unknowns]
    live_models: dict[str, dict[str, Any]] = {model["name"]: model for model in live.pop("models")}
    preview_models: dict[str, dict[str, Any]] = {
        model["name"]: model for model in preview.pop("models")
    }
    assert synced["cursor_window_starts"] == {
        name: live_models[name]["cursor_bounds"]["start"] for name in test_case.window_models
    }
    for name in (*test_case.window_models, *test_case.unplanned_cursor_models):
        for model in (live_models[name], preview_models.pop(name)):
            assert model["action"] == "incremental_append"
            assert model["reason"] == "normal_incremental"
        _ = live_models.pop(name)
    assert preview_models == live_models
    assert preview == live
