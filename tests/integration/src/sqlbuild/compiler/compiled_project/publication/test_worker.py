"""Retained compiler baselines cross the native worker boundary before lifecycle writes."""

import json
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen
from uuid import uuid4

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
    SnapshotWorkerTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    claimed_worker,
    snapshot_api,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


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
