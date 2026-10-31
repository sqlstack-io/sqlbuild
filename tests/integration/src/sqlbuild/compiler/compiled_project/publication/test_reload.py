"""A native supervisor reloads real project interpreters through the authenticated HTTP API."""

import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import TYPE_PROOF_FILES
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    LocationDatabaseTestCase,
    LocationReloadTestCase,
    LocationShutdownTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    location_process,
    snapshot_api,
    wait_for_generation,
    wait_for_interpreter,
    wait_for_location_health,
    wait_for_log,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        LocationReloadTestCase(
            "healthy connection",
            "development",
            7,
            int(sys.platform == "win32"),
            "warehouse.duckdb",
            True,
        ),
        LocationReloadTestCase(
            "failed connection with encoded target",
            "preview #1",
            7,
            int(sys.platform == "win32"),
            "missing/warehouse.duckdb",
            False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_served_location_when_inputs_change_or_restart_then_generations_reload_without_losing_last_good(
    test_case: LocationReloadTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    compiler_pid: Path = tmp_path / "compiler.pid"
    declaration: str = (
        "from sqlbuild.orchestration import job\n"
        "import os\nfrom pathlib import Path\n"
        f"Path({str(compiler_pid)!r}).write_text(str(os.getpid()))\n"
        'print("Orders declaration loaded.")\n'
        'daily_orders = job("daily_orders", select="orders", description="Build orders.")\n'
    )
    files: dict[str, str] = TYPE_PROOF_FILES | {
        "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
        + f"\n[connections.local]\ndatabase = {json.dumps(test_case.database)}\n"
        + f'\n[targets.{json.dumps(test_case.target)}]\nschema = "main"\n',
        "orchestration/orders.py": declaration,
        "sinks/orders.py": 'print("Loading orders sink", flush=True)\n',
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
            first_interpreter: int = int(compiler_pid.read_text())
            heartbeat: dict[str, Any] = wait_for_location_health(
                api, test_case.target, {"kind": "heartbeat"}, process, log
            )
            initial_health: dict[str, Any] = wait_for_location_health(
                api,
                test_case.target,
                {"kind": "connection", "generation": 1, "healthy": test_case.expected_connection},
                process,
                log,
            )
            assert initial_health["report"]["session_id"] == heartbeat["report"]["session_id"]
            staging: Path = project / "models/stg_orders.sql"
            for value in (11, 12, 13):
                _ = staging.write_text(
                    files["models/stg_orders.sql"].replace(
                        "CAST(5 AS INTEGER)", f"CAST({value} AS INTEGER)"
                    )
                )
            second: dict[str, Any] = wait_for_generation(api, test_case.target, 2, process, log)
            assert second["record"]["snapshot_id"] != first["record"]["snapshot_id"]
            assert int(compiler_pid.read_text()) == first_interpreter
            assert "CAST(13 AS INTEGER)" in second["nodes"]["model:stg_orders"]["raw_sql"]
            assert {change["subject_key"] for change in second["changes"]} == {"model:stg_orders"}

            _ = staging.write_text("SELECT FROM invalid SQL !\n")
            _ = wait_for_location_health(
                api, test_case.target, {"kind": "compile_failed", "generation": 3}, process, log
            )
            assert current_snapshot(api, test_case.target)["record"] == second["record"]
            _ = staging.write_text(
                files["models/stg_orders.sql"].replace("CAST(5 AS INTEGER)", "CAST(14 AS INTEGER)")
            )
            recovered: dict[str, Any] = wait_for_generation(api, test_case.target, 3, process, log)
            assert int(compiler_pid.read_text()) == first_interpreter
            assert "CAST(14 AS INTEGER)" in recovered["nodes"]["model:stg_orders"]["raw_sql"]

            _ = (project / "orchestration/orders.py").write_text("invalid Python declaration !\n")
            wait_for_log(process, log, "Location reload failed; retaining last good snapshot.")
            _ = wait_for_location_health(
                api, test_case.target, {"kind": "compile_failed", "generation": 4}, process, log
            )
            assert current_snapshot(api, test_case.target)["record"] == recovered["record"]
            _ = (project / "orchestration/orders.py").write_text(
                declaration.replace("Build orders.", "Refresh order totals.")
            )
            third: dict[str, Any] = wait_for_generation(api, test_case.target, 4, process, log)
            replacement_interpreter: int = int(compiler_pid.read_text())
            assert replacement_interpreter != first_interpreter
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
            committed: dict[str, Any] = wait_for_generation(api, test_case.target, 5, process, log)
            assert int(compiler_pid.read_text()) == replacement_interpreter
            assert committed["nodes"] == third["nodes"]
            _ = (project / "sqlbuild_project.toml").write_text(
                files["sqlbuild_project.toml"].replace('schema = "main"', 'schema = "reporting"')
            )
            configured: dict[str, Any] = wait_for_generation(api, test_case.target, 6, process, log)
            assert int(compiler_pid.read_text()) == replacement_interpreter
            assert (
                configured["nodes"]["model:orders"]["definition"]["destination"]["schema"]
                == "reporting"
            )
            assert (
                configured["nodes"]["model:orders"]["raw_sql"]
                == committed["nodes"]["model:orders"]["raw_sql"]
            )
            process.terminate()
            assert process.wait(timeout=5) == test_case.expected_stop_code
        with location_process(arguments, log, publications) as process:
            restarted: dict[str, Any] = wait_for_generation(
                api, test_case.target, test_case.expected_generation, process, log
            )
            assert int(compiler_pid.read_text()) != replacement_interpreter
            assert restarted["nodes"] == configured["nodes"]
            restarted_health: dict[str, Any] = wait_for_location_health(
                api,
                test_case.target,
                {"kind": "connection", "generation": 7, "healthy": test_case.expected_connection},
                process,
                log,
            )
            assert (
                restarted_health["report"]["session_id"] != initial_health["report"]["session_id"]
            )
            assert restarted_health["received_at"] >= initial_health["received_at"]
            # Redirected output and compiler cache writes must not feed the input watcher.
            time.sleep(1)
            assert current_snapshot(api, test_case.target)["record"] == restarted["record"]
            process.terminate()
            assert process.wait(timeout=5) == test_case.expected_stop_code
        records: list[dict[str, Any]] = [
            json.loads(line) for line in publications.read_text().splitlines()
        ]
        assert [record["generation"] for record in records] == [1, 2, 3, 4, 5, 6, 7]
        assert api.bearer not in log.read_text() + publications.read_text()
        assert not (project / ".sqlbuild/control.sqlite3").exists()
        assert (project / "warehouse.duckdb").exists() == test_case.expected_connection


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        LocationShutdownTestCase(
            "terminate compiling interpreter after repeated heartbeats",
            signal.SIGTERM,
            0,
            2,
            "runtime",
        ),
        LocationShutdownTestCase("interrupt compiling interpreter", signal.SIGINT, 0, 1, "runtime"),
        LocationShutdownTestCase(
            "terminate connection checking interpreter", signal.SIGTERM, 0, 1, "debug"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_busy_location_when_stopped_then_its_interpreter_is_reaped(
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
            "adapters/slow.py": (
                "import os, sys, time\nfrom pathlib import Path\n"
                f"if {test_case.blocked_command!r} in sys.argv:\n"
                f"    Path({str(child_pid)!r}).write_text(str(os.getpid()))\n"
                "    time.sleep(60)\n"
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
            _ = wait_for_location_health(
                api,
                "development",
                {"kind": "heartbeat"},
                process,
                log,
                minimum_reports=test_case.minimum_heartbeats,
            )
            process.send_signal(test_case.signal)
            assert process.wait(timeout=5) == test_case.expected_exit_code
            with pytest.raises(ProcessLookupError):
                os.kill(interpreter, 0)


@pytest.mark.parametrize(
    "test_case",
    [
        LocationDatabaseTestCase(
            "custom database extension", "warehouse.db", 1, 'database = "warehouse.db"'
        ),
        LocationDatabaseTestCase(
            "extensionless database", "warehouse", 1, 'database = "warehouse"'
        ),
        LocationDatabaseTestCase(
            "custom attached database",
            "inventory.db",
            1,
            'database = ":memory:"\nattach = [{path = "inventory.db", alias = "inventory"}]',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_location_using_a_local_database_when_it_is_created_and_written_then_only_source_edits_reload(
    test_case: LocationDatabaseTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + f"\n[connections.local]\n{test_case.connection_toml}\n"
            + '\n[targets.development]\nschema = "main"\n',
        },
    )
    database: Path = project / test_case.database
    assert not database.exists()
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
            baseline: dict[str, Any] = wait_for_generation(
                api, "development", test_case.expected_generation, process, log
            )
            _ = wait_for_location_health(
                api,
                "development",
                {"kind": "connection", "generation": 1, "healthy": True},
                process,
                log,
            )
            assert database.is_file()
            with duckdb.connect(str(database)) as connection:
                _ = connection.execute(
                    "CREATE TABLE orders (order_id INTEGER); INSERT INTO orders VALUES (17), (23)"
                )
                assert connection.execute("SELECT sum(order_id) FROM orders").fetchone() == (40,)
                time.sleep(1)
                assert current_snapshot(api, "development")["record"] == baseline["record"]
            time.sleep(1)
            assert current_snapshot(api, "development")["record"] == baseline["record"]
