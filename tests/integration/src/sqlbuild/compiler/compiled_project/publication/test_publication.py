"""Real compile, native transport, authenticated server and durable publication."""

import json
import socket
import sqlite3
import subprocess
from pathlib import Path
from typing import Any
from urllib.parse import quote
from urllib.request import Request, urlopen

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import (
    NativeOrchestrationSnapshot,
    build_compiled_project,
    build_orchestration_snapshot,
    discover_project_inputs,
    run_cli,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    METADATA,
    TYPE_PROOF_FILES,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SnapshotPublicationTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    RejectedPublicationOutput,
    current_snapshot,
    snapshot_api,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotPublicationTestCase(
            description="ordinary target",
            target="development",
            expected_generation=2,
            expected_nodes=frozenset({"model:orders", "model:stg_orders"}),
        ),
        SnapshotPublicationTestCase(
            description="target requiring URL path encoding",
            target="preview #1",
            expected_generation=2,
            expected_nodes=frozenset({"model:orders", "model:stg_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_retained_compiler_data_when_publishing_then_http_updates_atomically_and_rejections_keep_last_good(
    test_case: SnapshotPublicationTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "project"
    files: dict[str, str] = TYPE_PROOF_FILES | {
        "orchestration/edge_cases/definitions.py": Path(
            "benchmarks/duckshop/project/orchestration/edge_cases/definitions.py"
        ).read_text(),
        "models/orders.sql": TYPE_PROOF_FILES["models/orders.sql"].replace(
            "MODEL (", "MODEL (tags ['edge_case'],", 1
        ),
    }
    write_files(project_dir=project_dir, files=files)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    assert not any(item.severity == "error" for item in project.diagnostics)
    with snapshot_api(tmp_path / "control") as api:
        metadata: dict[str, Any] = METADATA | {
            "location_id": api.location_id,
            "target": test_case.target,
        }
        first: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            project, json.dumps(metadata), project_dir
        )
        record: dict[str, Any] = json.loads(first.publish(api.url, api.bearer))
        assert record["generation"] == 1
        assert record["location_id"] == api.location_id
        initial: dict[str, Any] = current_snapshot(api, test_case.target)
        assert frozenset(initial["nodes"]) == test_case.expected_nodes
        assert initial["nodes"]["model:orders"]["definition"]["deps"] == ["model:stg_orders"]
        assert initial["documents"]["triggers"]["edge_snapshot_orders"]["catch_up"] == {
            "all": {"max_runs": 3}
        }
        edited: dict[str, str] = files | {
            "models/stg_orders.sql": TYPE_PROOF_FILES["models/stg_orders.sql"].replace(
                "CAST(5 AS INTEGER)", "CAST(9 AS INTEGER)"
            )
        }
        write_files(project_dir=project_dir, files=edited)
        changed: CompiledProjectView = build_compiled_project(
            discovered_inputs=discover_project_inputs(project_dir=project_dir),
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        next_snapshot: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            changed, json.dumps(metadata | {"generation": 2}), project_dir
        )
        updated: dict[str, Any] = json.loads(next_snapshot.publish(api.url, api.bearer))
        assert updated["generation"] == test_case.expected_generation
        assert updated["snapshot_id"] != record["snapshot_id"]
        with pytest.raises(RuntimeError, match="HTTP 409"):
            _ = first.publish(api.url, api.bearer)
        with pytest.raises(RuntimeError, match="HTTP 401"):
            _ = next_snapshot.publish(api.url, "invalid-credential")
        current: dict[str, Any] = current_snapshot(api, test_case.target)
        assert current["record"] == updated
        assert "CAST(9 AS INTEGER)" in current["nodes"]["model:stg_orders"]["raw_sql"]
        request: Request = Request(
            f"{api.url}/api/locations/{api.location_id}/targets/"
            f"{quote(test_case.target, safe='')}/triggers",
            headers={"Authorization": f"Bearer {api.bearer}"},
        )
        with urlopen(request, timeout=5) as response:
            triggers: list[dict[str, Any]] = json.load(response)
        assert len(triggers) == 1
        assert triggers[0]["status"] == "paused"
        assert triggers[0]["definition_json"]["starts_at"] == "2026-07-02T19:47:23.456789Z"
        assert triggers[0]["definition_json"]["default_status"] == "paused"
        assert triggers[0]["definition_json"]["catch_up"] == {"all": {"max_runs": 3}}


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotPublicationTestCase(
            description="configured development target",
            target="development",
            expected_generation=2,
            expected_nodes=frozenset({"model:orders", "model:stg_orders"}),
        ),
        SnapshotPublicationTestCase(
            description="configured target with encoded path characters",
            target="preview #1",
            expected_generation=2,
            expected_nodes=frozenset({"model:orders", "model:stg_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_code_location_when_publishing_from_cli_then_compilation_isolated_from_store_and_failed_edits_keep_last_good(
    test_case: SnapshotPublicationTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / "project"
    files: dict[str, str] = TYPE_PROOF_FILES | {
        "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
        + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
        + f'\n[targets.{json.dumps(test_case.target)}]\nschema = "main"\n',
        "orchestration/orders.py": (
            "from sqlbuild.orchestration import job\n"
            'daily_orders = job("daily_orders", select="orders", description="Build orders.")\n'
        ),
    }
    write_files(project_dir=project_dir, files=files)
    _ = subprocess.run(["git", "init", "-q", str(project_dir)], check=True, capture_output=True)
    _ = subprocess.run(
        [
            "git",
            "-C",
            str(project_dir),
            "-c",
            "user.name=Orders Contributor",
            "-c",
            "user.email=orders@example.com",
            "commit",
            "--allow-empty",
            "-qm",
            "Initial orders",
        ],
        check=True,
        capture_output=True,
    )
    monkeypatch.setenv("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://must-not-open")
    with snapshot_api(tmp_path / "control") as api:
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        arguments: list[str] = [
            "--no-color",
            "--project-dir",
            str(project_dir),
            "location",
            "publish",
            "--server",
            api.url,
            "--location-id",
            api.location_id,
            "--target",
            test_case.target,
            "--image-id",
            "sha256:synthetic-orders-image",
        ]
        assert run_cli([*arguments, "--generation", "1"]) == 0
        first_output: CaptureResult[str] = capsys.readouterr()
        first: dict[str, Any] = json.loads(first_output.out)
        assert first["generation"] == 1
        assert "Location snapshot published." in first_output.err
        initial: dict[str, Any] = current_snapshot(api, test_case.target)
        assert frozenset(initial["nodes"]) == test_case.expected_nodes
        assert initial["documents"]["jobs"]["daily_orders"]["selection"] == ["model:orders"]
        assert not (project_dir / ".sqlbuild/control.sqlite3").exists()
        assert not (project_dir / "warehouse.duckdb").exists()
        with sqlite3.connect(tmp_path / "control/.sqlbuild/control.sqlite3") as connection:
            metadata: tuple[str, int, str, str] | None = connection.execute(
                "SELECT git_revision, git_dirty, git_author, image_id FROM sqb_snapshots WHERE snapshot_id=?",
                (first["snapshot_id"],),
            ).fetchone()
        assert metadata is not None
        assert len(metadata[0]) == 40
        assert metadata[1:] == (1, "Orders Contributor", "sha256:synthetic-orders-image")

        staging: Path = project_dir / "models/stg_orders.sql"
        _ = staging.write_text(
            files["models/stg_orders.sql"].replace("CAST(5 AS INTEGER)", "CAST(11 AS INTEGER)")
        )
        assert run_cli([*arguments, "--generation", "2"]) == 0
        second_output: CaptureResult[str] = capsys.readouterr()
        second: dict[str, Any] = json.loads(second_output.out)
        assert second["generation"] == test_case.expected_generation
        updated: dict[str, Any] = current_snapshot(api, test_case.target)
        assert {change["subject_key"] for change in updated["changes"]} == {"model:stg_orders"}
        assert "CAST(11 AS INTEGER)" in updated["nodes"]["model:stg_orders"]["raw_sql"]

        _ = staging.write_text("SELECT FROM\n")
        assert run_cli([*arguments, "--generation", "3"]) != 0
        failed_output: CaptureResult[str] = capsys.readouterr()
        assert not failed_output.out
        assert "Location command failed." in failed_output.err
        assert current_snapshot(api, test_case.target)["record"] == second

        _ = staging.write_text(files["models/stg_orders.sql"])
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", "invalid-credential")
        assert run_cli([*arguments, "--generation", "3"]) != 0
        rejected_output: CaptureResult[str] = capsys.readouterr()
        assert not rejected_output.out
        assert "HTTP 401" in rejected_output.err
        assert api.bearer not in "".join(
            (
                first_output.out,
                first_output.err,
                second_output.out,
                second_output.err,
                failed_output.err,
                rejected_output.err,
            )
        )
        assert current_snapshot(api, test_case.target)["record"] == second

        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        with socket.socket() as unavailable:
            unavailable.bind(("127.0.0.1", 0))
            disconnected: list[str] = list(arguments)
            disconnected[arguments.index("--server") + 1] = (
                f"http://127.0.0.1:{unavailable.getsockname()[1]}"
            )
            assert run_cli([*disconnected, "--generation", "3"]) != 0
        transport_output: CaptureResult[str] = capsys.readouterr()
        assert not transport_output.out
        assert "generation 3 may already be published" in transport_output.err
        assert current_snapshot(api, test_case.target)["record"] == second

        with monkeypatch.context() as result_scope:
            result_scope.setattr("sys.stdout", RejectedPublicationOutput())
            assert run_cli([*arguments, "--generation", "3"]) != 0
        delivery_output: CaptureResult[str] = capsys.readouterr()
        assert "was published, but result delivery failed" in delivery_output.err
        assert current_snapshot(api, test_case.target)["record"]["generation"] == 3
