"""Bootstrap, authenticate, lose authority, recover offline and authenticate again."""

import hashlib
import json
import sqlite3
import subprocess
import sys
from contextlib import closing
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.api.helpers import (
    login,
    request,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    administer,
    serve,
    successful,
)


def logical_table_state(connection: Any, tables: list[str]) -> dict[str, str]:
    """Hash complete logical rows and multiplicity without exposing stored credentials."""
    state: dict[str, str] = {}
    for table in tables:
        rows: list[str] = [
            json.dumps(row, sort_keys=True, default=str)
            for row in connection.execute(f'SELECT * FROM "{table}"').fetchall()
        ]
        state[table] = hashlib.sha256(json.dumps(sorted(rows)).encode()).hexdigest()
    connection.commit()
    return state


def postgres_recovery_state(connection: Any) -> dict[str, str]:
    """Compare every retained security row without logging credential hashes."""
    tables: tuple[str, ...] = (
        "sqb_principals",
        "sqb_role_assignments",
        "sqb_password_credentials",
        "sqb_password_sessions",
        "sqb_external_sessions",
        "sqb_security_audit",
        "sqb_admin_initialization",
        "sqb_installation",
    )
    return {
        table: hashlib.sha256(
            json.dumps(
                connection.execute(
                    f"SELECT to_jsonb(state) FROM {table} AS state ORDER BY to_jsonb(state)::text"
                ).fetchall(),
                sort_keys=True,
            ).encode()
        ).hexdigest()
        for table in tables
    }


def verify_recovery(binary: Path) -> str:
    """Exercise the wheel's native executable using only the Python standard library."""
    binary = binary.resolve(strict=True)
    print("Starting installed-artifact administrator recovery acceptance...", flush=True)
    with TemporaryDirectory(prefix="sqlbuild-wheel-recovery-") as temporary:
        root: Path = Path(temporary)
        installation: tuple[Path, Path] = binary, root
        person: dict[str, Any] = successful(
            administer(installation, ["create-user", "orders-admin"])
        )
        principal: str = person["principal_id"]
        password = "orders-recovery-password-2026"
        successful(administer(installation, ["set-password", principal], password))
        recovery: list[str] = [
            "recover-user",
            principal,
            "--reason",
            "restore local administrator access",
        ]
        with serve(installation) as url:
            previous: tuple[str, str] = login(url, person["name"], password)
            rejected: subprocess.CompletedProcess[str] = administer(installation, recovery)
            assert rejected.returncode != 0 and not rejected.stdout
            assert "exclusive access" in rejected.stderr
        print("Online recovery rejected; server killed and reaped.", flush=True)
        database: Path = root / ".sqlbuild/control.sqlite3"
        with closing(sqlite3.connect(database)) as control:
            control.execute("UPDATE sqb_principals SET disabled_at='2026-01-01T00:00:00Z'")
            control.commit()
        rejected = administer(installation, ["create-user", "replacement-admin"])
        assert rejected.returncode != 0 and not rejected.stdout
        assert "bootstrap is closed" in rejected.stderr
        result: dict[str, Any] = successful(administer(installation, recovery))
        assert result == {"principal_id": principal, "role": "admin", "recovered": True}
        with closing(sqlite3.connect(database)) as control:
            assert control.execute("SELECT COUNT(*) FROM sqb_admin_initialization").fetchone() == (
                1,
            )
            assert control.execute(
                "SELECT COUNT(*) FROM sqb_security_audit WHERE action='admin.offline_recovered'"
            ).fetchone() == (1,)
            assert control.execute(
                "SELECT COUNT(*) FROM sqb_password_sessions WHERE revoked_at IS NULL"
            ).fetchone() == (0,)
        print(
            "Offline recovery committed one audit; bootstrap stayed closed; sessions revoked.",
            flush=True,
        )
        with serve(installation) as url:
            assert request(f"{url}/api/auth/session", session=previous)[0] == 401
            session: tuple[str, str] = login(url, person["name"], password)
            assert request(f"{url}/api/auth/session", session=session)[0] == 200
            status, _, body = request(f"{url}/api/admin/people/{principal}", b'"revoke"', session)
            assert status == 403
            assert (
                "cannot remove or disable the final usable installation administrator"
                in body["error"]
            )
            rejected = administer(installation, ["create-user", "replacement-admin"])
            assert rejected.returncode != 0 and "bootstrap is closed" in rejected.stderr
    print("Installed-artifact administrator recovery acceptance passed.", flush=True)
    return body["error"]


if __name__ == "__main__":
    verify_recovery(Path(sys.argv[1]))
