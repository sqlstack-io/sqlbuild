"""A served DuckDB location with a scheduler, where people preview and run builds that name jobs."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from collections.abc import Iterable, Iterator
from contextlib import contextmanager
from datetime import UTC, datetime
from itertools import accumulate
from pathlib import Path
from typing import Any, NamedTuple
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SnapshotApi,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    run_sqb,
    snapshot_api,
    wait_for_generation,
    wait_for_location_work,
    wait_for_run,
    wait_for_warehouse_sync_health,
    write_server_config,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_request,
    provision_person,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

WAREHOUSE_REFUSAL: str = (
    "the requested warehouse is not configured for this target on this location; "
    "no warehouse statement ran"
)
JOB_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n'
        '[connections.local]\ndatabase = "warehouse.duckdb"\n'
        '[targets.development]\nschema = "main"\n'
        '[vars]\namount = "1"\n'
    ),
    "seeds/raw_orders.csv": "id\n1\n2\n",
    "seeds/raw_orders.yml": (
        "seeds:\n  - name: raw_orders\n    description: Raw orders.\n    columns:\n"
        "      - name: id\n        type: INTEGER\n"
    ),
    "models/orders.sql": (
        "MODEL (description 'Orders with their amount.', materialized incremental,\n"
        "  incremental_strategy append, cursor id, cursor_type integer);\n"
        'SELECT id, CAST(@@amount AS INTEGER) AS amount FROM __seed("raw_orders")\n'
    ),
    "orchestration/jobs/refresh_job.toml": (
        'name = "refresh_job"\ndescription = "Rebuild orders at the job amount."\n'
        'select = "orders"\n'
        '[settings]\nfull_refresh = true\nvars = { amount = "5" }\n'
    ),
    "orchestration/jobs/limited_job.toml": (
        'name = "limited_job"\ndescription = "Orders, one at a time."\nselect = "orders"\n'
        "[settings]\nconcurrency = 1\n"
    ),
    "orchestration/jobs/warehouse_job.toml": (
        'name = "warehouse_job"\ndescription = "Orders on a reporting warehouse."\n'
        'select = "orders"\n'
        '[settings]\nwarehouse = "REPORTS_XS"\nvars = { amount = "5" }\n'
    ),
    "orchestration/jobs/changed_job.toml": (
        'name = "changed_job"\ndescription = "Orders when they changed."\nselect = "orders"\n'
        "[settings]\nchanged = true\n"
    ),
}
LOADER_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n'
        '[connections.local]\ndatabase = "warehouse.duckdb"\n'
        '[targets.development]\nschema = "main"\n'
        "[settings]\nconcurrency = 1\n"
    ),
    "sources/loaded.yml": (
        "sources:\n"
        "  - name: raw_customers\n    description: Customers.\n    managed: true\n"
        "    write_strategy: table\n    table: raw_customers\n    columns:\n"
        "      - name: customer_id\n        type: BIGINT\n"
        "  - name: raw_inventory\n    description: Inventory.\n    managed: true\n"
        "    write_strategy: table\n    table: raw_inventory\n    columns:\n"
        "      - name: product_id\n        type: BIGINT\n"
    ),
    "python/loaders/loaded.py": (
        '"""Two loaders that record each run in the warehouse log."""\n\n'
        "import os\nfrom pathlib import Path\n\nfrom sqlbuild.loaders import loader\n\n\n"
        "def _record(name):\n"
        '    with Path(os.environ["WAREHOUSE_LOG"]).open("a") as log:\n'
        '        log.write(f"LOAD {name}\\n")\n\n\n'
        "@loader\ndef raw_customers(ctx):\n"
        '    """Load customers."""\n    _record("raw_customers")\n'
        '    return [{"customer_id": 7}]\n\n\n'
        "@loader\ndef raw_inventory(ctx):\n"
        '    """Load inventory."""\n    _record("raw_inventory")\n'
        '    return [{"product_id": 1}]\n\n\n'
        '@loader(write_strategy="table")\ndef stage_inventory(ctx):\n'
        '    """Stage inventory in the loader\'s own relation; no source declares it."""\n'
        '    _record("stage_inventory")\n'
        '    return [{"product_id": 2}]\n'
    ),
    "models/customers.sql": (
        "MODEL (description 'Loaded customers.', materialized table);\n"
        'SELECT customer_id FROM __source("raw_customers")\n'
    ),
    "orchestration/jobs/loads_job.toml": (
        'name = "loads_job"\ndescription = "Inventory loads on their own warehouse."\n'
        'select = "source:raw_inventory"\n'
        '[settings]\nwarehouse = "LOADS_WH"\n'
    ),
    "orchestration/jobs/stage_job.toml": (
        'name = "stage_job"\ndescription = "Staged inventory on its own warehouse."\n'
        'select = "loader:stage_inventory"\n'
        '[settings]\nwarehouse = "STAGE_WH"\n'
    ),
    "orchestration/jobs/customers_job.toml": (
        'name = "customers_job"\ndescription = "Customers."\nselect = "customers"\n'
    ),
}
# DuckDB with a session warehouse that logs each switch, showing the warehouse each node ran on.
WAREHOUSE_HOOK: str = (
    "import os\nfrom pathlib import Path\n"
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n"
    "DuckDbAdapter.supports_session_warehouse = True\n"
    "def use_session_warehouse(self, *, connection, warehouse):\n"
    '    with Path(os.environ["WAREHOUSE_LOG"]).open("a") as log:\n'
    '        log.write(f"USE {warehouse}\\n")\n'
    "    self._session_warehouse = warehouse\n"
    "def current_session_warehouse(self, *, connection):\n"
    '    return getattr(self, "_session_warehouse", "HOME_WH")\n'
    "DuckDbAdapter.use_session_warehouse = use_session_warehouse\n"
    "DuckDbAdapter.current_session_warehouse = current_session_warehouse\n"
    "DuckDbAdapter.session_warehouse = current_session_warehouse\n"
)


