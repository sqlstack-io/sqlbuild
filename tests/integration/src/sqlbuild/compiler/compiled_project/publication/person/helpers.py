"""Exercise owner provisioning and browser transport without bypassing role authorization."""

import json
import os
import subprocess
import sys
import time
from http.cookies import SimpleCookie
from pathlib import Path
from subprocess import Popen
from typing import Any
from urllib.error import HTTPError
from urllib.request import Request, urlopen

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SnapshotApi,
)


def provision_person(control: Path, command: str, name: str) -> dict[str, Any]:
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    executable: str = str(Path(sys.executable).with_name("sqb"))
    created: subprocess.CompletedProcess[str] = subprocess.run(
        [executable, "admin", command, name],
        cwd=control,
        env=environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=15,
    )
    person: dict[str, Any] = json.loads(created.stdout)
    _ = subprocess.run(
        [executable, "admin", "set-password", person["principal_id"]],
        input="orders-person-password-2026\n",
        cwd=control,
        env=environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=15,
    )
    return person


def person_headers(session: tuple[str, str]) -> dict[str, str]:
    return {
        "Host": "control.example.com",
        "Origin": "https://control.example.com",
        "Content-Type": "application/json",
        "Cookie": session[0],
        "x-sqb-csrf": session[1],
    }


def person_read(api: SnapshotApi, path: str, *, session: tuple[str, str]) -> Any:
    with urlopen(Request(api.url + path, headers=person_headers(session)), timeout=5) as response:
        return json.load(response)


def wait_for_person_catalog(
    api: SnapshotApi,
    session: tuple[str, str],
    process: Popen[bytes],
    log: Path,
    *,
    expected_definition_hash: str,
) -> None:
    """Wait for an actual worker-built relation through the scoped browser API, not store SQL."""
    path: str = f"/api/control/locations/{api.location_id}/targets/development/catalog?schema=main"
    deadline: float = time.monotonic() + 45
    page: dict[str, Any] = person_read(api, path, session=session)
    records: dict[str, Any] = {row["record"]["name"]: row for row in page["records"]}
    while "orders" not in records:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, (page, log.read_text())
        time.sleep(0.05)
        page = person_read(api, path, session=session)
        records = {row["record"]["name"]: row for row in page["records"]}
    assert records["orders"]["event_id"] and records["orders"]["observed_at"]
    assert records["orders"]["record"] == {
        "state_database": None,
        "state_schema": "main",
        "name": "orders",
        "relation_type": "BASE TABLE",
        "columns": [{"name": "amount", "data_type": "INTEGER"}],
    }
    assert page["next_after"] is None
    assert person_read(api, path + "&database=unlisted", session=session) == {
        "records": [],
        "next_after": None,
    }
    fingerprint_path: str = (
        f"/api/control/locations/{api.location_id}/targets/development/fingerprint"
        "?node_type=model&node_name=orders&schema=main&name=orders"
    )
    evidence: dict[str, Any] | None = person_read(api, fingerprint_path, session=session)
    while evidence is None or evidence["definition_hash"] != expected_definition_hash:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, (evidence, log.read_text())
        time.sleep(0.05)
        evidence = person_read(api, fingerprint_path, session=session)
    assert set(evidence) == {
        "event_id",
        "observed_at",
        "source_event_id",
        "run_id",
        "definition_hash",
        "version_hash",
        "schema_fingerprint",
    }
    assert all(evidence.values()), evidence
    assert person_read(api, fingerprint_path + "&database=unlisted", session=session) is None
    status: dict[str, Any] = person_read(
        api,
        f"/api/control/locations/{api.location_id}/targets/development/node-status?node_key=model:orders",
        session=session,
    )
    assert status["physical"]["status"] == "observed"
    assert status["physical"]["observation"] == records["orders"]
    assert status["definition"]["status"] == "matched", status
    assert status["build"] == evidence
    assert status["source_data"] == {
        "status": "unknown",
        "reason": "consumed-input provenance is not recorded",
    }
    statistics_path: str = (
        f"/api/control/locations/{api.location_id}/targets/development/statistics"
        "?schema=main&name=orders"
    )
    statistics: dict[str, Any] = person_read(api, statistics_path, session=session)
    while not statistics["records"]:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, (statistics, log.read_text())
        time.sleep(0.05)
        statistics = person_read(api, statistics_path, session=session)
    sample: dict[str, Any] = statistics["records"][0]
    assert sample["event_id"] and sample["record"]["observed_at"]
    assert sample["record"] | {"observed_at": None} == {
        "state_database": None,
        "state_schema": "main",
        "name": "orders",
        "observed_at": None,
        "row_count": 1,
        "detail": None,
        "byte_count": None,
        "byte_count_basis": None,
    }
    assert statistics["next_after"] is None
    assert person_read(api, statistics_path + "&database=unlisted", session=session) == {
        "records": [],
        "next_after": None,
    }


