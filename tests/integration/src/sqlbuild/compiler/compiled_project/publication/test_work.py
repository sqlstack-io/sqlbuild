"""Trusted owner requests cross the real HTTP channel to an isolated DuckDB connection check."""

import json
import os
import sqlite3
import subprocess
import sys
from functools import partial
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import TYPE_PROOF_FILES
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    LocationWorkTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    snapshot_api,
    wait_for_generation,
    wait_for_interpreter,
    wait_for_location_health,
    wait_for_location_work,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_read,
    person_request,
    provision_person,
    verify_connection_test_transport,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        LocationWorkTestCase("healthy requested check", "warehouse.db", True),
        LocationWorkTestCase("failed requested check", "missing/warehouse.db", False),
    ],
    ids=lambda case: case.description,
)
def test_given_a_published_location_when_an_owner_requests_work_then_results_return_outbound_and_stale_work_is_not_executed(
    test_case: LocationWorkTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    blocked: Path = tmp_path / "block-debug"
    child_pid: Path = tmp_path / "debug.pid"
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + f"\n[connections.local]\ndatabase = {json.dumps(test_case.database)}\n"
            + '\n[targets."preview #1"]\nschema = "main"\n',
            "adapters/slow.py": (
                "import os, sys, time\nfrom pathlib import Path\n"
                f"if 'debug' in sys.argv and Path({str(blocked)!r}).exists():\n"
                f"    Path({str(child_pid)!r}).write_text(str(os.getpid()))\n"
                f"    while Path({str(blocked)!r}).exists():\n"
                "        time.sleep(0.02)\n"
            ),
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    with snapshot_api(tmp_path / "control") as api:
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        # The serving location must never open the owner's database.
        monkeypatch.setenv("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://must-not-open")
        owner_environment: dict[str, str] = os.environ | {
            "SQLBUILD_CONTROL_DATABASE_URL": str(tmp_path / "control/.sqlbuild/control.sqlite3")
        }
        arguments: list[str] = [
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
            "preview #1",
        ]
        request_id: str = str(uuid4())
        enqueue: list[str] = [
            executable,
            "admin",
            "check-location",
            api.location_id,
            "--target",
            "preview #1",
            "--request-id",
            request_id,
            "--reason",
            "Check orders connectivity",
        ]
        log: Path = tmp_path / "location.log"
        publications: Path = tmp_path / "publications.jsonl"
        with location_process(arguments, log, publications) as process:
            _ = wait_for_generation(api, "preview #1", 1, process, log)
            _ = wait_for_location_health(
                api,
                "preview #1",
                {"kind": "connection", "generation": 1, "healthy": test_case.expected_connection},
                process,
                log,
            )
            first: subprocess.CompletedProcess[str] = subprocess.run(
                enqueue, env=owner_environment, capture_output=True, text=True, check=True
            )
            assert json.loads(first.stdout) == {"request_id": request_id, "applied": True}
            retry: subprocess.CompletedProcess[str] = subprocess.run(
                enqueue, env=owner_environment, capture_output=True, text=True, check=True
            )
            assert json.loads(retry.stdout) == {"request_id": request_id, "applied": False}
            result: dict[str, Any] = wait_for_location_work(api, request_id, process, log)
            assert result["generation"] == 1
            assert result["result_json"]["outcome"] == {
                "kind": "connection",
                "healthy": test_case.expected_connection,
            }
            read: subprocess.CompletedProcess[str] = subprocess.run(
                [executable, "admin", "location-work", request_id],
                env=owner_environment,
                capture_output=True,
                text=True,
                check=True,
            )
            assert json.loads(read.stdout) == result
            process.terminate()
            _ = process.wait(timeout=5)
        stale_id: str = str(uuid4())
        enqueue[enqueue.index(request_id)] = stale_id
        _ = subprocess.run(
            enqueue, env=owner_environment, capture_output=True, text=True, check=True
        )
        with location_process(arguments, log, publications) as process:
            _ = wait_for_generation(api, "preview #1", 2, process, log)
            result = wait_for_location_work(api, stale_id, process, log)
            assert result["generation"] == 1
            assert result["result_json"]["outcome"] == {"kind": "superseded"}
            changed_id: str = str(uuid4())
            enqueue[enqueue.index(stale_id)] = changed_id
            blocked.touch()
            _ = subprocess.run(
                enqueue, env=owner_environment, capture_output=True, text=True, check=True
            )
            _ = wait_for_interpreter(process, child_pid, log)
            _ = (project / "models/stg_orders.sql").write_text(
                TYPE_PROOF_FILES["models/stg_orders.sql"].replace(
                    "CAST(5 AS INTEGER)", "CAST(17 AS INTEGER)"
                )
            )
            blocked.unlink()
            result = wait_for_location_work(api, changed_id, process, log)
            assert result["generation"] == 2
            assert result["result_json"]["outcome"] == {"kind": "superseded"}
            _ = wait_for_generation(api, "preview #1", 3, process, log)
        assert [
            json.loads(line)["generation"] for line in publications.read_text().splitlines()
        ] == [1, 2, 3]
        assert api.bearer not in log.read_text() + publications.read_text()
        assert not (project / ".sqlbuild/control.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        LocationWorkTestCase("person healthy connection", "warehouse.db", True),
        LocationWorkTestCase("person failed connection", "missing/warehouse.db", False),
    ],
    ids=lambda case: case.description,
)
def test_given_person_roles_when_requesting_connection_tests_then_live_scope_and_real_results_hold(
    test_case: LocationWorkTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + f"\n[connections.local]\ndatabase = {json.dumps(test_case.database)}\n"
            + '\n[targets.development]\nschema = "main"\n',
        },
    )
    fixture: dict[str, Any] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/administrator_lifecycle.json"
        ).read_text()
    )
    with snapshot_api(
        tmp_path / "control",
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as api:
        administrator: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        person: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-developer"
        )
        admin: tuple[str, str] = person_login(api, administrator["name"])
        session: tuple[str, str] = person_login(api, person["name"])
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        monkeypatch.setenv("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://must-not-open")
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
        log: Path = tmp_path / "location.log"
        path: str = f"/api/control/locations/{api.location_id}/targets/development/connection-tests"
        with location_process(arguments, log, tmp_path / "publications.jsonl") as process:
            _ = wait_for_generation(api, "development", 1, process, log)
            request: dict[str, Any] = {
                "request_id": str(uuid4()),
                "reason": "Check orders connectivity",
            }
            for operation in [
                partial(person_request, api, path, body=request, session=session),
                partial(person_read, api, path + "/" + request["request_id"], session=session),
            ]:
                with pytest.raises(HTTPError) as denied:
                    operation()
                assert denied.value.code == 403
            for role in fixture["connection_test_roles"][1:3]:
                assert role["allowed"] is False
                assignment: dict[str, Any] = {
                    "principal_id": person["principal_id"],
                    "role": role["role"],
                    "location_id": api.location_id,
                    "target": "development",
                }
                assert person_request(api, "/api/admin/roles", body=assignment, session=admin) == {
                    "applied": True
                }
                for operation in [
                    partial(person_request, api, path, body=request, session=session),
                    partial(person_read, api, path + "/" + request["request_id"], session=session),
                ]:
                    with pytest.raises(HTTPError) as denied:
                        operation()
                    assert denied.value.code == 403
                assert person_request(
                    api, "/api/admin/roles", body=assignment | {"revoke": True}, session=admin
                ) == {"applied": True}
            scopes: dict[str, tuple[str | None, str | None]] = {
                "admin": (None, None),
                "developer": (api.location_id, "development"),
            }
            for role in fixture["connection_test_roles"][3:]:
                assert role["allowed"] is True
                assignment = {
                    "principal_id": person["principal_id"],
                    "role": role["role"],
                    "location_id": scopes[role["role"]][0],
                    "target": scopes[role["role"]][1],
                }
                assert person_request(api, "/api/admin/roles", body=assignment, session=admin) == {
                    "applied": True
                }
                request = {"request_id": str(uuid4()), "reason": "Check orders connectivity"}
                assert person_request(api, path, body=request, session=session) == {"applied": True}
                assert person_request(api, path, body=request, session=session) == {
                    "applied": False
                }
                result: dict[str, Any] = wait_for_location_work(
                    api, request["request_id"], process, log
                )
                assert (
                    person_read(api, path + "/" + request["request_id"], session=session) == result
                )
                assert result["result_json"]["outcome"] == {
                    "kind": "connection",
                    "healthy": test_case.expected_connection,
                }
                assert result["requested_by"] == "principal:" + person["principal_id"]
                assert result["generation"] == 1
                assert person_request(
                    api, "/api/admin/roles", body=assignment | {"revoke": True}, session=admin
                ) == {"applied": True}
            assert person_request(api, "/api/admin/roles", body=assignment, session=admin) == {
                "applied": True
            }
            for other in [
                path.replace("development", "preview"),
                path.replace(api.location_id, str(uuid4())),
            ]:
                with pytest.raises(HTTPError) as denied:
                    person_request(api, other, body=request, session=session)
                assert denied.value.code == 403
                with pytest.raises(HTTPError) as denied:
                    person_read(api, other + "/" + request["request_id"], session=session)
                assert denied.value.code == 403
            assert person_request(
                api, "/api/admin/roles", body=assignment | {"revoke": True}, session=admin
            ) == {"applied": True}
            for operation in [
                partial(person_request, api, path, body=request, session=session),
                partial(person_read, api, path + "/" + request["request_id"], session=session),
            ]:
                with pytest.raises(HTTPError) as denied:
                    operation()
                assert denied.value.code == 403
            verify_connection_test_transport(api, path, request, admin)
        audit: list[dict[str, Any]] = person_read(
            api, "/api/control/audit?limit=100", session=admin
        )
        accepted: list[dict[str, Any]] = list(
            filter(lambda event: event["action"] == "location.work_requested", audit)
        )
        assert {event["granting_role"] for event in accepted} == {"admin", "developer"}
        assert all(event["actor"] == "principal:" + person["principal_id"] for event in accepted)
        assert all(
            event["scope_json"] == {"location_id": api.location_id, "target": "development"}
            for event in accepted
        )
        with sqlite3.connect(tmp_path / "control/.sqlbuild/control.sqlite3") as store:
            assert store.execute("SELECT count(*) FROM sqb_location_work").fetchone() == (2,)
            assert store.execute("SELECT count(*) FROM sqb_runs").fetchone() == (0,)
            assert store.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
        assert api.bearer not in log.read_text()
        assert not (project / ".sqlbuild/control.sqlite3").exists()
