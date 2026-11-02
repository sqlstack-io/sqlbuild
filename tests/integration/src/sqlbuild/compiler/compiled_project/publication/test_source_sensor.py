"""An external warehouse writer drives a real sensor, queue and isolated DuckDB worker."""

import json
import os
import sqlite3
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any

import duckdb
import pytest

from sqlbuild._native import (
    SourceFreshnessRecord,
    SourceFreshnessRenderers,
    write_source_freshness_records,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    scheduled_worker,
    wait_for_generation,
    wait_for_run,
)


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "changed external source versions",
            "SELECT 73 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(73,)],
            expected_origin="sensor",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.py"
                ).read_text(),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
        ),
        WorkerExecutionTestCase(
            "stale external timestamp versions",
            "SELECT 73 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(73,)],
            expected_origin="sensor",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/stale_orders.py"
                ).read_text(),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/managed_orders.yml"
                ).read_text(),
                "python/loaders/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/orders_loader.py"
                ).read_text(),
                "source_observations.json": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_stale_journey.json"
                ).read_text(),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_external_source_versions_when_a_location_resyncs_then_only_changed_data_starts_real_work(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    data: dict[str, Any] = json.loads(
        test_case.files.get(
            "source_observations.json",
            Path(
                "benchmarks/duckshop/edge_cases/orchestration/source_data_sensor.json"
            ).read_text(),
        )
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        for phase, (source, count) in enumerate(
            (
                (data["record"], data["source_rows"]),
                (data["record"] | data["late_record"], 1),
            )
        ):
            values: dict[str, Any] = source.copy()
            for key in ("event_id", "state_database", "state_schema", "observed_at"):
                values.pop(key)
            records: tuple[SourceFreshnessRecord, ...] = tuple(
                SourceFreshnessRecord(
                    **(
                        values
                        | {
                            "run_id": f"{source['run_id']}-{phase}-{index}",
                            "observed_at": datetime.fromisoformat(source["observed_at"])
                            + timedelta(microseconds=index),
                        }
                    )
                )
                for index in range(count)
            )
            adapter: Any = DuckDbAdapter()
            connection: Any = adapter.connect(
                {"database": str(worker.project / "warehouse.duckdb")}
            )
            renderers: SourceFreshnessRenderers = SourceFreshnessRenderers(
                render_qualified_name=adapter.render_qualified_name,
                render_framework_type=adapter.render_framework_type,
                render_insert_records_sql=adapter.render_insert_source_freshness_records_sql,
                render_create_table_sql=adapter.render_create_source_freshness_table_sql,
                read_columns=adapter.get_columns,
                render_add_columns=adapter.render_add_columns,
            )
            try:
                for batch in (records, records[:1] * 3):
                    write_source_freshness_records(
                        connection=connection,
                        execute=adapter.execute,
                        database=None,
                        schema="main",
                        records=batch,
                        renderers=renderers,
                    )
            finally:
                adapter.close(connection)
            log: Path = tmp_path / f"location-{phase}.log"
            with location_process(
                [
                    str(Path(sys.executable).with_name("sqb")),
                    "--project-dir",
                    str(worker.project),
                    "location",
                    "serve",
                    "--server",
                    worker.api.url,
                    "--location-id",
                    worker.api.location_id,
                    "--target",
                    "development",
                ],
                log,
                tmp_path / f"location-{phase}.stdout",
                env=os.environ | {"SQLBUILD_LOCATION_TOKEN": worker.api.bearer},
            ) as location:
                _ = wait_for_generation(worker.api, "development", phase + 2, location, log)
                deadline: float = time.monotonic() + 30
                observed: int = 0
                runs: list[tuple[str]] = []
                while observed != data["source_rows"] + phase or len(runs) != phase + 1:
                    assert location.poll() is None, log.read_text()
                    assert time.monotonic() < deadline, log.read_text() + worker.log.read_text()
                    with sqlite3.connect(worker.control_store) as control:
                        observed = control.execute(
                            "SELECT COUNT(*) FROM sqb_warehouse_freshness"
                        ).fetchone()[0]
                        runs = control.execute(
                            "SELECT run_id FROM sqb_runs WHERE origin='sensor' ORDER BY queued_at,run_id"
                        ).fetchall()
                    assert len(runs) <= phase + 1, (
                        "unchanged observations and exact retries must not create runs"
                    )
                    time.sleep(0.05)
                result: dict[str, Any] = wait_for_run(
                    worker.api, worker.process, worker.log, run_id=runs[-1][0]
                )
                assert result["status"] == test_case.expected_status
                assert result["job_key"] == "orders_refresh"
                assert result["node_counts_json"] == {"success": 1}
            with duckdb.connect(
                str(worker.project / "warehouse.duckdb"), read_only=True
            ) as warehouse:
                assert (
                    warehouse.execute(test_case.warehouse_query).fetchall()
                    == test_case.expected_rows
                )
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT COUNT(*) FROM sqb_run_requests").fetchone() == (2,)
            assert control.execute(
                "SELECT COUNT(*) FROM recall_events WHERE producer='warehouse'"
            ).fetchone() == (105,)
