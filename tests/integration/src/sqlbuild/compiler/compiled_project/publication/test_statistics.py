"""Statistics sampling failures and expiration never disturb catalog reconciliation."""

import json
import os
import sqlite3
import subprocess
import sys
import time
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    StatisticsHorizonRejectionTestCase,
    StatisticsHorizonTestCase,
    StatisticsRecoveryTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    catalog_facts,
    wait_for_catalog,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    open_legacy_warehouse,
    run_statistics_retention,
    snapshot_api,
    statistics_relations,
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


@pytest.mark.parametrize(
    "test_case",
    [
        StatisticsHorizonTestCase(
            "Default horizon keeps a two-day-old sample",
            (),
            ("customers_two_days", "customers_recent"),
        ),
        StatisticsHorizonTestCase(
            "One-day horizon deletes the two-day-old sample",
            ("--statistics-retention-days", "1"),
            ("customers_recent",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_configured_statistics_horizon_when_server_starts_then_older_samples_are_deleted(
    test_case: StatisticsHorizonTestCase, tmp_path: Path
) -> None:
    record: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/warehouse_statistics.json").read_text()
    )["record"]
    _ = run_statistics_retention(tmp_path, ())
    now: datetime = datetime.now(UTC)
    location_id: str = str(uuid4())
    with sqlite3.connect(tmp_path / ".sqlbuild/control.sqlite3") as connection:
        _ = connection.execute(
            "INSERT INTO sqb_locations (location_id,name,created_by,created_at) "
            "VALUES (?, 'orders', 'test-admin', ?)",
            (location_id, now.isoformat()),
        )
        _ = connection.executemany(
            "INSERT INTO sqb_warehouse_statistics (event_id,location_id,target,state_database,"
            "state_schema,relation_name,observed_at,record_json,received_at) "
            "VALUES (?,?,'development',NULL,'main',?,?,?,?)",
            [
                (
                    str(uuid4()),
                    location_id,
                    name,
                    (now - age).isoformat(),
                    json.dumps(record | {"name": name}),
                    now.isoformat(),
                )
                for name, age in [
                    ("sentinel_expired", timedelta(days=100)),
                    ("customers_two_days", timedelta(days=2)),
                    ("customers_recent", timedelta(hours=1)),
                ]
            ],
        )
    log: str = run_statistics_retention(tmp_path, test_case.server_arguments)
    assert "Control API ready" in log
    assert statistics_relations(tmp_path) == test_case.expected_relations


@pytest.mark.parametrize(
    "test_case",
    [
        StatisticsHorizonRejectionTestCase(
            "Zero days", "0", "--statistics-retention-days must be between 1 and 65535"
        ),
        StatisticsHorizonRejectionTestCase(
            "Beyond the supported range",
            "65536",
            "--statistics-retention-days must be between 1 and 65535",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_statistics_horizon_when_server_starts_then_it_refuses_before_storage(
    test_case: StatisticsHorizonRejectionTestCase, tmp_path: Path
) -> None:
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "server",
            "--port",
            "0",
            "--statistics-retention-days",
            test_case.days,
        ],
        cwd=tmp_path,
        env=environment,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert result.returncode != 0
    assert test_case.expected_error in result.stderr
    assert result.stdout == ""
    assert not (tmp_path / ".sqlbuild/control.sqlite3").exists()
