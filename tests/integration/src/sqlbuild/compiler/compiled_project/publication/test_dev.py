"""The real development process tree retains identity, reloads, streams and executes DuckDB work."""

import json
import os
import signal
import sqlite3
import subprocess
import sys
import time
from datetime import UTC, datetime
from itertools import islice
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    PYTHON_PIPELINE_FILES,
    TYPE_PROOF_FILES,
    duckshop_orchestration,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentAllowHostRefusalTestCase,
    DevelopmentDiscoveryTestCase,
    DevelopmentExposureTestCase,
    DevelopmentInvalidEnvFileTestCase,
    DevelopmentSecretReloadTestCase,
    DevelopmentTestCase,
    DevelopmentWarningTestCase,
    SnapshotApi,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    record_finished_run,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    REGION_PROVIDER_FILES,
    current_snapshot,
    location_process,
    request_identified_run,
    stop_location,
    wait_for_connection_health,
    wait_for_development_ready,
    wait_for_generation,
    wait_for_log,
    wait_for_run,
    wait_for_warehouse_sync_health,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentTestCase(
            "default target and store",
            "development",
            (),
            "project/.sqlbuild/control.sqlite3",
            [(1, 5)],
        ),
        DevelopmentTestCase(
            "encoded target and custom store",
            "preview #1",
            ("--target", "preview #1", "--control-store-url", "{control}"),
            "project/custom-control.sqlite3",
            [(1, 5)],
            '[targets."preview #1"]\nschema = "main"\n',
        ),
        DevelopmentTestCase(
            "local target takes precedence over project default",
            "preview #1",
            (),
            "project/.sqlbuild/control.sqlite3",
            [(1, 5)],
            'target = "preview #1"\n[targets."preview #1"]\nschema = "main"\n',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_development_when_reloaded_and_restarted_then_local_scope_and_paused_defaults_survive(
    test_case: DevelopmentTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    pidfile: Path = tmp_path / "interpreter.pid"
    probe: str = (
        "import os\nfrom pathlib import Path\nfrom sqlbuild.orchestration import calendar\n"
        "assert 'SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL' not in os.environ\n"
        f"Path({str(pidfile)!r}).write_text(str(os.getpid()))\n"
        "@calendar(name='interpreter_probe_calendar', description='Probe.', horizon='1d')\n"
        "def interpreter_probe(window):\n    return []\n"
    )
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": 'default_target = "development"\n'
            # A control store inside the project writes its run output beside it.
            + 'extra_paths = ["output/"]\n'
            + TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
            "sqlbuild_local.toml": test_case.local_config,
            "sql/models/orders.sql": TYPE_PROOF_FILES["sql/models/orders.sql"].replace(
                "SELECT order_id, amount", "SELECT order_id, CAST(amount AS BIGINT) AS amount"
            ),
            **duckshop_orchestration("development_orders"),
            "orchestration/calendars/interpreter_probe_calendar.py": probe,
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    control: Path = tmp_path / test_case.control_path
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"] + [
        argument.format(control=control) for argument in test_case.arguments
    ]
    environment: dict[str, str] = os.environ | {
        "SQLBUILD_CONTROL_DATABASE_URL": "unsupported://not-development",
        "SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL": (
            "http://127.0.0.1:1/audit?token=synthetic-audit-credential"
        ),
    }
    _ = environment.pop("SQLBUILD_PLATFORM_WEBHOOK_URL", None)
    log: Path = tmp_path / "development.log"
    output: Path = tmp_path / "publications.jsonl"
    with location_process(arguments, log, output, env=environment) as process:
        url, identity = wait_for_development_ready(process, log)
        api: SnapshotApi = SnapshotApi(url, identity, "local-development")
        scope: str = f"{url}/api/locations/{identity}/targets/{quote(test_case.target, safe='')}"
        _ = wait_for_generation(api, test_case.target, 1, process, log)
        with urlopen(f"{scope}/triggers", timeout=5) as response:
            triggers: list[dict[str, Any]] = json.load(response)
        assert len(triggers) == 1
        assert triggers[0]["status"] == "paused"
        assert triggers[0]["definition_json"]["default_status"] == "running"
        first_interpreter: int = int(pidfile.read_text())
        for headers in [{"Host": "outside.example.com"}, {"Origin": "https://outside.example.com"}]:
            with pytest.raises(HTTPError) as rejected:
                urlopen(Request(f"{scope}/snapshots", headers=headers), timeout=5)
            assert rejected.value.code == 403
        with pytest.raises(HTTPError) as foreign:
            urlopen(
                f"{url}/api/locations/{uuid4()}/targets/{quote(test_case.target, safe='')}/snapshots",
                timeout=5,
            )
        assert foreign.value.code == 403
        run_id: str = str(uuid4())
        requested: subprocess.CompletedProcess[str] = subprocess.run(
            [
                executable,
                "--project-dir",
                str(project),
                "admin",
                "request-run",
                identity,
                "annual_orders_job",
                "--target",
                test_case.target,
                "--run-id",
                run_id,
                "--reason",
                "Development execution contract",
            ],
            env=environment | {"SQLBUILD_CONTROL_DATABASE_URL": str(control)},
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert requested.returncode == 0, requested.stderr
        run: dict[str, Any] = wait_for_run(
            api, process, log, run_id=run_id, target=test_case.target
        )
        assert run["status"] == "success", log.read_text()
        # The worker uploads its output before it reports the run finished.
        assert list((control.parent / "output/runs" / run_id).rglob("*.zst")), log.read_text()
        time.sleep(1)
        assert current_snapshot(api, test_case.target)["record"]["generation"] == 1
        with duckdb.connect(str(project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert (
                warehouse.sql("SELECT order_id, amount FROM orders").fetchall()
                == test_case.expected_rows
            )
        stop_location(process, first_interpreter, log)
    _ = log.write_text("")
    with sqlite3.connect(control) as connection:
        _ = connection.execute("DELETE FROM sqb_scheduler_lease")
    with location_process(
        arguments + ["--no-scheduler", "--triggers"], log, output, env=environment
    ) as restarted:
        url, restarted_identity = wait_for_development_ready(restarted, log)
        assert restarted_identity == identity
        api = SnapshotApi(url, identity, "local-development")
        scope = f"{url}/api/locations/{identity}/targets/{quote(test_case.target, safe='')}"
        _ = wait_for_generation(api, test_case.target, 2, restarted, log)
        assert int(pidfile.read_text()) != first_interpreter
        with urlopen(f"{scope}/triggers", timeout=5) as response:
            triggers = json.load(response)
        assert triggers[0]["status"] == "paused"
        with urlopen(f"{scope}/changes", timeout=5) as stream:
            assert stream.readline() == b"event: changed\n"
            assert json.loads(stream.readline().removeprefix(b"data: ")) == {"topic": "all"}
            _ = (project / "orchestration/jobs/additional_orders_job.toml").write_text(
                'name = "additional_orders_job"\n'
                'description = "Additional orders."\n'
                'select = "orders"\n'
                'schedule = { cron = "0 0 1 1 *", timezone = "UTC", '
                'starts_at = "2099-01-01T00:00:00Z" }\n'
            )
            assert f"snapshot:{identity}" in islice(
                (
                    json.loads(line[6:])["topic"]
                    for line in filter(lambda line: line.startswith(b"data: "), stream)
                ),
                100,
            )
        _ = wait_for_generation(api, test_case.target, 3, restarted, log)
        with urlopen(f"{scope}/triggers", timeout=5) as response:
            statuses: dict[str, str] = {
                entry["job_key"]: entry["status"] for entry in json.load(response)
            }
        assert statuses == {"annual_orders_job": "paused", "additional_orders_job": "running"}
        stop_location(restarted, int(pidfile.read_text()), log)
    with sqlite3.connect(control) as connection:
        assert connection.execute("SELECT count(*) FROM sqb_scheduler_lease").fetchone() == (0,)
        assert connection.execute("SELECT count(*) FROM sqb_development_locations").fetchone() == (
            1,
        )
    for line in output.read_text().splitlines():
        assert json.loads(line)["location_id"] == identity


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [DevelopmentDiscoveryTestCase("python nodes imported once per start", 1)],
    ids=lambda case: case.description,
)
def test_given_development_start_when_location_is_ready_then_project_python_is_discovered_once(
    test_case: DevelopmentDiscoveryTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    imports: Path = tmp_path / "python_imports.txt"
    probe: str = (
        "import os\nfrom pathlib import Path\nfrom sqlbuild.assets import asset\n"
        f"with Path({str(imports)!r}).open('a') as imported:\n"
        "    imported.write(f'{os.getpid()}\\n')\n"
        "@asset(description='Probe.')\n"
        "def import_probe(ctx):\n    return None\n"
    )
    write_files(
        project_dir=project,
        files=PYTHON_PIPELINE_FILES
        | {
            "sqlbuild_project.toml": 'default_target = "development"\n'
            + PYTHON_PIPELINE_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
            **node_module_files("python/nodes/probes/import_probe.py", probe),
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    log: Path = tmp_path / "development.log"
    output: Path = tmp_path / "publications.jsonl"
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"]
    environment: dict[str, str] = dict(os.environ)
    _ = environment.pop("SQLBUILD_PLATFORM_WEBHOOK_URL", None)
    with location_process(arguments, log, output, env=environment) as process:
        url, identity = wait_for_development_ready(process, log)
        health: dict[str, Any] = wait_for_connection_health(
            SnapshotApi(url, identity, "local-development"), process, log
        )
        assert health["healthy"], log.read_text()
        interpreters: list[str] = imports.read_text().splitlines()
        stop_location(process, int(interpreters[0]), log)

    assert len(interpreters) == test_case.expected_imports, log.read_text()


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentWarningTestCase(
            "a failed warehouse read is a plain line, without the detailed log",
            "  why    retrying in 30s: ",
            "Warehouse history read failed; retaining mirror",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_plain_development_when_a_warehouse_read_fails_then_a_warning_line_says_so(
    test_case: DevelopmentWarningTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=PYTHON_PIPELINE_FILES
        | {
            "sqlbuild_project.toml": 'default_target = "development"\n'
            + PYTHON_PIPELINE_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    log: Path = tmp_path / "development.log"
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"]
    environment: dict[str, str] = dict(os.environ)
    _ = environment.pop("SQLBUILD_PLATFORM_WEBHOOK_URL", None)
    with location_process(
        arguments, log, tmp_path / "publications.jsonl", env=environment
    ) as process:
        url, identity = wait_for_development_ready(process, log)
        api: SnapshotApi = SnapshotApi(url, identity, "local-development")
        _ = wait_for_warehouse_sync_health(
            api,
            {"failure": None, "tables": ["fingerprints"]},
            process,
            log,
            after=datetime(1970, 1, 1, tzinfo=UTC),
        )
        _ = (project / "warehouse.duckdb").rename(tmp_path / "retained.duckdb")
        record_finished_run(project / ".sqlbuild/control.sqlite3")
        wait_for_log(process, log, test_case.expected_line, timeout=45)
        process.send_signal(signal.SIGTERM)
        assert process.wait(timeout=15) == 128 + signal.SIGTERM, log.read_text()

    assert test_case.hidden_line not in log.read_text()


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentSecretReloadTestCase(
            "a secret added to .env reaches the next worker run",
            "UNRELATED_SETTING=1\n",
            "WORKER_ENV_REGION_CODE=42\nWORKER_ENV_TOKEN=worker-token\n",
            "failed",
            ("source:raw_regions", "failed", "D012"),
            "success",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_missing_secret_when_it_is_added_to_the_env_file_then_the_next_run_reads_it(
    test_case: DevelopmentSecretReloadTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "development"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n"
                'SELECT CAST(code AS BIGINT) AS amount FROM __source("raw_regions")\n'
            ),
            ".env": test_case.first_env_file,
        }
        | REGION_PROVIDER_FILES
        | duckshop_orchestration("upstream_loaded_orders"),
    )
    (project / ".env").chmod(0o600)
    executable: str = str(Path(sys.executable).with_name("sqb"))
    environment: dict[str, str] = dict(os.environ)
    for name in ("SQLBUILD_PLATFORM_WEBHOOK_URL", "SQLBUILD_CONTROL_DATABASE_URL"):
        _ = environment.pop(name, None)
    admin: dict[str, str] = environment | {
        "SQLBUILD_CONTROL_DATABASE_URL": str(project / ".sqlbuild/control.sqlite3")
    }
    log: Path = tmp_path / "development.log"
    output: Path = tmp_path / "publications.jsonl"
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"]
    with location_process(arguments, log, output, env=environment) as process:
        url, identity = wait_for_development_ready(process, log)
        api: SnapshotApi = SnapshotApi(url, identity, "local-development")
        _ = wait_for_generation(api, "development", 1, process, log)
        first_id: str = request_identified_run(executable, project, identity, admin)
        first: dict[str, Any] = wait_for_run(api, process, log, run_id=first_id)
        with sqlite3.connect(project / ".sqlbuild/control.sqlite3") as control:
            node: tuple[str, str, str | None] = control.execute(
                "SELECT node_key, status, error_code FROM sqb_run_nodes "
                "WHERE run_id=? AND node_key='source:raw_regions'",
                (first_id,),
            ).fetchone()
        _ = (project / ".env").write_text(test_case.second_env_file)
        _ = wait_for_generation(api, "development", 2, process, log)
        second_id: str = request_identified_run(executable, project, identity, admin)
        second: dict[str, Any] = wait_for_run(api, process, log, run_id=second_id)

    assert first["status"] == test_case.expected_first_status, log.read_text()
    assert node == test_case.expected_first_node, log.read_text()
    assert second["status"] == test_case.expected_second_status, log.read_text()


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentInvalidEnvFileTestCase(
            "an unparsable .env fails the next run with its line, never its text",
            "WORKER_ENV_REGION_CODE=42\nWORKER_ENV_TOKEN=worker-token\n",
            "WORKER_ENV_REGION_CODE=42\nWORKER_ENV_TOKEN=worker-token\nnot a pair s3cr3t\n",
            "failed",
            "Couldn't run: env file .env line 3 is not KEY=value "
            '(set by the default env_file = ".env"); nothing ran',
            "s3cr3t",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unparsable_env_file_when_a_run_starts_then_its_reason_names_the_line(
    test_case: DevelopmentInvalidEnvFileTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "development"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n"
                'SELECT CAST(code AS BIGINT) AS amount FROM __source("raw_regions")\n'
            ),
            ".env": test_case.first_env_file,
        }
        | REGION_PROVIDER_FILES
        | duckshop_orchestration("upstream_loaded_orders"),
    )
    (project / ".env").chmod(0o600)
    executable: str = str(Path(sys.executable).with_name("sqb"))
    environment: dict[str, str] = dict(os.environ)
    for name in ("SQLBUILD_PLATFORM_WEBHOOK_URL", "SQLBUILD_CONTROL_DATABASE_URL"):
        _ = environment.pop(name, None)
    admin: dict[str, str] = environment | {
        "SQLBUILD_CONTROL_DATABASE_URL": str(project / ".sqlbuild/control.sqlite3")
    }
    log: Path = tmp_path / "development.log"
    output: Path = tmp_path / "publications.jsonl"
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"]
    with location_process(arguments, log, output, env=environment) as process:
        url, identity = wait_for_development_ready(process, log)
        api: SnapshotApi = SnapshotApi(url, identity, "local-development")
        _ = wait_for_generation(api, "development", 1, process, log)
        _ = (project / ".env").write_text(test_case.invalid_env_file)
        wait_for_log(process, log, "env file not reloaded")
        # Queued from outside the project, as the UI does, so the request itself reads no .env.
        run_id: str = request_identified_run(executable, tmp_path, identity, admin)
        run: dict[str, Any] = wait_for_run(api, process, log, run_id=run_id)

    assert run["status"] == test_case.expected_status, log.read_text()
    assert run["status_reason"] == test_case.expected_reason, log.read_text()
    assert test_case.secret_text not in log.read_text()


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentExposureTestCase(
            "every interface and an allowed name",
            ("--host", "0.0.0.0", "--allow-host", "control.example.com"),
            200,
            ("No sign-in", "  access     anyone who can reach 0.0.0.0:"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_opened_development_server_when_started_then_it_publishes_takes_the_name_and_warns(
    test_case: DevelopmentExposureTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": 'default_target = "development"\n'
            + TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
            "sql/models/orders.sql": TYPE_PROOF_FILES["sql/models/orders.sql"].replace(
                "SELECT order_id, amount", "SELECT order_id, CAST(amount AS BIGINT) AS amount"
            ),
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"]
    log: Path = tmp_path / "development.log"
    output: Path = tmp_path / "publications.jsonl"
    with location_process(arguments + list(test_case.arguments), log, output) as process:
        url, identity = wait_for_development_ready(process, log)
        api: SnapshotApi = SnapshotApi(url, identity, "local-development")
        _ = wait_for_generation(api, "development", 1, process, log)
        request: Request = Request(
            f"{url}/api/control/bootstrap",
            headers={"Host": "control.example.com", "Origin": "https://control.example.com"},
        )
        with urlopen(request, timeout=5) as response:
            assert response.status == test_case.expected_status
        assert all(line in log.read_text() for line in test_case.expected_lines), log.read_text()


@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentAllowHostRefusalTestCase("an origin", "https://control.example.com", 2),
        DevelopmentAllowHostRefusalTestCase("a wildcard", "*", 2),
        DevelopmentAllowHostRefusalTestCase("a port out of range", "control.example.com:70000", 2),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unusable_allowed_host_when_development_starts_then_it_is_refused_by_name(
    test_case: DevelopmentAllowHostRefusalTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": 'default_target = "development"\n'
            + TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"]
    started: subprocess.CompletedProcess[str] = subprocess.run(
        [*arguments, "--allow-host", test_case.value],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    assert started.returncode == test_case.expected_exit_code, started.stderr
    assert "--allow-host" in started.stderr
