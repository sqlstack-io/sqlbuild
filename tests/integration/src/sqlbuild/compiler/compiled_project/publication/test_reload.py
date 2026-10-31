"""A native supervisor reloads real project interpreters through the authenticated HTTP API."""

import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import TYPE_PROOF_FILES
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    LocationReloadTestCase,
    LocationShutdownTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    location_process,
    snapshot_api,
    wait_for_generation,
    wait_for_interpreter,
    wait_for_log,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        LocationReloadTestCase("ordinary target", "development", 5, int(sys.platform == "win32")),
        LocationReloadTestCase("encoded target", "preview #1", 5, int(sys.platform == "win32")),
    ],
    ids=lambda case: case.description,
)
def test_given_a_served_location_when_inputs_change_or_restart_then_generations_reload_without_losing_last_good(
    test_case: LocationReloadTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    declaration: str = (
        "from sqlbuild.orchestration import job\n"
        'daily_orders = job("daily_orders", select="orders", description="Build orders.")\n'
    )
    files: dict[str, str] = TYPE_PROOF_FILES | {
        "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
        + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
        + f'\n[targets.{json.dumps(test_case.target)}]\nschema = "main"\n',
        "orchestration/orders.py": declaration,
    }
    write_files(project_dir=project, files=files)
    _ = subprocess.run(["git", "init", "-q", str(project)], check=True, capture_output=True)
    monkeypatch.setenv("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://must-not-open")
    with snapshot_api(tmp_path / "control") as api:
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        # A presentation-only width fallback must not bypass the native-only supervisor.
        monkeypatch.setenv("COLUMNS", "999999999999999999999999999999")
        arguments: list[str] = [
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
            test_case.target,
        ]
        log: Path = project / "location.log"
        publications: Path = project / "publications.jsonl"
        with location_process(arguments, log, publications) as process:
            first: dict[str, Any] = wait_for_generation(api, test_case.target, 1, process, log)
            staging: Path = project / "models/stg_orders.sql"
            for value in (11, 12, 13):
                _ = staging.write_text(
                    files["models/stg_orders.sql"].replace(
                        "CAST(5 AS INTEGER)", f"CAST({value} AS INTEGER)"
                    )
                )
            second: dict[str, Any] = wait_for_generation(api, test_case.target, 2, process, log)
            assert second["record"]["snapshot_id"] != first["record"]["snapshot_id"]
            assert "CAST(13 AS INTEGER)" in second["nodes"]["model:stg_orders"]["raw_sql"]
            assert {change["subject_key"] for change in second["changes"]} == {"model:stg_orders"}

            _ = (project / "orchestration/orders.py").write_text("invalid Python declaration !\n")
            wait_for_log(process, log, "Location reload failed; retaining last good snapshot.")
            assert current_snapshot(api, test_case.target)["record"] == second["record"]
            _ = (project / "orchestration/orders.py").write_text(
                declaration.replace("Build orders.", "Refresh order totals.")
            )
            third: dict[str, Any] = wait_for_generation(api, test_case.target, 3, process, log)
            assert (
                third["documents"]["jobs"]["daily_orders"]["description"] == "Refresh order totals."
            )
            _ = subprocess.run(
                [
                    "git",
                    "-C",
                    str(project),
                    "-c",
                    "user.name=Orders Contributor",
                    "-c",
                    "user.email=orders@example.com",
                    "commit",
                    "--allow-empty",
                    "-qm",
                    "Record orders",
                ],
                check=True,
                capture_output=True,
            )
            committed: dict[str, Any] = wait_for_generation(api, test_case.target, 4, process, log)
            assert committed["nodes"] == third["nodes"]
            process.terminate()
            assert process.wait(timeout=5) == test_case.expected_stop_code
        with location_process(arguments, log, publications) as process:
            restarted: dict[str, Any] = wait_for_generation(
                api, test_case.target, test_case.expected_generation, process, log
            )
            assert restarted["nodes"] == third["nodes"]
            # Redirected output and compiler cache writes must not feed the input watcher.
            time.sleep(1)
            assert current_snapshot(api, test_case.target)["record"] == restarted["record"]
            process.terminate()
            assert process.wait(timeout=5) == test_case.expected_stop_code
        records: list[dict[str, Any]] = [
            json.loads(line) for line in publications.read_text().splitlines()
        ]
        assert [record["generation"] for record in records] == [1, 2, 3, 4, 5]
        assert api.bearer not in log.read_text() + publications.read_text()
        assert not (project / ".sqlbuild/control.sqlite3").exists()
        assert not (project / "warehouse.duckdb").exists()


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        LocationShutdownTestCase("terminate compiling interpreter", signal.SIGTERM, 0),
        LocationShutdownTestCase("interrupt compiling interpreter", signal.SIGINT, 0),
    ],
    ids=lambda case: case.description,
)
def test_given_a_compiling_location_when_stopped_then_its_interpreter_is_reaped(
    test_case: LocationShutdownTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    child_pid: Path = tmp_path / "interpreter.pid"
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '\n[targets.development]\nschema = "main"\n',
            "orchestration/slow.py": (
                "import os, time\nfrom pathlib import Path\n"
                f"Path({str(child_pid)!r}).write_text(str(os.getpid()))\n"
                "time.sleep(60)\n"
            ),
        },
    )
    with snapshot_api(tmp_path / "control") as api:
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        log: Path = tmp_path / "location.log"
        arguments: list[str] = [
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
        ]
        with location_process(arguments, log, tmp_path / "publications.jsonl") as process:
            interpreter: int = wait_for_interpreter(process, child_pid, log)
            process.send_signal(test_case.signal)
            assert process.wait(timeout=5) == test_case.expected_exit_code
            with pytest.raises(ProcessLookupError):
                os.kill(interpreter, 0)
