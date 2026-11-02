"""A real owner, browser, scheduler, location and DuckDB share one permission-enforced journey."""

import json
import sys
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from uuid import uuid4

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    location_process,
    scheduled_worker,
    wait_for_generation,
    wait_for_location_health,
    wait_for_run,
    worker_result,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_read,
    person_request,
    provision_person,
)


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            "owner bootstrap, least-privilege browser operations, reload and last-good recovery",
            "SELECT 23 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(23,)],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_initialized_installation_when_people_run_and_reload_then_permissions_and_recovery_hold(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(
        tmp_path,
        monkeypatch,
        test_case,
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as worker:
        scheduled: dict[str, Any] = worker_result(worker, test_case)
        admin: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        person: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-person"
        )
        admin_session: tuple[str, str] = person_login(worker.api, admin["name"])
        session: tuple[str, str] = person_login(worker.api, person["name"])
        runs_path: str = f"/api/control/locations/{worker.api.location_id}/targets/development/runs"
        request: dict[str, Any] = {
            "run_id": str(uuid4()),
            "location_id": worker.api.location_id,
            "target": "development",
            "job_key": "annual_orders",
            "reason": "Refresh order totals",
            "priority": 0,
        }
        assignment: dict[str, Any] = {
            "principal_id": person["principal_id"],
            "role": "viewer",
            "location_id": worker.api.location_id,
            "target": "development",
        }
        with pytest.raises(HTTPError) as no_role:
            person_read(worker.api, runs_path, session=session)
        assert no_role.value.code == 403
        for path, body in [
            ("/api/control/runs", request),
            ("/api/admin/roles", assignment),
        ]:
            with pytest.raises(HTTPError) as rejected:
                person_request(worker.api, path, body=body, session=session)
            assert rejected.value.code == 403
            assert "decision" in json.load(rejected.value)["error"]
        assert person_request(
            worker.api, "/api/admin/roles", body=assignment, session=admin_session
        ) == {"applied": True}
        metadata: list[dict[str, Any]] = person_read(worker.api, runs_path, session=session)
        assert metadata[0]["run_id"] == scheduled["run_id"]
        assert not ({"sql", "compiled_sql", "output", "rows", "evidence"} & metadata[0].keys())
        for path, body in [
            ("/api/control/runs", request),
            ("/api/admin/people/" + admin["principal_id"], "disable"),
        ]:
            with pytest.raises(HTTPError) as rejected:
                person_request(worker.api, path, body=body, session=session)
            assert rejected.value.code == 403
        with pytest.raises(HTTPError) as wrong_target:
            person_read(worker.api, runs_path.replace("development", "preview"), session=session)
        assert wrong_target.value.code == 403
        assignment["role"] = "operator"
        assert person_request(
            worker.api, "/api/admin/roles", body=assignment, session=admin_session
        ) == {"applied": True}
        with pytest.raises(HTTPError) as csrf_rejected:
            person_request(
                worker.api, "/api/control/runs", body=request, session=(session[0], "wrong-csrf")
            )
        assert csrf_rejected.value.code == 401
        for sensitive in [request | {"window": {"kind": "integer", "start": 3, "end": 8}}]:
            with pytest.raises(HTTPError) as rewrite_rejected:
                person_request(worker.api, "/api/control/runs", body=sensitive, session=session)
            assert rewrite_rejected.value.code == 403
        assert person_request(worker.api, "/api/control/runs", body=request, session=session) == {
            "applied": True
        }
        manual: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=request["run_id"]
        )
        assert manual["status"] == test_case.expected_status
        assert person_request(worker.api, "/api/control/runs", body=request, session=session) == {
            "applied": False
        }
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", worker.api.bearer)
        log: Path = tmp_path / "location.log"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(worker.project),
                "location",
                "serve",
                "--server",
                worker.api.url,
                "--location-id",
                worker.api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
        ) as location:
            _ = wait_for_generation(worker.api, "development", 2, location, log)
            model: Path = worker.project / "models/orders.sql"
            original: str = model.read_text()
            _ = model.write_text(original.replace("23", "37"))
            edited: dict[str, Any] = wait_for_generation(
                worker.api, "development", 3, location, log
            )
            request["run_id"] = str(uuid4())
            assert person_request(
                worker.api, "/api/control/runs", body=request, session=session
            ) == {"applied": True}
            reloaded: dict[str, Any] = wait_for_run(
                worker.api, worker.process, worker.log, run_id=request["run_id"]
            )
            assert reloaded["status"] == test_case.expected_status
            assert reloaded["snapshot_id"] == edited["record"]["snapshot_id"]
            with duckdb.connect(
                str(worker.project / "warehouse.duckdb"), read_only=True
            ) as warehouse:
                assert warehouse.sql(test_case.warehouse_query).fetchall() == [(37,)]
            _ = model.write_text("SELECT FROM invalid SQL !\n")
            _ = wait_for_location_health(
                worker.api,
                "development",
                {"kind": "compile_failed", "generation": 4},
                location,
                log,
            )
            assert current_snapshot(worker.api, "development")["record"] == edited["record"]
            _ = model.write_text(original.replace("23", "41"))
            recovered: dict[str, Any] = wait_for_generation(
                worker.api, "development", 4, location, log
            )
            request["run_id"] = str(uuid4())
            assert person_request(
                worker.api, "/api/control/runs", body=request, session=session
            ) == {"applied": True}
            recovery: dict[str, Any] = wait_for_run(
                worker.api, worker.process, worker.log, run_id=request["run_id"]
            )
            assert recovery["status"] == test_case.expected_status
            assert recovery["snapshot_id"] == recovered["record"]["snapshot_id"]
            with duckdb.connect(
                str(worker.project / "warehouse.duckdb"), read_only=True
            ) as warehouse:
                assert warehouse.sql(test_case.warehouse_query).fetchall() == [(41,)]
        assignment["revoke"] = True
        assert person_request(
            worker.api, "/api/admin/roles", body=assignment, session=admin_session
        ) == {"applied": True}
        with pytest.raises(HTTPError) as revoked:
            person_request(
                worker.api,
                "/api/control/runs",
                body=request | {"run_id": str(uuid4())},
                session=session,
            )
        assert revoked.value.code == 403
