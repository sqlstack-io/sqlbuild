"""Scheduled jobs with `changed` run only models whose consumed inputs changed, on DuckDB and SQLite."""

import os
import sqlite3
import sys
import time
from contextlib import closing
from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ChangedJobTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    changed_job_files,
    duckdb_rows,
    location_process,
    request_job_run,
    scheduled_worker,
    sqlite_rows,
)

ORDER_SKEW_SECONDS: float = 5.2
EXPECTED_CHANGED_RUNS: tuple[tuple[str, frozenset[str], dict[str, int]], ...] = (
    ("first run", frozenset({"stg_orders", "stg_customers", "customer_orders"}), {"success": 3}),
    ("nothing changed", frozenset(), {"success": 3}),
    ("a new order", frozenset({"stg_orders", "customer_orders"}), {"success": 3}),
)


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedJobTestCase(
            description="DuckDB job with changed runs only consumers of new orders",
            model_sql="SELECT 1 AS amount",
            warehouse_query="",
            expected_status="success",
            expected_rows=[],
            project_toml=(
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            warehouse_file="warehouse.duckdb",
            consumption_table="main._sqlbuild_consumption",
            raw_tables_sql=(
                "CREATE SCHEMA raw",
                "CREATE TABLE raw.orders AS SELECT 1 AS order_id, 1 AS customer_id, 1 AS load_id",
                "CREATE TABLE raw.customers AS SELECT 1 AS customer_id, 1 AS load_id",
            ),
            new_order_sql="INSERT INTO raw.orders VALUES (2, 1, 2)",
            execute=duckdb_rows,
            expected_runs=EXPECTED_CHANGED_RUNS,
        ),
        ChangedJobTestCase(
            description="SQLite job with changed runs only consumers of new orders",
            model_sql="SELECT 1 AS amount",
            warehouse_query="",
            expected_status="success",
            expected_rows=[],
            project_toml=(
                'name = "orders"\nadapter = "sqlite"\n'
                '[connections.local]\npath = "warehouse.sqlite"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            warehouse_file="warehouse.sqlite",
            consumption_table="_sqlbuild_consumption",
            raw_tables_sql=(
                "CREATE TABLE raw__orders (order_id INTEGER, customer_id INTEGER, load_id INTEGER)",
                "INSERT INTO raw__orders VALUES (1, 1, 1)",
                "CREATE TABLE raw__customers (customer_id INTEGER, load_id INTEGER)",
                "INSERT INTO raw__customers VALUES (1, 1)",
            ),
            new_order_sql="INSERT INTO raw__orders VALUES (2, 1, 2)",
            execute=sqlite_rows,
            expected_runs=EXPECTED_CHANGED_RUNS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_changed_job_when_inputs_change_between_runs_then_only_their_consumers_run(
    test_case: ChangedJobTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    test_case = replace(test_case, files=changed_job_files(test_case.project_toml))
    observed: list[tuple[str, frozenset[str], dict[str, int]]] = []
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        warehouse: Path = worker.project / test_case.warehouse_file
        for sql in test_case.raw_tables_sql:
            test_case.execute(warehouse, sql)
        recorded: set[tuple[Any, ...]] = set()
        for description, mutation in [
            ("first run", ()),
            ("nothing changed", ()),
            ("a new order", (test_case.new_order_sql,)),
        ]:
            time.sleep(ORDER_SKEW_SECONDS)
            for sql in mutation:
                test_case.execute(warehouse, sql)
            run: dict[str, Any] = request_job_run(worker, "changed_orders")
            assert run["status"] == "success", worker.log.read_text()
            started: set[tuple[Any, ...]] = set(
                test_case.execute(
                    warehouse,
                    f"SELECT event_id, node_name FROM {test_case.consumption_table} "
                    "WHERE phase = 'started'",
                )
            )
            observed.append(
                (
                    description,
                    frozenset(row[1] for row in started - recorded),
                    run["node_counts_json"],
                )
            )
            recorded |= started
        warehouse_events: set[str] = {
            row[0]
            for row in test_case.execute(
                warehouse, f"SELECT event_id FROM {test_case.consumption_table}"
            )
        }
        log: Path = tmp_path / "location.log"
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
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": worker.api.bearer},
        ) as location:
            deadline: float = time.monotonic() + 45
            mirrored: set[str] = set()
            while mirrored != warehouse_events:
                assert location.poll() is None, log.read_text()
                assert time.monotonic() < deadline, (warehouse_events - mirrored, log.read_text())
                with closing(sqlite3.connect(worker.control_store)) as control:
                    mirrored = {
                        row[0]
                        for row in control.execute(
                            "SELECT source_event_id FROM sqb_warehouse_consumption"
                        )
                    }
                time.sleep(0.2)

    assert tuple(observed) == test_case.expected_runs
