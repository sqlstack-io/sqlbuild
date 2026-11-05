"""Denial metadata identifies durable decisions without granting access to their scope."""

import json
import sqlite3
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from uuid import UUID, uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DenialMetadataTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    snapshot_api,
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
        DenialMetadataTestCase(
            description="durable ID without scope disclosure", expected_denial_count=6
        )
    ],
    ids=lambda case: case.description,
)
def test_given_denied_browser_requests_when_returned_then_only_durable_decision_metadata_is_exposed(
    test_case: DenialMetadataTestCase,
    tmp_path: Path,
) -> None:
    with snapshot_api(
        tmp_path / "control",
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as api:
        administrator: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        person: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-reader"
        )
        admin_session: tuple[str, str] = person_login(api, administrator["name"])
        session: tuple[str, str] = person_login(api, person["name"])
        catalog: str = (
            f"/api/control/locations/{api.location_id}/targets/development/catalog?schema=main"
        )
        unlisted: str = catalog.replace(api.location_id, str(uuid4()))
        decisions: dict[str, str] = {}
        for path in [catalog, unlisted, "/api/control/audit"]:
            with pytest.raises(HTTPError) as rejected:
                person_read(api, path, session=session)
            assert rejected.value.code == 403
            assert rejected.value.headers["Cache-Control"] == "no-store"
            body: dict[str, Any] = json.load(rejected.value)
            assert set(body) == {"error", "decision_id"}
            decision: str = str(UUID(body["decision_id"]))
            assert body["error"] == (
                "missing permission in this scope; all requested nodes denied; decision " + decision
            )
            decisions[decision] = "authorization.denied"
        request: dict[str, Any] = {
            "run_id": str(uuid4()),
            "location_id": api.location_id,
            "target": "development",
            "job_key": "unlisted_orders_job",
            "reason": "Refresh order totals",
            "priority": 0,
        }
        for credentials, expected_status, action in [
            (session, 403, "authorization.denied"),
            ((session[0], "wrong-csrf"), 401, "authentication.denied"),
            (("", ""), 401, "authentication.denied"),
        ]:
            with pytest.raises(HTTPError) as rejected:
                person_request(api, "/api/control/runs", body=request, session=credentials)
            assert rejected.value.code == expected_status
            assert rejected.value.headers["Cache-Control"] == "no-store"
            body = json.load(rejected.value)
            assert set(body) == {"error", "decision_id"}
            decision = str(UUID(body["decision_id"]))
            assert (
                body["error"]
                == {
                    "authorization.denied": (
                        "missing permission in this scope; all requested nodes denied; decision "
                        + decision
                    ),
                    "authentication.denied": "authentication rejected",
                }[action]
            )
            assert decision not in decisions
            decisions[decision] = action
        assignment: dict[str, Any] = {
            "principal_id": person["principal_id"],
            "role": "viewer",
            "location_id": api.location_id,
            "target": "development",
        }
        assert person_request(api, "/api/admin/roles", body=assignment, session=admin_session) == {
            "applied": True
        }
        assert person_read(api, catalog, session=session) == {"records": [], "next_after": None}
        with pytest.raises(HTTPError) as rejected:
            person_read(api, unlisted, session=session)
        assert rejected.value.code == 403
        assert person_request(
            api, "/api/admin/roles", body=assignment | {"revoke": True}, session=admin_session
        ) == {"applied": True}
        with pytest.raises(HTTPError) as rejected:
            person_read(api, catalog, session=session)
        assert rejected.value.code == 403
        events: list[dict[str, Any]] = person_read(
            api, "/api/control/audit?limit=100", session=admin_session
        )
        by_id: dict[str, dict[str, Any]] = {event["event_id"]: event for event in events}
        assert len(decisions) == test_case.expected_denial_count
        recorded: dict[str, str] = {
            "authorization.denied": "authorization.denied",
            "authentication.denied": "not audited",
        }
        assert {
            decision: by_id.get(decision, {"action": "not audited"})["action"]
            for decision in decisions
        } == {decision: recorded[action] for decision, action in decisions.items()}, (
            "only authenticated denials reach the durable audit"
        )
        with sqlite3.connect(tmp_path / "control/.sqlbuild/control.sqlite3") as store:
            assert store.execute("SELECT count(*) FROM sqb_runs").fetchone() == (0,)
            assert store.execute("SELECT count(*) FROM sqb_write_claims").fetchone() == (0,)
