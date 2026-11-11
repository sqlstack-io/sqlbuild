"""Retained compiler baselines cross the native worker boundary before lifecycle writes."""

import json
import os
import signal
import sqlite3
import time
from dataclasses import replace
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen
from uuid import uuid4

import duckdb
import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    NativeOrchestrationSnapshot,
    build_compiled_project,
    build_orchestration_snapshot,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    METADATA,
    TYPE_PROOF_FILES,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CheckExecutionTestCase,
    ClaimedWorker,
    GroupExecutionTestCase,
    SnapshotWorkerTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    cancel_running_statement,
    claimed_worker,
    running_statement,
    scheduled_worker,
    snapshot_api,
    wait_for_log,
    wait_for_run,
    worker_result,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        GroupExecutionTestCase(
            "group execution preserves independent downstream work",
            "WITH source(value) AS (VALUES ('invalid')) SELECT CAST(value AS INTEGER) AS amount FROM source",
            "SELECT amount FROM orders",
            "partial",
            [(11,)],
            expected_child_statuses={"staging": ("partial",), "marts": ("partial",)},
        ),
        GroupExecutionTestCase(
            "successful jobs honor compiled boundaries",
            "SELECT 17 AS amount",
            "SELECT amount FROM totals",
            "success",
            [(17,)],
            expected_child_statuses={"staging": ("success",), "marts": ("success",)},
            expected_totals=("success", None),
        ),
        GroupExecutionTestCase(
            "a platform sensor starts group parents and children",
            "SELECT 17 AS amount",
            "SELECT amount FROM totals",
            "success",
            [(17,)],
            expected_origin="sensor",
            expected_child_statuses={"staging": ("success",), "marts": ("success",)},
            expected_totals=("success", None),
            files={
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import PlatformEventKind, job, job_group, platform_event, sensor\n"
                    "staging = job('staging', description='Stage orders.', select='stg_orders broken_orders')\n"
                    "marts = job('marts', description='Build independent marts.', select='orders totals')\n"
                    "annual_orders = job_group('annual_orders', description='Stage then build marts.', jobs=[marts, staging])\n"
                    "health_orders = sensor('health_orders', description='Refresh on platform heartbeat.', "
                    "when=platform_event(kinds=[PlatformEventKind.HEARTBEAT]), run=annual_orders, "
                    "minimum_interval_seconds=60)\n"
                )
            },
        ),
        GroupExecutionTestCase(
            "critical node stops the group under the default policy",
            "WITH source(value) AS (VALUES ('invalid')) SELECT CAST(value AS INTEGER) AS amount FROM source",
            "SELECT count(*) FROM information_schema.tables WHERE table_name IN ('orders','totals')",
            "failed",
            [(0,)],
            expected_child_statuses={"staging": ("partial", "cancelled"), "marts": ("cancelled",)},
            expected_totals=("cancelled", None),
            expected_marts_started=False,
            files={
                "models/broken_orders.sql": (
                    "MODEL (description 'Critical orders.', materialized table, critical true);\n"
                    "WITH source(value) AS (VALUES ('invalid')) SELECT CAST(value AS INTEGER) AS amount FROM source\n"
                ),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_scheduled_group_when_workers_execute_then_dependencies_and_failure_policy_hold(
    test_case: GroupExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project_dir: Path = tmp_path / "project"
    project_dir.mkdir()
    with duckdb.connect(str(project_dir / "warehouse.duckdb")) as warehouse:
        warehouse.execute("CREATE TABLE broken_orders AS SELECT 99 AS amount")
    test_case = replace(
        test_case,
        files={
            "models/orders.sql": (
                "MODEL (description 'Independent orders.', materialized table);\n"
                'SELECT amount FROM __ref("stg_orders")\n'
            ),
            "models/stg_orders.sql": (
                "MODEL (description 'Staged orders.', materialized table);\nSELECT 11 AS amount\n"
            ),
            "models/broken_orders.sql": (
                "MODEL (description 'Source values.', materialized table);\n" + test_case.model_sql
            ),
            "models/totals.sql": (
                "MODEL (description 'Dependent totals.', materialized table);\n"
                'SELECT amount FROM __ref("broken_orders")\n'
            ),
            "orchestration/orders.py": (
                "from sqlbuild.orchestration import job, job_group, schedule\n"
                'staging = job("staging", description="Stage orders.", '
                'select="stg_orders broken_orders")\n'
                'marts = job("marts", description="Build independent marts.", select="orders totals")\n'
                'annual_orders = job_group("annual_orders", description="Stage then build marts.", '
                "jobs=[marts, staging], "
                'schedule=schedule(cron="0 0 * * *", timezone="UTC", '
                'starts_at="2000-01-01T00:00:00Z"))\n'
            ),
        }
        | test_case.files,
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        parent: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert parent["status"] == test_case.expected_status, worker.log.read_text()
        request: Request = Request(
            f"{worker.api.url}/api/locations/{worker.api.location_id}/targets/development/runs",
            headers={"Authorization": f"Bearer {worker.api.bearer}"},
        )
        with urlopen(request, timeout=5) as response:
            runs: dict[str, Any] = {run["job_key"]: run for run in json.load(response)}
        assert set(runs) == {"annual_orders", *test_case.expected_child_statuses}
        assert all(
            runs[name]["status"] in statuses
            for name, statuses in test_case.expected_child_statuses.items()
        )
        assert (runs["marts"]["started_at"] is not None) == test_case.expected_marts_started
        assert datetime.fromisoformat(runs["staging"]["ended_at"]) <= datetime.fromisoformat(
            runs["marts"]["started_at"] or runs["staging"]["ended_at"]
        )
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute(
                "SELECT origin FROM sqb_runs WHERE run_id=?", (parent["run_id"],)
            ).fetchone() == (test_case.expected_origin,)
            assert (
                control.execute(
                    "SELECT parent_run_id, root_run_id FROM sqb_runs WHERE kind='job' ORDER BY job_key"
                ).fetchall()
                == [(runs["annual_orders"]["run_id"],) * 2] * 2
            )
            assert (
                control.execute(
                    "SELECT status, skipped_because_of FROM sqb_run_nodes WHERE run_id=? AND node_key='model:totals'",
                    (runs["marts"]["run_id"],),
                ).fetchone()
                == test_case.expected_totals
            )
            assert control.execute(
                "SELECT status FROM sqb_run_nodes WHERE run_id=? AND node_key='model:broken_orders'",
                (runs["staging"]["run_id"],),
            ).fetchone() == ({"success": "success"}.get(test_case.expected_status, "failed"),)
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone() == (0,)
        with duckdb.connect(str(project_dir / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "a stale source blocks only its own branch",
            "SELECT 11 AS amount",
            "SELECT amount FROM orders UNION ALL SELECT return_id FROM returns ORDER BY 1",
            "partial",
            [(2,), (11,)],
            files={
                "sources/raw.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/worker_freshness_skip.yml"
                ).read_text(),
                "models/stale_orders.sql": (
                    "MODEL (description 'Stale orders.', materialized table);\n"
                    'SELECT order_id FROM __source("raw_orders")\n'
                ),
                "models/late_orders.sql": (
                    "MODEL (description 'Late orders.', materialized table);\n"
                    'SELECT order_id FROM __source("raw_orders")\n'
                ),
                "models/returns.sql": (
                    "MODEL (description 'Returns.', materialized table);\n"
                    'SELECT return_id FROM __source("raw_returns")\n'
                ),
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import job, schedule\n"
                    'annual_orders = job("annual_orders", '
                    'select="orders stale_orders late_orders returns", '
                    'description="Build orders.", '
                    'schedule=schedule(cron="0 0 * * *", timezone="UTC", '
                    'starts_at="2000-01-01T00:00:00Z"))\n'
                ),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_stale_source_when_a_worker_builds_then_blocked_nodes_finish_skipped_unstarted(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            nodes: list[tuple[str, str, bool]] = [
                (key, status, started is not None)
                for key, status, started in control.execute(
                    "SELECT node_key, status, started_at FROM sqb_run_nodes "
                    "WHERE run_id=? AND kind='node' ORDER BY node_key",
                    (run["run_id"],),
                )
            ]
        assert nodes == [
            ("model:late_orders", "skipped", False),
            ("model:orders", "success", True),
            ("model:returns", "success", True),
            ("model:stale_orders", "skipped", False),
        ], worker.log.read_text()
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            description="opposing parent and independent child limits",
            model_sql="SELECT 11 AS amount",
            warehouse_query="SELECT amount FROM orders UNION ALL SELECT amount FROM inventory ORDER BY amount",
            expected_status="success",
            expected_rows=[(11,), (23,)],
            concurrency_limits={"orders_limit": 1, "inventory_limit": 1},
            files={
                "models/inventory.sql": (
                    "MODEL (description 'Inventory.', materialized table);\nSELECT 23 AS amount\n"
                ),
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import job, job_group, schedule\n"
                    "orders = job('orders_job', description='Build orders.', select='orders', "
                    "concurrency_groups=('inventory_limit',))\n"
                    "inventory = job('inventory_job', description='Build inventory.', select='inventory', "
                    "concurrency_groups=('orders_limit',))\n"
                    "annual_orders = job_group('annual_orders', description='Orders group.', jobs=[orders], "
                    "concurrency_groups=('orders_limit',), "
                    "schedule=schedule(cron='0 0 * * *', timezone='UTC', starts_at='2000-01-01T00:00:00Z'))\n"
                    "annual_inventory = job_group('annual_inventory', description='Inventory group.', "
                    "jobs=[inventory], concurrency_groups=('inventory_limit',), "
                    "schedule=schedule(cron='0 0 * * *', timezone='UTC', starts_at='2000-01-01T00:00:00Z'))\n"
                ),
            },
            store_setup=(
                "CREATE TABLE test_slot_claims (run_id TEXT, group_key TEXT, occupied INTEGER);\n"
                "CREATE TRIGGER test_observe_slots AFTER INSERT ON sqb_concurrency_slots BEGIN "
                "INSERT INTO test_slot_claims SELECT NEW.run_id, NEW.group_key, count(*) "
                "FROM sqb_concurrency_slots WHERE group_key=NEW.group_key; END;\n"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_opposing_group_limits_when_scheduled_then_each_group_holds_every_limit_and_both_finish(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        orders: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        inventory: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_inventory"
        )
        assert orders["status"] == inventory["status"] == "success", worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute(
                "SELECT r.job_key, c.group_key, max(c.occupied) FROM test_slot_claims c "
                "JOIN sqb_runs r ON r.run_id=c.run_id GROUP BY r.job_key, c.group_key ORDER BY r.job_key"
            ).fetchall() == [
                ("annual_inventory", "inventory_limit", 1),
                ("annual_inventory", "orders_limit", 1),
                ("annual_orders", "inventory_limit", 1),
                ("annual_orders", "orders_limit", 1),
            ]
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE status='success'"
            ).fetchone() == (4,)
            assert control.execute("SELECT count(*) FROM sqb_concurrency_slots").fetchone() == (0,)
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotWorkerTestCase(
            description="unselected dependency drift",
            model_file="models/stg_orders.sql",
            original_sql="CAST(5 AS INTEGER)",
            replacement_sql="CAST(17 AS INTEGER)",
            expected_error="node content differs",
        ),
        SnapshotWorkerTestCase(
            description="selected model drift",
            model_file="models/orders.sql",
            original_sql="SELECT order_id, amount",
            replacement_sql="SELECT order_id, amount * CAST(2 AS BIGINT) AS amount",
            expected_error="node content differs",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_retained_baseline_when_starting_a_worker_then_verification_and_claim_lifetime_are_native(
    test_case: SnapshotWorkerTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = tmp_path / "project"
    write_files(project_dir=project_dir, files=TYPE_PROOF_FILES)
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    with snapshot_api(tmp_path / "control") as api:
        metadata: dict[str, Any] = METADATA | {"location_id": api.location_id}
        baseline: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            project, json.dumps(metadata), project_dir, discovered_inputs=discovered
        )
        publication: dict[str, Any] = json.loads(baseline.publish(api.url, api.bearer))
        claim: ClaimedWorker = claimed_worker(tmp_path / "control", api, publication)
        identity: tuple[str, str] = (claim.run_id, claim.claim_id)
        changed: dict[str, str] = TYPE_PROOF_FILES | {
            test_case.model_file: TYPE_PROOF_FILES[test_case.model_file].replace(
                test_case.original_sql, test_case.replacement_sql
            )
        }
        write_files(project_dir=project_dir, files=changed)
        discovered = discover_project_inputs(project_dir=project_dir)
        drifted: CompiledProjectView = build_compiled_project(
            discovered_inputs=discovered,
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        wrong: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            drifted, json.dumps(metadata), project_dir, discovered_inputs=discovered
        )
        with pytest.raises(RuntimeError, match=test_case.expected_error):
            _ = wrong.start_worker(api.url, claim.bearer, identity)
        runs_url: str = f"{api.url}/api/locations/{api.location_id}/targets/development/runs"
        location_headers: dict[str, str] = {"Authorization": f"Bearer {api.bearer}"}
        with urlopen(Request(runs_url, headers=location_headers), timeout=5) as response:
            assert json.load(response)[0]["status"] == "queued"
        with pytest.raises(RuntimeError, match="does not match"):
            _ = baseline.start_worker(api.url, claim.bearer, (claim.run_id, str(uuid4())))
        with baseline.start_worker(api.url, claim.bearer, identity) as worker:
            execution: dict[str, Any] = json.loads(worker.execution_json())
            assert execution["claim"]["request"]["selection"] == ["model:orders"]
            assert execution["claim"]["request"]["settings"] == {"full_refresh": True}
            assert frozenset(execution["node_hashes"]) == {
                "model:orders",
                "model:stg_orders",
            }
            assert not worker.cancel_requested()
            event_id: str = str(uuid4())
            change: str = json.dumps({"kind": "node_started", "node_key": "model:orders"})
            assert worker.report(event_id, change)
            assert not worker.report(event_id, change)
            assert claim.bearer not in repr(worker)
        worker.close()
        assert worker.cancel_requested()
        with pytest.raises(RuntimeError, match="closed"):
            _ = worker.execution_json()
        with pytest.raises(RuntimeError, match="closed"):
            _ = worker.report(str(uuid4()), change)
        with pytest.raises(RuntimeError, match="closed"), worker:
            pytest.fail("closed worker cannot be entered")
        # Closing an interpreter-side owner is not proof of physical warehouse termination.
        with urlopen(Request(runs_url, headers=location_headers), timeout=5) as response:
            assert json.load(response)[0]["status"] == "running"
        with baseline.start_worker(api.url, claim.bearer, identity) as resumed:
            assert resumed.report(
                str(uuid4()),
                json.dumps(
                    {
                        "kind": "node_finished",
                        "node_key": "model:orders",
                        "outcome": "success",
                        "rows_affected": 7,
                        "error_code": None,
                        "skipped_because_of": None,
                    }
                ),
            )
            assert resumed.report(
                str(uuid4()),
                json.dumps(
                    {
                        "kind": "finished",
                        "outcome": "success",
                        "reason": "worker execution completed",
                        "exit_code": 0,
                    }
                ),
            )
        with urlopen(Request(runs_url, headers=location_headers), timeout=5) as response:
            result: dict[str, Any] = json.load(response)[0]
        assert result["status"] == "success"
        assert result["node_counts_json"] == {"success": 1}


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "successful model writes actual rows",
            "WITH source(value) AS (VALUES ('23')) SELECT CAST(value AS BIGINT) AS amount FROM source",
            "SELECT amount FROM orders",
            "success",
            [(23,)],
        ),
        WorkerExecutionTestCase(
            "a platform sensor catches up retained events through a real worker",
            "SELECT 47 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(47,)],
            expected_origin="sensor",
            files={
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import PlatformEventKind, job, platform_event, sensor\n"
                    "annual_orders = job('annual_orders', select='orders', description='Build orders.')\n"
                    "health_orders = sensor('health_orders', description='Refresh on platform heartbeat.', "
                    "when=platform_event(kinds=[PlatformEventKind.HEARTBEAT]), run=annual_orders, "
                    "minimum_interval_seconds=60)\n"
                )
            },
        ),
        WorkerExecutionTestCase(
            "authored variables and reload use ordinary build argument conversion",
            "SELECT @@batch AS amount",
            "SELECT amount FROM orders",
            "success",
            [(23,)],
            files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n[vars]\nbatch = "3"\n'
                    '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                    '[targets.development]\nschema = "main"\n'
                ),
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import job, schedule\n"
                    "annual_orders = job('annual_orders', select='orders', description='Build orders.', "
                    "settings={'vars': {'batch': 23}, 'reload': True, 'run_tests': False}, "
                    "schedule=schedule(cron='0 0 * * *', timezone='UTC', "
                    "starts_at='2000-01-01T00:00:00Z'))\n"
                ),
            },
        ),
        WorkerExecutionTestCase(
            "a selected upstream source succeeds once the build returns",
            'SELECT order_id AS amount FROM __source("raw_orders")',
            "SELECT amount FROM orders",
            "success",
            [(23,)],
            expected_node_count=2,
            files={
                "sources/raw_orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/upstream_source_orders.py"
                ).read_text(),
            },
            warehouse_setup=lambda path: (
                duckdb.connect(str(path))
                .execute("CREATE TABLE raw_orders AS SELECT CAST(23 AS BIGINT) AS order_id")
                .close()
            ),
        ),
        WorkerExecutionTestCase(
            "a failed build with a selected source fails rather than partly succeeds",
            "WITH bad(value) AS (VALUES ('invalid')) "
            """SELECT CAST(value AS BIGINT) + order_id AS amount FROM __source("raw_orders") CROSS JOIN bad""",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "failed",
            [(0,)],
            expected_node_counts={"success": 1, "failed": 1},
            files={
                "sources/raw_orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/upstream_source_orders.py"
                ).read_text(),
            },
            warehouse_setup=lambda path: (
                duckdb.connect(str(path))
                .execute("CREATE TABLE raw_orders AS SELECT CAST(23 AS BIGINT) AS order_id")
                .close()
            ),
        ),
        WorkerExecutionTestCase(
            "warehouse error retains failed node evidence",
            "WITH source(value) AS (VALUES ('invalid')) SELECT CAST(value AS BIGINT) AS amount FROM source",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "failed",
            [(0,)],
        ),
        WorkerExecutionTestCase(
            "scalar function dependency is claimed and executed",
            'SELECT __udf("order_total")(23) AS amount',
            "SELECT amount FROM orders",
            "success",
            [(46,)],
            expected_node_count=2,
            files={
                "functions/sql/order_total.sql": (
                    "FUNCTION (description 'Order total.', "
                    "arguments (amount BIGINT), returns BIGINT);\nSELECT amount * 2\n"
                )
            },
        ),
        WorkerExecutionTestCase(
            "table function dependency is claimed and executed",
            'SELECT amount FROM __table_fn("order_amounts")()',
            "SELECT amount FROM orders",
            "success",
            [(31,)],
            expected_node_count=2,
            files={
                "functions/sql/order_amounts.sql": (
                    "FUNCTION (description 'Order amounts.', returns table (amount BIGINT));\n"
                    "SELECT CAST(31 AS BIGINT) AS amount\n"
                )
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_scheduled_job_when_published_then_an_isolated_worker_reports_real_warehouse_results(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = worker_result(worker, test_case)
        assert run["status"] == test_case.expected_status, worker.log.read_text()


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "a write lock outlasting the busy timeout during a running statement",
            Path(
                "benchmarks/duckshop/edge_cases/orchestration/local_worker_recovery.sql"
            ).read_text(),
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "cancelled",
            [(0,)],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_busy_control_store_when_a_build_runs_then_the_scheduler_retries_and_keeps_its_outcome(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    edge_case: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/busy_control_store.json").read_text()
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        active: dict[str, Any] = running_statement(worker)
        holder: sqlite3.Connection = sqlite3.connect(worker.control_store, isolation_level=None)
        try:
            _ = holder.execute("BEGIN IMMEDIATE")
            time.sleep(edge_case["lock_seconds"])
            assert worker.process.poll() is None, worker.log.read_text()
        finally:
            _ = holder.execute("ROLLBACK")
            holder.close()
        cancel_running_statement(worker, run_id=active["run_id"])
        run: dict[str, Any] = worker_result(worker, test_case)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        assert "the scheduler retries: control store: database is locked" in worker.log.read_text()
        assert worker.process.poll() is None, worker.log.read_text()


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "unchanged attached Python check",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            files={"python/checks/orders.py": "initial"},
        ),
        WorkerExecutionTestCase(
            "attached Python check changed after publication",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "failed",
            [],
            files={"python/checks/orders.py": "initial"},
            files_after_publication={"python/checks/orders.py": "changed"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_attached_python_code_when_a_worker_starts_then_publication_pins_it_before_execution(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture: dict[str, str] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/python_execution_inputs.json"
        ).read_text()
    )
    test_case = replace(
        test_case,
        files={path: fixture[key] for path, key in test_case.files.items()},
        files_after_publication={
            path: fixture[key] for path, key in test_case.files_after_publication.items()
        },
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        runs_url: str = (
            f"{worker.api.url}/api/locations/{worker.api.location_id}/targets/development/runs"
        )
        with urlopen(
            Request(
                f"{runs_url}/{run['run_id']}/nodes",
                headers={"Authorization": f"Bearer {worker.api.bearer}"},
            ),
            timeout=5,
        ) as response:
            nodes: list[dict[str, Any]] = json.load(response)
        assert [(node["node_key"], node["started_at"] is not None) for node in nodes] == [
            ("model:orders", test_case.expected_status == "success")
        ]
        assert (worker.project / "warehouse.duckdb").exists() == (
            test_case.expected_status == "success"
        )


@pytest.mark.parametrize(
    "test_case",
    [
        CheckExecutionTestCase(
            "passed", "SELECT 17 AS amount", "SELECT amount FROM orders", "success", [(17,)]
        ),
        CheckExecutionTestCase(
            "warned",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            expected_check_outcome="warn",
        ),
        CheckExecutionTestCase(
            "failed",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "failed",
            [(17,)],
            expected_check_outcome="failed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_attached_check_when_a_worker_executes_then_its_own_outcome_is_durable(
    test_case: CheckExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture: dict[str, str] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/attached_checks.json").read_text()
    )
    test_case = replace(
        test_case, files={"python/checks/orders.py": fixture[test_case.description]}
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        assert run["selection_json"] == ["model:orders"]
        assert run["node_counts_json"] == {"success": 1}
        with sqlite3.connect(worker.control_store) as control:
            events: list[tuple[str, str]] = control.execute(
                "SELECT event_type,payload FROM recall_events WHERE run_id=? "
                "AND event_type IN ('run_check_started','run_check_finished') "
                "ORDER BY storage_id",
                (run["run_id"],),
            ).fetchall()
        assert [(kind, json.loads(payload)["change"]["change"]) for kind, payload in events] == [
            ("run_check_started", {"kind": "check_started", "node_key": "check:orders_valid"}),
            (
                "run_check_finished",
                {
                    "kind": "check_finished",
                    "node_key": "check:orders_valid",
                    "outcome": test_case.expected_check_outcome,
                },
            ),
        ]
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "actual attached check failure starts a sensor destination",
            "SELECT 17 AS amount",
            "SELECT amount FROM inventory",
            "success",
            [(59,)],
            files={
                "python/checks/orders.py": "failed",
                "orchestration/orders.py": "on_failure",
                "models/inventory.sql": "inventory",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_attached_check_failure_when_a_sensor_observes_it_then_its_destination_executes(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture: dict[str, str] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/attached_checks.json").read_text()
    )
    test_case = replace(
        test_case, files={path: fixture[key] for path, key in test_case.files.items()}
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        source: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert source["status"] == "failed", worker.log.read_text()
        destination: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="refresh_inventory"
        )
        assert destination["status"] == test_case.expected_status, worker.log.read_text()
        assert destination["selection_json"] == ["model:inventory"]
        assert destination["snapshot_id"] == source["snapshot_id"]
        assert source["selection_json"] == ["model:orders"]
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "cancelling active attached-check SQL cannot set failure health",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "cancelled",
            [(17,)],
            files={
                "python/checks/orders.py": "cancelled",
                "orchestration/orders.py": "on_failure",
                "models/inventory.sql": "inventory",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_attached_check_query_when_cancelled_then_it_cannot_trigger_failure_health(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture: dict[str, str] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/attached_checks.json").read_text()
    )
    test_case = replace(
        test_case, files={path: fixture[key] for path, key in test_case.files.items()}
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        cancel_running_statement(worker, job_key="annual_orders")
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert run["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            outcomes: list[tuple[str]] = control.execute(
                "SELECT payload -> 'change' -> 'change' ->> 'outcome' "
                "FROM recall_events WHERE run_id=? AND event_type='run_check_finished'",
                (run["run_id"],),
            ).fetchall()
            assert outcomes == [("cancelled",)]
            completed_query: str = (
                "SELECT count(*) FROM sqb_sensor_evaluation_events WHERE phase='completed'"
            )
            # Two more completions mean at least one evaluation started after the outcome.
            target: int = control.execute(completed_query).fetchone()[0] + 2
            deadline: float = time.monotonic() + 15
            evaluated: int = 0
            while evaluated < target:
                assert worker.process.poll() is None, worker.log.read_text()
                assert time.monotonic() < deadline, worker.log.read_text()
                evaluated = control.execute(completed_query).fetchone()[0]
                time.sleep(0.02)
            assert control.execute("SELECT count(*) FROM sqb_run_requests").fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE job_key='refresh_inventory'"
            ).fetchone() == (0,)
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "group completion starts a real sensor worker",
            "SELECT 59 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(59,)],
            expected_origin="sensor",
            files={
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import RunEventKind, job, job_group, run_event, schedule, sensor\n"
                    "source = job('source_orders', select='orders', description='Source orders.')\n"
                    "source_group = job_group('source_group', description='Source group.', jobs=[source], "
                    "schedule=schedule(cron='0 0 * * *', timezone='UTC', starts_at='2000-01-01T00:00:00Z'))\n"
                    "annual_orders = job('annual_orders', select='orders', description='Refresh orders.', settings={'reload': True})\n"
                    "on_group = sensor('on_group', description='Refresh after group completion.', "
                    "when=run_event(job=source_group, kinds=[RunEventKind.COMPLETED, RunEventKind.SUCCESS], executed_successfully=('orders',)), "
                    "run=annual_orders, minimum_interval_seconds=1)\n"
                )
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_real_group_completion_when_a_run_sensor_matches_then_one_destination_worker_executes(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        destination: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert destination["status"] == test_case.expected_status, worker.log.read_text()
        result: dict[str, Any] = worker_result(worker, test_case)
        assert result["run_id"] == destination["run_id"]
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT run_key,run_id FROM sqb_run_requests").fetchall() == [
                (
                    control.execute(
                        "SELECT run_id FROM sqb_runs WHERE job_key='source_group'"
                    ).fetchone()[0],
                    destination["run_id"],
                )
            ]
            assert control.execute(
                "SELECT job_key,status FROM sqb_runs ORDER BY job_key"
            ).fetchall() == [
                ("annual_orders", "success"),
                ("source_group", "success"),
                ("source_orders", "success"),
            ]


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "group child node sensor",
            "SELECT 67 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(67,)],
            expected_origin="sensor",
            files={
                "models/stg_orders.sql": "MODEL (description 'Staging orders.');\nSELECT 71 AS amount\n",
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import NodeEventKind, job, job_group, node_event, schedule, sensor\n"
                    "source = job('source_orders', select='orders', description='Source orders.')\n"
                    "source_group = job_group('source_group', description='Source group.', jobs=[source], "
                    "schedule=schedule(cron='0 0 * * *', timezone='UTC', starts_at='2000-01-01T00:00:00Z'))\n"
                    "annual_orders = job('annual_orders', select='stg_orders', description='Refresh staging.', settings={'reload': True})\n"
                    "on_node = sensor('on_node', description='React to actual orders rebuilds.', "
                    "when=node_event(select='orders', kinds=[NodeEventKind.SUCCESS]), "
                    "run=annual_orders, minimum_interval_seconds=1)\n"
                ),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_real_group_child_rebuild_when_a_node_sensor_matches_then_only_the_child_finish_queues_a_worker(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        destination: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert destination["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute(
                "SELECT job_key,status FROM sqb_runs ORDER BY job_key"
            ).fetchall() == [
                ("annual_orders", "success"),
                ("source_group", "success"),
                ("source_orders", "success"),
            ]
            assert control.execute(
                "SELECT q.run_key,q.run_id FROM sqb_run_requests q"
            ).fetchall() == [
                (
                    control.execute(
                        "SELECT run_id FROM sqb_runs WHERE job_key='source_orders'"
                    ).fetchone()[0]
                    + "/model:orders",
                    destination["run_id"],
                )
            ]
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "cancellation stops an active warehouse statement",
            "SELECT sum(sin(value)) AS amount FROM range(10000000000) t(value)",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'orders'",
            "cancelled",
            [(0,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_executing_scheduled_worker_when_cancelled_then_the_statement_stops_before_claim_release(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        cancel_running_statement(worker)
        run: dict[str, Any] = worker_result(worker, test_case)
        assert run["status"] == test_case.expected_status, worker.log.read_text()


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "parent cancellation interrupts its active child and cancels its waiting child",
            "SELECT sum(sin(value)) AS amount FROM range(10000000000) t(value)",
            "SELECT count(*) FROM duckdb_tables() WHERE table_name IN ('orders','totals')",
            "cancelled",
            [(0,)],
            files={
                "models/totals.sql": (
                    "MODEL (description 'Downstream totals.', materialized table);\n"
                    'SELECT amount FROM __ref("orders")\n'
                ),
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import job, job_group, schedule\n"
                    "orders = job('long_orders', description='Long query.', select='orders')\n"
                    "totals = job('waiting_totals', description='Waiting query.', select='totals')\n"
                    "annual_orders = job_group('annual_orders', description='Cancellable group.', "
                    "jobs=[totals, orders], schedule=schedule(cron='0 0 * * *', timezone='UTC', "
                    "starts_at='2000-01-01T00:00:00Z'))\n"
                ),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_executing_group_when_parent_cancelled_then_all_children_stop_before_parent_completion(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        parent: dict[str, Any] = wait_for_run(
            worker.api,
            worker.process,
            worker.log,
            pending=frozenset({"queued"}),
            job_key="annual_orders",
        )
        assert parent["status"] == "running", worker.log.read_text()
        cancel_running_statement(worker, job_key="long_orders", run_id=parent["run_id"])
        completed: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute(
                "SELECT job_key, status FROM sqb_runs ORDER BY job_key"
            ).fetchall() == [
                ("annual_orders", "cancelled"),
                ("long_orders", "cancelled"),
                ("waiting_totals", "cancelled"),
            ]
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE parent_run_id=? AND ended_at>?",
                (parent["run_id"], completed["ended_at"]),
            ).fetchone() == (0,)
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone() == (0,)
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "credential failure after a committed claim",
            "SELECT 23 AS amount",
            "",
            "failed",
            [],
            store_setup=(
                "CREATE TRIGGER reject_run_token BEFORE INSERT ON sqb_access_tokens "
                "WHEN NEW.run_id IS NOT NULL "
                "BEGIN SELECT RAISE(ABORT, 'synthetic run credential failure'); END;"
            ),
        ),
        WorkerExecutionTestCase(
            "launch publication failure after a committed claim",
            "SELECT 23 AS amount",
            "",
            "failed",
            [],
            store_setup=(
                "CREATE TRIGGER reject_launch BEFORE UPDATE OF status ON sqb_runs "
                "WHEN NEW.status = 'launched' "
                "BEGIN SELECT RAISE(ABORT, 'synthetic launch publication failure'); END;"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_committed_claim_when_launch_preparation_fails_then_unstarted_work_is_terminal_and_unreserved(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        assert worker.process.wait(timeout=15) != 0, worker.log.read_text()
        request: Request = Request(
            f"{worker.api.url}/api/locations/{worker.api.location_id}/targets/development/runs",
            headers={"Authorization": f"Bearer {worker.api.bearer}"},
        )
        with urlopen(request, timeout=5) as response:
            runs: list[dict[str, Any]] = json.load(response)
        assert len(runs) == 1
        assert runs[0]["status"] == test_case.expected_status, worker.log.read_text()
        assert runs[0]["node_counts_json"] == {"skipped": 1}
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute("SELECT count(*) FROM sqb_concurrency_slots").fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone() == (0,)
        assert not (worker.project / "warehouse.duckdb").exists()
        assert "Worker launched for run" not in worker.log.read_text()


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "failed terminal publication keeps the owner alive until recovery",
            "SELECT 23 AS amount",
            "",
            "failed",
            [],
            store_setup=(
                "CREATE TRIGGER reject_run_token BEFORE INSERT ON sqb_access_tokens "
                "WHEN NEW.run_id IS NOT NULL "
                "BEGIN SELECT RAISE(ABORT, 'synthetic run credential failure'); END;"
                "CREATE TRIGGER reject_terminal BEFORE UPDATE OF status ON sqb_runs "
                "WHEN NEW.status = 'failed' "
                "BEGIN SELECT RAISE(ABORT, 'synthetic terminal publication failure'); END;"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_stopped_worker_when_terminal_publication_fails_then_shutdown_retains_and_retries_its_claim(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        wait_for_log(worker.process, worker.log, "Retrying worker shutdown publication")
        assert worker.process.poll() is None, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (1,)
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone() == (1,)
            _ = control.execute("DROP TRIGGER reject_terminal")
        assert worker.process.wait(timeout=10) != 0, worker.log.read_text()
        request: Request = Request(
            f"{worker.api.url}/api/locations/{worker.api.location_id}/targets/development/runs",
            headers={"Authorization": f"Bearer {worker.api.bearer}"},
        )
        with urlopen(request, timeout=5) as response:
            runs: list[dict[str, Any]] = json.load(response)
        assert len(runs) == 1
        assert runs[0]["status"] == test_case.expected_status
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM recall_events WHERE event_type='run_terminated'"
            ).fetchone() == (1,)


@pytest.mark.skipif(os.name != "posix", reason="POSIX TERM/KILL escalation")
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "uncooperative child is reaped after a lease-preserving hard kill",
            "SELECT 23 AS amount",
            "",
            "failed",
            [],
            launcher="uncooperative",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_uncooperative_worker_when_scheduler_stops_then_the_fence_and_claims_survive_until_reaping(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        pidfile: Path = worker.project / "worker.pid"
        deadline: float = time.monotonic() + 15
        while not pidfile.exists():
            assert worker.process.poll() is None, worker.log.read_text()
            assert time.monotonic() < deadline, worker.log.read_text()
            time.sleep(0.02)
        pid: int = int(pidfile.read_text())
        worker.process.send_signal(signal.SIGTERM)
        time.sleep(32)
        assert worker.process.poll() is None, worker.log.read_text()
        os.kill(pid, 0)
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (1,)
            assert control.execute(
                "SELECT julianday(expires_at) > julianday('now') FROM sqb_scheduler_lease"
            ).fetchone() == (1,)
        assert worker.process.wait(timeout=40) == 0, worker.log.read_text()
        with pytest.raises(ProcessLookupError):
            os.kill(pid, 0)
        request: Request = Request(
            f"{worker.api.url}/api/locations/{worker.api.location_id}/targets/development/runs",
            headers={"Authorization": f"Bearer {worker.api.bearer}"},
        )
        with urlopen(request, timeout=5) as response:
            runs: list[dict[str, Any]] = json.load(response)
        assert len(runs) == 1
        assert runs[0]["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE worker_instance IS NOT NULL"
            ).fetchone() == (0,)
            assert control.execute(
                "SELECT json_extract(payload, '$.change.change.exit_code') FROM recall_events "
                "WHERE event_type='run_terminated'"
            ).fetchone() == (137,)
