"""The real development process tree retains identity, reloads, streams and executes DuckDB work."""

import json
import os
import sqlite3
import subprocess
import sys
import time
from itertools import islice
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.parse import quote
from urllib.request import Request, urlopen
from uuid import uuid4

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import TYPE_PROOF_FILES
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentTestCase,
    SnapshotApi,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    location_process,
    stop_location,
    wait_for_generation,
    wait_for_log,
    wait_for_run,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


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
    declaration: str = (
        "import os\nfrom pathlib import Path\n"
        f"Path({str(pidfile)!r}).write_text(str(os.getpid()))\n"
        + Path("benchmarks/duckshop/edge_cases/orchestration/development_orders.py").read_text()
    )
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": 'default_target = "development"\n'
            + TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
            "sqlbuild_local.toml": test_case.local_config,
            "models/orders.sql": TYPE_PROOF_FILES["models/orders.sql"].replace(
                "SELECT order_id, amount", "SELECT order_id, CAST(amount AS BIGINT) AS amount"
            ),
            "orchestration/orders.py": declaration,
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    control: Path = tmp_path / test_case.control_path
    arguments: list[str] = [executable, "dev", "--project-dir", str(project), "--port", "0"] + [
        argument.format(control=control) for argument in test_case.arguments
    ]
    environment: dict[str, str] = os.environ | {
        "SQLBUILD_CONTROL_DATABASE_URL": "unsupported://not-development"
    }
    _ = environment.pop("SQLBUILD_PLATFORM_WEBHOOK_URL", None)
    log: Path = tmp_path / "development.log"
    output: Path = tmp_path / "publications.jsonl"
    with location_process(arguments, log, output, env=environment) as process:
        wait_for_log(process, log, "Development API ready: ")
        url: str = (
            "http://" + log.read_text().partition("Development API ready: ")[2].splitlines()[0]
        )
        identity: str = log.read_text().partition("Development location: ")[2].splitlines()[0]
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
                "annual_orders",
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
        time.sleep(1)
        assert current_snapshot(api, test_case.target)["record"]["generation"] == 1
        with duckdb.connect(str(project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert (
                warehouse.sql("SELECT order_id, amount FROM orders").fetchall()
                == test_case.expected_rows
            )
        stop_location(process, first_interpreter, log)
    _ = log.write_text("")
    with location_process(
        arguments + ["--no-scheduler", "--triggers"], log, output, env=environment
    ) as restarted:
        wait_for_log(restarted, log, "Development API ready: ")
        url = "http://" + log.read_text().partition("Development API ready: ")[2].splitlines()[0]
        assert log.read_text().partition("Development location: ")[2].splitlines()[0] == identity
        assert "Starting development scheduler" not in log.read_text()
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
            _ = (project / "orchestration/orders.py").write_text(
                declaration
                + '\nadditional_orders = job("additional_orders", description="Additional orders.", select="orders", schedule=annual)\n'
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
        assert statuses == {"annual_orders": "paused", "additional_orders": "running"}
        stop_location(restarted, int(pidfile.read_text()), log)
    with sqlite3.connect(control) as connection:
        assert connection.execute("SELECT count(*) FROM sqb_development_locations").fetchone() == (
            1,
        )
    for line in output.read_text().splitlines():
        assert json.loads(line)["location_id"] == identity