def verify_security_audit(
    api: SnapshotApi, administrator: tuple[str, str], ordinary: tuple[str, str]
) -> None:
    audit: list[dict[str, Any]] = person_read(
        api, "/api/control/audit?limit=3", session=administrator
    )
    assert len(audit) == 3
    cursor: int = audit[-1]["storage_id"]
    following: list[dict[str, Any]] = person_read(
        api, f"/api/control/audit?after={cursor}&limit=3", session=administrator
    )
    assert len(following) == 3
    assert all(event["storage_id"] > cursor for event in following)
    assert len({event["event_id"] for event in audit + following}) == 6
    assert all(event["actor"] and event["granting_role"] for event in audit + following)
    with pytest.raises(HTTPError) as audit_rejected:
        person_read(api, "/api/control/audit", session=ordinary)
    assert audit_rejected.value.code == 403


def person_request(
    api: SnapshotApi, path: str, *, body: Any, session: tuple[str, str] = ("", "")
) -> Any:
    request: Request = Request(
        api.url + path,
        data=json.dumps(body).encode(),
        headers=person_headers(session),
    )
    with urlopen(request, timeout=5) as response:
        return json.load(response)


def person_login(api: SnapshotApi, name: str) -> tuple[str, str]:
    request: Request = Request(
        api.url + "/api/auth/password/login",
        data=json.dumps({"username": name, "password": "orders-person-password-2026"}).encode(),
        headers={
            "Host": "control.example.com",
            "Origin": "https://control.example.com",
            "Content-Type": "application/json",
        },
    )
    with urlopen(request, timeout=5) as response:
        cookies: SimpleCookie[str] = SimpleCookie("\n".join(response.headers.get_all("Set-Cookie")))
        cookie: str = "__Host-sqb-session=" + cookies["__Host-sqb-session"].value
        return cookie, json.load(response)["csrf_token"]


def verify_connection_test_transport(
    api: SnapshotApi, path: str, body: dict[str, Any], administrator: tuple[str, str]
) -> None:
    """Neither machine credentials, missing CSRF nor caller-supplied work cross the person API."""
    for credentials in [("", ""), (administrator[0], "wrong-csrf")]:
        with pytest.raises(HTTPError) as denied:
            person_request(api, path, body=body, session=credentials)
        assert denied.value.code == 401
    with pytest.raises(HTTPError) as denied:
        person_read(api, path + "/" + body["request_id"], session=(administrator[0], "wrong-csrf"))
    assert denied.value.code == 403
    with pytest.raises(HTTPError) as denied:
        urlopen(
            Request(
                api.url + path,
                data=json.dumps(body).encode(),
                headers=person_headers(("", "")) | {"Authorization": "Bearer " + api.bearer},
            ),
            timeout=5,
        )
    assert denied.value.code == 401
    for extra in [{"actor": "forged"}, {"work": {"kind": "evaluate_sensor"}}]:
        with pytest.raises(HTTPError) as denied:
            person_request(api, path, body=body | extra, session=administrator)
        assert denied.value.code == 422
