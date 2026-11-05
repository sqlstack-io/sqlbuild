"""An installed native executable restores lost administrator access with grant-admin."""

import json
import shutil
import subprocess
from pathlib import Path
from typing import Any

import psycopg
import pytest
from testcontainers.postgres import PostgresContainer

from scripts.native_tests.constants import POSTGRES_IMAGE
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    InstalledRecoveryTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.api.helpers import (
    login,
    request,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    administer,
    serve,
    successful,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.helpers import (
    postgres_recovery_state,
    verify_recovery,
)


@pytest.mark.parametrize(
    "test_case",
    [
        InstalledRecoveryTestCase(
            description="installed native server recovers authority without rebootstrap",
            executable="sqb",
            expected_rejection="cannot remove or disable the final installation administrator",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_installed_server_when_admin_is_granted_then_authority_is_restored_without_rebootstrap(
    test_case: InstalledRecoveryTestCase,
) -> None:
    binary: str | None = shutil.which(test_case.executable)
    assert binary is not None
    assert test_case.expected_rejection in verify_recovery(Path(binary))


@pytest.mark.parametrize(
    "test_case",
    [
        InstalledRecoveryTestCase(
            description="installed PostgreSQL grant-admin restores authority while replicas run",
            executable="sqb",
            expected_rejection="cannot remove or disable the final installation administrator",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_postgres_replicas_when_grant_admin_runs_then_only_the_granted_persons_sessions_end(
    test_case: InstalledRecoveryTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://must-not-open")
    binary: str | None = shutil.which(test_case.executable)
    assert binary is not None
    installation: tuple[Path, Path] = Path(binary), tmp_path
    replica: Path = tmp_path / "replica"
    replica.mkdir()
    fixture: dict[str, Any] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/administrator_lifecycle.json"
        ).read_text()
    )
    state_sql: str = (
        "SELECT (SELECT COUNT(*) FROM sqb_principals WHERE disabled_at IS NOT NULL),"
        "(SELECT COUNT(*) FROM sqb_role_assignments WHERE revoked_at IS NULL),"
        "(SELECT COUNT(*) FROM sqb_sessions WHERE revoked_at IS NULL),"
        "(SELECT COUNT(*) FROM sqb_security_audit WHERE action='admin.granted')"
    )
    with PostgresContainer(POSTGRES_IMAGE, driver=None) as postgres:
        url: str = postgres.get_connection_url()
        extra: dict[str, str] = {"SQLBUILD_CONTROL_DATABASE_URL": url}
        people: list[dict[str, Any]] = []
        for command, name in [
            ("create-user", fixture["first_person"]),
            ("create-principal", fixture["second_person"]),
        ]:
            person: dict[str, Any] = successful(
                administer(installation, [command, name], extra_environment=extra)
            )
            successful(
                administer(
                    installation,
                    ["set-password", person["principal_id"]],
                    fixture["password"],
                    extra_environment=extra,
                )
            )
            people.append(person)
        principal: str = people[0]["principal_id"]
        recovery: list[str] = ["grant-admin", principal]
        with (
            serve(installation, extra_environment=extra) as first,
            serve((Path(binary), replica), extra_environment=extra) as second,
        ):
            previous: tuple[str, str] = login(first, people[0]["name"], fixture["password"])
            other: tuple[str, str] = login(second, people[1]["name"], fixture["password"])
            for endpoint in [first, second]:
                assert request(endpoint + "/api/auth/session", session=previous)[0] == 200
                assert request(endpoint + "/api/auth/session", session=other)[0] == 200
            with psycopg.connect(url) as control:
                control.execute(
                    "UPDATE sqb_principals SET disabled_at=now() WHERE principal_id=%s",
                    (principal,),
                )
                control.execute(
                    "UPDATE sqb_role_assignments SET revoked_at=now(),active_key=NULL "
                    "WHERE principal_id=%s",
                    (principal,),
                )
                assert control.execute(state_sql).fetchone() == (1, 0, 2, 0)
                retained_before: dict[str, str] = postgres_recovery_state(control)
            assert successful(administer(installation, recovery, extra_environment=extra)) == {
                "principal_id": principal,
                "role": "admin",
            }
            with psycopg.connect(url) as control:
                after: Any = control.execute(state_sql).fetchone()
                assert after is not None
                assert after == (0, 1, 1, 1)
                retained_after: dict[str, str] = postgres_recovery_state(control)
                assert retained_after["sqb_installation"] == retained_before["sqb_installation"]
                audit: Any = control.execute(
                    "SELECT scope_json FROM sqb_security_audit WHERE action='admin.granted'"
                ).fetchone()
                assert audit == ({"principal_id": principal},)
            current: tuple[str, str] = login(first, people[0]["name"], fixture["password"])
            for endpoint in [first, second]:
                assert request(endpoint + "/api/auth/session", session=previous)[0] == 401
                assert request(endpoint + "/api/auth/session", session=other)[0] == 200
                assert (
                    request(
                        endpoint + "/api/admin/people/" + principal,
                        b'"revoke"',
                        (current[0], "wrong-csrf"),
                    )[0]
                    == 401
                )
                assert request(endpoint + "/api/auth/session", session=current)[0] == 200
                status, _, body = request(
                    endpoint + "/api/admin/people/" + principal, b'"revoke"', current
                )
                assert status == 403
                assert test_case.expected_rejection in body["error"]
        rejected: subprocess.CompletedProcess[str] = administer(
            installation, ["create-user", "replacement-admin"], extra_environment=extra
        )
        assert rejected.returncode != 0 and not rejected.stdout
        assert "bootstrap is closed" in rejected.stderr
