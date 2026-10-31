"""Retained compiler baselines cross the native worker boundary before lifecycle writes."""

import json
import os
import signal
import sqlite3
import time
from dataclasses import replace
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen
from uuid import uuid4

import duckdb
import pytest

from sqlbuild._native import (
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
    ClaimedWorker,
    GroupExecutionTestCase,
    SnapshotWorkerTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    cancel_running_statement,
    claimed_worker,
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
            "SELECT CAST(value AS INTEGER) AS amount FROM (VALUES ('invalid')) AS source(value)",
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
            "whole downstream job is blocked without building an empty selection",
            "SELECT CAST(value AS INTEGER) AS amount FROM (VALUES ('invalid')) AS source(value)",
            "SELECT count(*) FROM information_schema.tables WHERE table_name IN ('orders','totals')",
            "partial",
            [(0,)],
            on_failure="skip_downstream_jobs",
            expected_child_statuses={"staging": ("partial",), "marts": ("failed",)},
        ),
        GroupExecutionTestCase(
            "job failure stops the group",
            "SELECT CAST(value AS INTEGER) AS amount FROM (VALUES ('invalid')) AS source(value)",
            "SELECT count(*) FROM information_schema.tables WHERE table_name IN ('orders','totals')",
            "failed",
            [(0,)],
            on_failure="stop_group",
            expected_child_statuses={"staging": ("partial", "cancelled"), "marts": ("cancelled",)},
            expected_totals=("cancelled", None),
            expected_marts_started=False,
        ),
        GroupExecutionTestCase(
            "critical node stops the group under the default policy",
            "SELECT CAST(value AS INTEGER) AS amount FROM (VALUES ('invalid')) AS source(value)",
            "SELECT count(*) FROM information_schema.tables WHERE table_name IN ('orders','totals')",
            "failed",
            [(0,)],
            expected_child_statuses={"staging": ("partial", "cancelled"), "marts": ("cancelled",)},
            expected_totals=("cancelled", None),
            expected_marts_started=False,
            files={
                "models/broken_orders.sql": (
                    "MODEL (description 'Critical orders.', materialized table, critical true);\n"
                    "SELECT CAST(value AS INTEGER) AS amount FROM (VALUES ('invalid')) AS source(value)\n"
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
                f'select="stg_orders broken_orders", on_failure="{test_case.on_failure}")\n'
                'marts = job("marts", description="Build independent marts.", select="orders totals")\n'
                'annual_orders = job_group("annual_orders", description="Stage then build marts.", '
                "jobs=[marts, staging], "
                'schedule=schedule(cron="0 0 1 1 *", timezone="UTC", '
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
        assert runs["staging"]["ended_at"] <= (
            runs["marts"]["started_at"] or runs["staging"]["ended_at"]
        )
        with sqlite3.connect(worker.control_store) as control:
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
            assert control.execute("SELECT count(*) FROM sqb_run_liveness").fetchone() == (0,)
        with duckdb.connect(str(project_dir / "warehouse.duckdb"), read_only=True) as warehouse:
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
                    "schedule=schedule(cron='0 0 1 1 *', timezone='UTC', starts_at='2000-01-01T00:00:00Z'))\n"
                    "annual_inventory = job_group('annual_inventory', description='Inventory group.', "
                    "jobs=[inventory], concurrency_groups=('inventory_limit',), "
                    "schedule=schedule(cron='0 0 1 1 *', timezone='UTC', starts_at='2000-01-01T00:00:00Z'))\n"
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
def test_given_opposing_group_limits_when_scheduled_then_both_real_workers_finish_without_hoisting_limits(
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
                ("annual_orders", "orders_limit", 1),
                ("inventory_job", "orders_limit", 1),
                ("orders_job", "inventory_limit", 1),
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
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    with snapshot_api(tmp_path / "control") as api:
        metadata: dict[str, Any] = METADATA | {"location_id": api.location_id}
        baseline: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            project, json.dumps(metadata), project_dir
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
        drifted: CompiledProjectView = build_compiled_project(
            discovered_inputs=discover_project_inputs(project_dir=project_dir),
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        wrong: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            drifted, json.dumps(metadata), project_dir
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
            "SELECT CAST(value AS BIGINT) AS amount FROM (VALUES ('23')) t(value)",
            "SELECT amount FROM orders",
            "success",
            [(23,)],
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
                    "schedule=schedule(cron='0 0 1 1 *', timezone='UTC', "
                    "starts_at='2000-01-01T00:00:00Z'))\n"
                ),
            },
        ),
        WorkerExecutionTestCase(
            "warehouse error retains failed node evidence",
            "SELECT CAST(value AS BIGINT) AS amount FROM (VALUES ('invalid')) t(value)",
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
                    "orders = job('orders', description='Long query.', select='orders')\n"
                    "totals = job('totals', description='Waiting query.', select='totals')\n"
                    "annual_orders = job_group('annual_orders', description='Cancellable group.', "
                    "jobs=[totals, orders], schedule=schedule(cron='0 0 1 1 *', timezone='UTC', "
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
        cancel_running_statement(worker, job_key="orders", run_id=parent["run_id"])
        completed: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        with sqlite3.connect(worker.control_store) as control:
            assert control.execute(
                "SELECT job_key, status FROM sqb_runs ORDER BY job_key"
            ).fetchall() == [
                ("annual_orders", "cancelled"),
                ("orders", "cancelled"),
                ("totals", "cancelled"),
            ]
            assert control.execute(
                "SELECT count(*) FROM sqb_runs WHERE parent_run_id=? AND ended_at>?",
                (parent["run_id"], completed["ended_at"]),
            ).fetchone() == (0,)
            assert control.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
            assert control.execute("SELECT count(*) FROM sqb_run_liveness").fetchone() == (0,)
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
            assert control.execute("SELECT count(*) FROM sqb_run_liveness").fetchone() == (0,)
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
            assert control.execute("SELECT count(*) FROM sqb_run_liveness").fetchone() == (1,)
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
            assert control.execute("SELECT count(*) FROM sqb_run_liveness").fetchone() == (0,)
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
            assert control.execute("SELECT count(*) FROM sqb_run_liveness").fetchone() == (0,)
            assert control.execute(
                "SELECT json_extract(payload, '$.change.change.exit_code') FROM recall_events "
                "WHERE event_type='run_terminated'"
            ).fetchone() == (137,)
