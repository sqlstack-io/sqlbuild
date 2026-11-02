"""A failed sample must leave catalog reconciliation able to remove dropped relations."""

import json
import os
import sys
import time
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    StatisticsRecoveryTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    catalog_facts,
    wait_for_catalog,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    open_legacy_warehouse,
    snapshot_api,
    wait_for_generation,
)


@pytest.mark.parametrize(
    "test_case",
    [StatisticsRecoveryTestCase("Drop after discovery before sampling", (True, False))],
    ids=lambda case: case.description,
)
def test_given_relation_dropped_during_sampling_when_read_fails_then_catalog_can_reconcile(
    test_case: StatisticsRecoveryTestCase, tmp_path: Path
) -> None:
    fixture: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/warehouse_statistics.json").read_text()
    )["drop_during_sampling"]
    project: Path = tmp_path / "project"
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(project, "sqlite", "path")
    _ = adapter.execute(connection=connection, sql=fixture["create_sql"])
    adapter.close(connection)
    (project / "orchestration/orders.py").write_text("")
    (project / "models").mkdir()
    (project / "models/orders.sql").write_text(fixture["model"])
    hook: Path = tmp_path / "sampling-hook"
    hook.mkdir()
    dropped: Path = hook / "dropped"
    (hook / "sitecustomize.py").write_text(
        "from pathlib import Path\n"
        "import sqlite3\n"
        "from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter\n"
        "original = SqliteAdapter.estimate_relation_row_count\n"
        "def sample(self, *, connection, database, schema, name, metadata_only=False):\n"
        f"    if name == {fixture['name']!r} and metadata_only:\n"
        f"        marker = Path({str(dropped)!r})\n"
        "        if not marker.exists():\n"
        f"            with sqlite3.connect({str(project / 'warehouse.db')!r}) as writer:\n"
        f"                writer.execute({fixture['drop_sql']!r})\n"
        "            marker.write_text('dropped after discovery')\n"
        f"        raise RuntimeError({fixture['failure']!r})\n"
        "    return original(self, connection=connection, database=database, schema=schema, "
        "name=name, metadata_only=metadata_only)\n"
        "SqliteAdapter.estimate_relation_row_count = sample\n"
    )
    log: Path = tmp_path / "location.log"
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
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
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer, "PYTHONPATH": str(hook)},
        ) as location,
    ):
        _ = wait_for_generation(api, "development", 1, location, log)
        deadline: float = time.monotonic() + 30
        while not dropped.exists():
            assert location.poll() is None, log.read_text()
            assert time.monotonic() < deadline, log.read_text()
            time.sleep(0.05)
        wait_for_catalog(control, [], location, log)
        assert (
            tuple(fact["present"] for fact in catalog_facts(control))
            == test_case.expected_catalog_presence
        )
        assert "Warehouse history read failed" in log.read_text()
