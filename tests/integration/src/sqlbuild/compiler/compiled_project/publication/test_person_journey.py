"""A real owner, browser, scheduler, location and DuckDB share one permission-enforced journey."""

import json
import sys
from contextlib import nullcontext
from functools import partial
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from uuid import uuid4

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    PersonJourneyTestCase,
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
    verify_security_audit,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PersonJourneyTestCase(
            description="no-role default, scoped browser operations, reload and last-good recovery",
            model_sql="SELECT 23 AS amount",
            warehouse_query="SELECT amount FROM orders",
            expected_status="success",
            expected_rows=[(23,)],
            default_role="none",
            expected_metadata_access=partial(pytest.raises, HTTPError, match="HTTP Error 403"),
        ),
        PersonJourneyTestCase(
            description="configured viewer default, scoped execution, reload and last-good recovery",
            model_sql="SELECT 23 AS amount",
            warehouse_query="SELECT amount FROM orders",
            expected_status="success",
            expected_rows=[(23,)],
            default_role="viewer",
            expected_metadata_access=nullcontext,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_initialized_installation_when_people_run_and_reload_then_permissions_and_recovery_hold(
    test_case: PersonJourneyTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    with scheduled_worker(
        tmp_path,
        monkeypatch,
        test_case,
        server_arguments=(
            "--password-origin",
            "https://control.example.com",
            "--default-role",
            test_case.default_role,
        ),
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
        verify_security_audit(worker.api, admin_session, session)
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
        rerun: dict[str, Any] = {
            "run_id": str(uuid4()),
            "source_run_id": scheduled["run_id"],
            "from_failure": False,
            "reason": "Repeat the pinned order build",
            "priority": 0,
        }
        cancellation: dict[str, Any] = {
            "run_id": scheduled["run_id"],
            "event_id": str(uuid4()),
            "reason": "Stop the order build",
        }
        with test_case.expected_metadata_access():
            assert (
                person_read(worker.api, runs_path, session=session)[0]["run_id"]
                == scheduled["run_id"]
            )
        for path, body in [
            ("/api/control/runs", request),
            ("/api/admin/roles", assignment),
            ("/api/control/runs/cancel", cancellation),
            ("/api/control/runs/rerun", rerun),
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
            ("/api/control/runs/cancel", cancellation),
            ("/api/control/runs/rerun", rerun),
            ("/api/admin/people/" + admin["principal_id"], "disable"),
        ]:
            with pytest.raises(HTTPError) as rejected:
                person_request(worker.api, path, body=body, session=session)
            assert rejected.value.code == 403
        with test_case.expected_metadata_access():
            assert (
                person_read(
                    worker.api, runs_path.replace("development", "preview"), session=session
                )
                == []
            )
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
        assert (
            wait_for_run(worker.api, worker.process, worker.log, run_id=request["run_id"])["status"]
            == test_case.expected_status
        )
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
        for path, body in [
            ("/api/control/runs", request | {"run_id": str(uuid4())}),
            ("/api/control/runs/cancel", cancellation),
            ("/api/control/runs/rerun", rerun),
        ]:
            with pytest.raises(HTTPError) as revoked:
                person_request(worker.api, path, body=body, session=session)
            assert revoked.value.code == 403


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            description="scoped operator frozen rerun, cancellation and revoked-state privacy",
            model_sql="SELECT 23 AS amount",
            warehouse_query="SELECT amount FROM orders",
            expected_status="success",
            expected_rows=[(23,)],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_person_run_controls_when_scopes_change_then_frozen_execution_and_state_privacy_hold(
    test_case: WorkerExecutionTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
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
        assignment: dict[str, Any] = {
            "principal_id": person["principal_id"],
            "role": "operator",
            "location_id": worker.api.location_id,
            "target": "preview",
        }
        assert person_request(
            worker.api, "/api/admin/roles", body=assignment, session=admin_session
        ) == {"applied": True}
        rerun: dict[str, Any] = {
            "run_id": str(uuid4()),
            "source_run_id": scheduled["run_id"],
            "from_failure": False,
            "reason": "Repeat pinned order build",
            "priority": 0,
        }
        for from_failure in [False, True]:
            with pytest.raises(HTTPError) as out_of_scope:
                person_request(
                    worker.api,
                    "/api/control/runs/rerun",
                    body=rerun | {"from_failure": from_failure},
                    session=session,
                )
            assert out_of_scope.value.code == 403
        assignment["target"] = "development"
        assert person_request(
            worker.api, "/api/admin/roles", body=assignment, session=admin_session
        ) == {"applied": True}
        for applied in [True, False]:
            assert person_request(
                worker.api, "/api/control/runs/rerun", body=rerun, session=session
            ) == {"applied": applied}
        repeated: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=rerun["run_id"]
        )
        assert repeated["status"] == test_case.expected_status
        assert repeated["snapshot_id"] == scheduled["snapshot_id"]
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows
        worker.process.terminate()
        assert worker.process.wait(timeout=5) == 0
        request: dict[str, Any] = {
            "run_id": str(uuid4()),
            "location_id": worker.api.location_id,
            "target": "development",
            "job_key": "annual_orders",
            "reason": "Build order totals",
            "priority": 0,
        }
        assert person_request(worker.api, "/api/control/runs", body=request, session=session) == {
            "applied": True
        }
        cancellation: dict[str, Any] = {
            "run_id": request["run_id"],
            "event_id": str(uuid4()),
            "reason": "Stop before launch",
        }
        with pytest.raises(HTTPError) as forged:
            person_request(
                worker.api,
                "/api/control/runs/cancel",
                body=cancellation | {"actor": "local-control-store-owner"},
                session=session,
            )
        assert forged.value.code == 422
        with pytest.raises(HTTPError) as csrf:
            person_request(
                worker.api,
                "/api/control/runs/cancel",
                body=cancellation,
                session=(session[0], "wrong-csrf"),
            )
        assert csrf.value.code == 401
        for applied in [True, False]:
            assert person_request(
                worker.api, "/api/control/runs/cancel", body=cancellation, session=session
            ) == {"applied": applied}
        runs_path: str = f"/api/control/locations/{worker.api.location_id}/targets/development/runs"
        assert person_read(worker.api, runs_path, session=session)[0]["status"] == "cancelled"
        assignment["revoke"] = True
        assert person_request(
            worker.api, "/api/admin/roles", body=assignment, session=admin_session
        ) == {"applied": True}
        queued: dict[str, Any] = request | {"run_id": str(uuid4())}
        assert person_request(
            worker.api, "/api/control/runs", body=queued, session=admin_session
        ) == {"applied": True}
        with pytest.raises(HTTPError) as metadata:
            person_read(worker.api, runs_path, session=session)
        assert metadata.value.code == 403
        for source_id in [scheduled["run_id"], request["run_id"], queued["run_id"], str(uuid4())]:
            for from_failure in [False, True]:
                with pytest.raises(HTTPError) as private:
                    person_request(
                        worker.api,
                        "/api/control/runs/rerun",
                        body=rerun
                        | {
                            "run_id": str(uuid4()),
                            "source_run_id": source_id,
                            "from_failure": from_failure,
                        },
                        session=session,
                    )
                assert private.value.code == 403
            with pytest.raises(HTTPError) as cancel_denied:
                person_request(
                    worker.api,
                    "/api/control/runs/cancel",
                    body=cancellation | {"run_id": source_id},
                    session=session,
                )
            assert cancel_denied.value.code == 403
