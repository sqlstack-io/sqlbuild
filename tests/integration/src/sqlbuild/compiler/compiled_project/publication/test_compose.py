"""Acceptance at the distributed package boundary, not the source-tree runtime mounts."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ComposeDeploymentTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.docker.compose.helpers import (
    ComposeStack,
    platform_receiver,
    wait_for_location_outage,
)


@pytest.mark.skipif(
    os.environ.get("SQLBUILD_REQUIRE_COMPOSE_TESTS") != "1",
    reason="Run make test-compose to build and test the packaged deployment",
)
@pytest.mark.parametrize(
    "test_case",
    [
        ComposeDeploymentTestCase(
            "PostgreSQL control store and immutable DuckDB project images", [[41]]
        )
    ],
    ids=lambda case: case.description,
)
def test_given_packaged_services_when_redeployed_and_restarted_then_pinned_runs_and_platform_events_survive(
    tmp_path: Path,
    test_case: ComposeDeploymentTestCase,
) -> None:
    stack: ComposeStack = ComposeStack(tmp_path)
    with platform_receiver() as (port, events):
        stack.environment["SQLBUILD_PLATFORM_WEBHOOK_URL"] = (
            f"http://host.docker.internal:{port}/events"
        )
        try:
            first_image, second_image, broken_image = stack.build_projects()
            stack.register()
            stack.compose("up", "-d", "--wait", "server")
            stack.compose("up", "-d", "--no-deps", "location")
            first: dict[str, Any] = stack.wait(
                "SELECT * FROM sqb_runs WHERE status='running' AND origin='schedule'"
            )[0]
            assert first["image_id"] == first_image
            worker = f"sqlbuild-{stack.location_id}-{first['run_id']}"
            original: dict[str, Any] = json.loads(
                stack.docker("inspect", "--format", "{{json .State}}", worker).stdout
            )

            # A real replacement removes the old network endpoint; merely restarting is insufficient.
            stack.compose("stop", "--timeout", "1", "location")
            stack.redeploy("second")
            after: dict[str, Any] = json.loads(
                stack.docker("inspect", "--format", "{{json .State}}", worker).stdout
            )
            assert after["Running"] is True
            assert after["Pid"] == original["Pid"]
            assert (
                stack.docker("inspect", "--format", "{{.Image}}", worker).stdout.strip()
                == first_image
            )
            second: dict[str, Any] = stack.wait(
                f"SELECT snapshot_id,image_id FROM sqb_snapshots WHERE image_id='{second_image}'"
            )[0]
            assert second["snapshot_id"] != first["snapshot_id"]
            stack.wait(
                f"SELECT run_id,status FROM sqb_runs WHERE run_id='{first['run_id']}' AND status='success'"
            )

            # These are real Docker/credential/mount boundaries, without exposing their values.
            names: list[str] = json.loads(
                stack.docker("inspect", "--format", "{{json .Config.Env}}", worker).stdout
            )
            names = [entry.split("=", 1)[0] for entry in names]
            assert "SQLBUILD_RUN_TOKEN" in names
            assert not {
                "SQLBUILD_LOCATION_TOKEN",
                "SQLBUILD_CONTROL_DATABASE_URL",
                "SQLBUILD_PLATFORM_WEBHOOK_URL",
            }.intersection(names)
            mounts: list[dict[str, Any]] = json.loads(
                stack.docker("inspect", "--format", "{{json .Mounts}}", worker).stdout
            )
            assert {mount["Destination"] for mount in mounts} == {"/warehouse"}
            assert (
                stack.docker(
                    "inspect", "--format", "{{.HostConfig.NetworkMode}}", worker
                ).stdout.strip()
                == f"{stack.name}_workers"
            )
            assert (
                stack.compose(
                    "exec",
                    "-T",
                    "server",
                    "sh",
                    "-c",
                    "! command -v python && ! command -v python3",
                )
                == ""
            )

            # A location cannot substitute an operator-provided equivalent hash for physical evidence.
            mismatch: subprocess.CompletedProcess[str] = stack.docker(
                "exec",
                f"{stack.name}-location",
                "sqb",
                "location",
                "publish",
                "--server",
                "http://server:8000",
                "--location-id",
                stack.location_id,
                "--target",
                "development",
                "--generation",
                "99",
                "--image-id",
                "sha256:" + "0" * 64,
                check=False,
            )
            assert mismatch.returncode != 0
            assert "409" in mismatch.stderr
            assert (
                stack.query("SELECT MAX(generation) AS generation FROM sqb_snapshots")[0][
                    "generation"
                ]
                == 2
            )

            stack.redeploy("broken")
            failure: dict[str, Any] = stack.wait(
                "SELECT occurred_at FROM recall_events WHERE event_type='location_health' AND payload->'report'->'health'->>'kind'='compile_failed' ORDER BY storage_id DESC"
            )[0]
            current: dict[str, Any] = stack.query(
                "SELECT image_id,snapshot_id FROM sqb_snapshots JOIN sqb_current_snapshots USING(snapshot_id,location_id,target)"
            )[0]
            assert current == second
            assert current["image_id"] != broken_image

            # Queue the next scheduled occurrence from the surviving declaration, not a manual run.
            fallback: dict[str, Any] = stack.wait(
                f"SELECT run_id,status,image_id,snapshot_id FROM sqb_runs WHERE image_id='{second_image}' AND origin='schedule' AND queued_at>'{failure['occurred_at']}' AND status='success'"
            )[0]
            assert fallback["snapshot_id"] == second["snapshot_id"]
            warehouse: str = stack.compose(
                "exec",
                "-T",
                "location",
                "python",
                "-c",
                "import duckdb,json; print(json.dumps(duckdb.connect('/warehouse/orders.duckdb',read_only=True).sql('SELECT order_id FROM orders').fetchall()))",
            )
            assert json.loads(warehouse) == test_case.expected_warehouse_rows

            # Let a real minute boundary pass with no scheduler process, then catch up once.
            stack.compose("stop", "--timeout", "5", "server")
            before: int = stack.query("SELECT count(*) AS count FROM sqb_runs")[0]["count"]
            due: str = stack.query("SELECT next_due_at FROM sqb_trigger_state")[0]["next_due_at"]
            deadline: float = datetime.fromisoformat(due).timestamp() + 2
            while datetime.now(UTC).timestamp() < deadline:
                time.sleep(0.5)
            assert stack.query("SELECT count(*) AS count FROM sqb_runs")[0]["count"] == before
            stack.compose("up", "-d", "--wait", "server")
            stack.wait(
                f"SELECT r.run_id,r.status,t.scheduled_for FROM sqb_runs r JOIN sqb_ticks t USING(tick_id) WHERE t.scheduled_for='{due}' AND r.status='success'"
            )
            assert (
                stack.query(
                    "SELECT trigger_id,scheduled_for,count(*) FROM sqb_ticks GROUP BY trigger_id,scheduled_for HAVING count(*)>1"
                )
                == []
            )

            prior_events: set[str] = {event["event_id"] for event in events}
            stack.compose("stop", "--timeout", "1", "location")
            stopped_at: datetime = datetime.now(UTC)
            broken: dict[str, Any] = wait_for_location_outage(events, prior_events)
            assert broken["payload"]["platform"]["location_id"] == stack.location_id
            assert broken["payload"]["platform"]["target"] == "development"
            assert (
                datetime.fromisoformat(broken["payload"]["platform"]["last_seen_at"]) <= stopped_at
            )

            # Even a valid location bearer cannot publish without physical image evidence.
            stack.compose("rm", "--force", "location")
            unavailable: subprocess.CompletedProcess[str] = stack.docker(
                "run",
                "--rm",
                "--network",
                f"{stack.name}_workers",
                "--env",
                "SQLBUILD_LOCATION_TOKEN",
                f"{stack.name}:second",
                "location",
                "publish",
                "--server",
                "http://server:8000",
                "--location-id",
                stack.location_id,
                "--target",
                "development",
                "--generation",
                "99",
                check=False,
            )
            assert unavailable.returncode != 0
            assert "503" in unavailable.stderr
            assert (
                stack.query(
                    "SELECT image_id,snapshot_id FROM sqb_snapshots JOIN sqb_current_snapshots USING(snapshot_id,location_id,target)"
                )[0]
                == second
            )

            stack.verify_control_store_restore(second)
        finally:
            (tmp_path / "platform-events.json").write_text(json.dumps(events, indent=2))
            stack.close()


@pytest.mark.skipif(
    os.environ.get("SQLBUILD_REQUIRE_COMPOSE_TESTS") != "1",
    reason="Run make test-compose to build and test the packaged deployment",
)
@pytest.mark.parametrize(
    "test_case",
    [
        ComposeDeploymentTestCase(
            "Host-owned development source with isolated runtime writes", [[41]]
        )
    ],
    ids=lambda case: case.description,
)
def test_given_host_owned_source_when_using_the_development_overlay_then_reloaded_workers_can_write_runtime_outputs(
    tmp_path: Path,
    test_case: ComposeDeploymentTestCase,
) -> None:
    stack: ComposeStack = ComposeStack(tmp_path)
    stack.environment["SQLBUILD_LOCATION_IMAGE"] = os.environ.get(
        "SQLBUILD_TEST_LOCATION_IMAGE", "sqlbuild-location:test"
    )
    project: Path = tmp_path / "source"
    shutil.copytree(Path("deploy/project"), project)
    assert project.stat().st_uid != 10001
    override: dict[str, Any] = json.loads(stack.override.read_text())
    override["services"]["location"] = {
        "volumes": [{"type": "bind", "source": str(project), "target": "/project"}]
    }
    stack.override.write_text(json.dumps(override))
    stack.compose_files.insert(1, Path("deploy/compose.dev.yml").resolve())
    try:
        stack.register()
        stack.compose("up", "-d", "--wait", "server")
        stack.compose("up", "-d", "--no-deps", "location")
        first: dict[str, Any] = stack.wait(
            "SELECT snapshot_id FROM sqb_snapshots WHERE generation=1", timeout=60
        )[0]
        cases: dict[str, str] = json.loads(
            Path(
                "benchmarks/duckshop/edge_cases/orchestration/container_deployment.json"
            ).read_text()
        )
        (project / "models/orders.sql").write_text(
            "MODEL (description 'Example orders.', materialized table);\n" + cases["second"] + "\n"
        )
        second: dict[str, Any] = stack.wait(
            "SELECT snapshot_id FROM sqb_snapshots WHERE generation=2", timeout=45
        )[0]
        assert second != first
        stack.wait(
            f"SELECT run_id FROM sqb_runs WHERE snapshot_id='{second['snapshot_id']}' "
            "AND origin='schedule' AND status='success'",
            timeout=100,
        )
        warehouse: str = stack.compose(
            "exec",
            "-T",
            "location",
            "python",
            "-c",
            "import duckdb,json; print(json.dumps(duckdb.connect('/warehouse/orders.duckdb',read_only=True).sql('SELECT order_id FROM orders').fetchall()))",
        )
        assert json.loads(warehouse) == test_case.expected_warehouse_rows
        (project / "models/orders.sql").write_text(cases["broken"] + "\n")
        stack.wait(
            "SELECT event_id FROM recall_events WHERE event_type='location_health' "
            "AND payload->'report'->'health'->>'kind'='compile_failed'",
            timeout=45,
        )
        assert stack.query(
            "SELECT snapshot_id FROM sqb_snapshots "
            "JOIN sqb_current_snapshots USING(snapshot_id,location_id,target)"
        ) == [second]
    finally:
        stack.close()