class JobSettingsLocation(NamedTuple):
    api: SnapshotApi
    project: Path
    served: subprocess.Popen[bytes]
    log: Path
    scheduler: subprocess.Popen[bytes]
    scheduler_log: Path
    session: tuple[str, str]


@contextmanager
def job_settings_location(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    files: dict[str, str] = JOB_PROJECT_FILES,
    environment: dict[str, str] | None = None,
) -> Iterator[JobSettingsLocation]:
    """Build the project, serve it with a scheduler's local workers, and sign an admin in."""
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=files)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    executable: str = str(Path(sys.executable).with_name("sqb"))
    log: Path = tmp_path / "location.log"
    started: datetime = datetime.now(UTC)
    with (
        snapshot_api(
            tmp_path / "control",
            server_arguments=("--password-origin", "https://control.example.com"),
        ) as api,
        location_process(
            [
                executable,
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
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | (environment or {}) | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as served,
    ):
        _ = wait_for_generation(api, "development", 1, served, log)
        _ = wait_for_warehouse_sync_health(
            api,
            {"failure": None, "tables": ["catalog", "consumption", "fingerprints"]},
            served,
            log,
            after=started,
        )
        configuration: Path = tmp_path / "server.toml"
        write_server_config(
            configuration,
            {
                "launchers": {
                    "local": [
                        {
                            "location": "orders",
                            "target": "development",
                            "server_url": api.url,
                            "max_workers": 1,
                            "project_dir": str(project),
                            "executable": executable,
                        }
                    ]
                }
            },
        )
        control_store: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
        monkeypatch.setenv("SQLBUILD_SCHEDULER_TEST_STORE", f"sqlite://{control_store}")
        scheduler_log: Path = tmp_path / "scheduler.log"
        with location_process(
            [
                executable,
                "scheduler",
                "--database-env",
                "SQLBUILD_SCHEDULER_TEST_STORE",
                "--config",
                str(configuration),
            ],
            scheduler_log,
            tmp_path / "scheduler.stdout",
            env=os.environ | (environment or {}),
        ) as scheduler:
            admin: dict[str, Any] = provision_person(
                tmp_path / "control", "create-user", "orders-admin"
            )
            yield JobSettingsLocation(
                api,
                project,
                served,
                log,
                scheduler,
                scheduler_log,
                person_login(api, admin["name"]),
            )


def preview(location: JobSettingsLocation, command: list[str]) -> dict[str, Any]:
    """The plan document of a person's Plan of `command`, frozen by the server like a run."""
    api: SnapshotApi = location.api
    request_id: str = str(uuid4())
    _ = person_request(
        api,
        f"/api/control/locations/{api.location_id}/targets/development/plans",
        body={"request_id": request_id, "command": command, "fresh": True},
        session=location.session,
    )
    result: dict[str, Any] = wait_for_location_work(api, request_id, location.served, location.log)
    outcome: dict[str, Any] = result["result_json"]["outcome"]
    assert outcome["kind"] == "plan", outcome
    return outcome["document"]


def adhoc_run(location: JobSettingsLocation, command: list[str]) -> dict[str, Any]:
    """A person's ad-hoc run of `command`, once the scheduler's worker finished it."""
    api: SnapshotApi = location.api
    run_id: str = str(uuid4())
    _ = person_request(
        api,
        "/api/control/runs/adhoc",
        body={
            "location_id": api.location_id,
            "target": "development",
            "run_id": run_id,
            "command": command,
            "reason": "Build what the job builds",
            "priority": 0,
        },
        session=location.session,
    )
    return wait_for_run(api, location.scheduler, location.scheduler_log, run_id=run_id)


def cli_plan(project: Path, command: list[str]) -> dict[str, Any]:
    """`sqb plan --json` of `command`'s selectors, as the command line plans them."""
    planned: subprocess.CompletedProcess[str] = run_sqb(
        project, "plan", "--target", "development", "--json", "--no-color", *command[1:]
    )
    return json.loads(planned.stdout)


def planned_actions(document: dict[str, Any]) -> dict[str, tuple[str, str]]:
    """Each planned model's action and reason."""
    return {
        model["name"]: (model["action"], model["reason"]) for model in document.get("models", [])
    }


def job_keys(document: dict[str, Any]) -> dict[str, Any]:
    """The plan's keys about named jobs: full refresh, concurrency and warehouses."""
    keys: tuple[str, ...] = ("full_refresh_jobs", "job_concurrency", "job_warehouses", "changed")
    return {key: document[key] for key in document.keys() & set(keys)}


def error_code(document: dict[str, Any]) -> str | None:
    """The plan's error code, or None when it planned."""
    return document.get("error", {}).get("code")


def loader_warehouses(log: str, loaders: Iterable[str]) -> dict[str, str]:
    """Each of `loaders` with the warehouse current when the warehouse log records it ran."""
    lines: list[list[str]] = [line.split(" ", 1) for line in log.splitlines()]
    current: list[str] = list(
        accumulate(
            lines,
            lambda warehouse, line: {"USE": line[1]}.get(line[0], warehouse),
            initial="HOME_WH",
        )
    )
    ran: dict[str, str] = dict(
        (line[1], warehouse)
        for line, warehouse in filter(
            lambda pair: pair[0][0] == "LOAD", zip(lines, current[1:], strict=True)
        )
    )
    return {loader: ran.get(loader, "did not run") for loader in loaders}
