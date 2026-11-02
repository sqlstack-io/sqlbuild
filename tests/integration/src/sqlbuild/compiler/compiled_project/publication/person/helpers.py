"""Exercise owner provisioning and browser transport without bypassing role authorization."""

import json
import os
import subprocess
import sys
from http.cookies import SimpleCookie
from pathlib import Path
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
