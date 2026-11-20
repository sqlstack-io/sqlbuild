"""A served DuckDB location with a scheduler, where people preview and run builds that name jobs."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from collections.abc import Iterator
from contextlib import contextmanager
from datetime import UTC, datetime
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
}


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
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> Iterator[JobSettingsLocation]:
    """Build the project, serve it with a scheduler's local workers, and sign an admin in."""
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=JOB_PROJECT_FILES)
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
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
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
    keys: tuple[str, ...] = ("full_refresh_jobs", "job_concurrency", "job_warehouses")
    return {key: document[key] for key in document.keys() & set(keys)}


def error_code(document: dict[str, Any]) -> str | None:
    """The plan's error code, or None when it planned."""
    return document.get("error", {}).get("code")
