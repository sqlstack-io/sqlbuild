"""An installed CLI inspects actual published SQL through a bounded live person session."""

import json
import sqlite3
import subprocess
import sys
from contextlib import closing
from functools import partial
from pathlib import Path
from typing import Any

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    scheduled_worker,
    worker_result,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.login.helpers import (
    browser,
    gateway,
    pending_login,
    request,
    sign_in,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    provision_person,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    administer,
    environment,
)


@pytest.mark.skipif(sys.platform != "linux", reason="Headless Linux credential-store acceptance")
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            description="saved person session, actual SQL, exact key and live revocation",
            model_sql="SELECT 23 AS amount, '" + "orders " * 400 + "' AS description",
            warehouse_query="SELECT amount FROM orders",
            expected_status="success",
            expected_rows=[(23,)],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_published_orders_when_cli_reads_sql_then_only_live_scoped_authority_discloses_sources(
    test_case: WorkerExecutionTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    home: Path = tmp_path / "home"
    home.mkdir()
    empty: Path = tmp_path / "empty"
    empty.mkdir()
    client_environment: dict[str, str] = environment({"HOME": str(home)})
    client_environment.pop("DBUS_SESSION_BUS_ADDRESS", None)
    client_environment.pop("XDG_RUNTIME_DIR", None)
    binary: str = str(Path(sys.executable).with_name("sqb"))
    invoke: partial[subprocess.CompletedProcess[Any]] = partial(
        subprocess.run,
        cwd=empty,
        env=client_environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=20,
    )
    with (
        gateway(tmp_path) as (origin, certificate, transport),
        scheduled_worker(
            tmp_path,
            monkeypatch,
            test_case,
            server_arguments=("--password-origin", origin),
        ) as worker,
    ):
        transport["backend"] = worker.api.url.removeprefix("http://")
        _ = worker_result(worker, test_case)
        admin: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        person: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-person"
        )
        client, _ = browser(origin, certificate)
        csrf: str = sign_in(
            client, origin, "password", person["name"], "orders-person-password-2026"
        )
        common: list[str] = ["--server", origin, "--ca-cert", str(certificate)]
        command: list[str] = [
            binary,
            "control",
            "sql",
            "model:orders",
            *common,
            "--location-id",
            worker.api.location_id,
            "--target",
            "development",
        ]
        sync: list[str] = [
            binary,
            "control",
            "sync",
            *common,
            "--location-id",
            worker.api.location_id,
            "--target",
            "development",
        ]
        for misused in ([*command[:3], *command[4:]], [*sync[:3], "model:orders", *sync[3:]]):
            usage: subprocess.CompletedProcess[str] = invoke(misused)
            assert (usage.returncode, usage.stdout, usage.stderr[:20]) == (
                2,
                "",
                "sqb control: error: ",
            )
        absent: subprocess.CompletedProcess[str] = invoke(command)
        assert absent.returncode != 0 and not absent.stdout and "not signed in" in absent.stderr
        with pending_login(
            [binary, "login", *common, "--no-browser"],
            client_environment,
            empty,
            tmp_path / "cli-login.log",
        ) as (process, url):
            assert (
                request(
                    client,
                    origin + "/api/auth/cli/approve",
                    {"browser_code": url.split("#", 1)[1], "approve": True},
                    csrf=csrf,
                )[0]
                == 204
            )
            stdout, _ = process.communicate(timeout=20)
            assert process.returncode == 0 and not stdout
        denied: subprocess.CompletedProcess[str] = invoke(command)
        assert (
            denied.returncode != 0 and not denied.stdout and "HTTP 403; decision " in denied.stderr
        )
        refused: subprocess.CompletedProcess[str] = invoke(sync)
        assert (
            refused.returncode != 0
            and not refused.stdout
            and "Warehouse sync request rejected (HTTP 403; decision " in refused.stderr
        )
        administrator, _ = browser(origin, certificate)
        admin_csrf: str = sign_in(
            administrator, origin, "password", admin["name"], "orders-person-password-2026"
        )
        assignment: dict[str, Any] = {
            "principal_id": person["principal_id"],
            "role": "developer",
            "location_id": worker.api.location_id,
            "target": "development",
        }
        assert (
            request(administrator, origin + "/api/admin/roles", assignment, csrf=admin_csrf)[0]
            == 200
        )
        requested: subprocess.CompletedProcess[str] = invoke(sync)
        assert requested.returncode == 0, requested.stderr
        assert json.loads(requested.stdout) == {"applied": True}
        with closing(sqlite3.connect(tmp_path / "control/.sqlbuild/control.sqlite3")) as store:
            queued: list[tuple[str]] = store.execute(
                "SELECT requested_by FROM sqb_location_work "
                "WHERE json_extract(request_json,'$.work.kind')='sync_warehouse'"
            ).fetchall()
        assert queued == [("principal:" + person["principal_id"],)]
        inspected: subprocess.CompletedProcess[str] = invoke(command)
        assert inspected.returncode == 0 and "Published SQL read successfully" in inspected.stderr
        sql: dict[str, Any] = json.loads(inspected.stdout)
        assert set(sql) == {
            "snapshot_id",
            "node_key",
            "raw_sql",
            "compiled_sql",
            "expression_ranges",
        }
        compiled: bytes = sql["compiled_sql"].encode()
        assert {
            column: compiled[start:end].decode()
            for column, (start, end) in sql["expression_ranges"].items()
        } == {
            "amount": "23 AS amount",
            "description": "'" + "orders " * 400 + "' AS description",
        }
        assert (
            sql["snapshot_id"]
            == current_snapshot(worker.api, "development")["record"]["snapshot_id"]
        )
        assert sql["node_key"] == "model:orders"
        assert sql["raw_sql"] == (worker.project / "models/orders.sql").read_text()
        with duckdb.connect() as verification:
            assert verification.sql(sql["compiled_sql"]).fetchall() == [
                (*row, "orders " * 400) for row in test_case.expected_rows
            ]
        assert 4097 <= len(inspected.stdout.encode()) < 8192
        with Path("/dev/full").open("w") as unavailable:
            undelivered: subprocess.CompletedProcess[str] = invoke(
                command,
                capture_output=False,
                stdout=unavailable,
                stderr=subprocess.PIPE,
            )
        assert undelivered.returncode != 0 and "SQL inspection failed" in undelivered.stderr
        assert "Published SQL read successfully" not in undelivered.stderr
        for node in ["model:orders+", "model:orders?query=1/#café"]:
            missing: subprocess.CompletedProcess[str] = invoke(command[:3] + [node] + command[4:])
            assert missing.returncode == 0 and json.loads(missing.stdout) is None
        assert (
            request(
                administrator,
                origin + "/api/admin/roles",
                assignment | {"revoke": True},
                csrf=admin_csrf,
            )[0]
            == 200
        )
        revoked: subprocess.CompletedProcess[str] = invoke(command)
        assert (
            revoked.returncode != 0
            and not revoked.stdout
            and "HTTP 403; decision " in revoked.stderr
        )
        assert invoke(sync).returncode != 0
        changed: subprocess.CompletedProcess[str] = administer(
            (Path(binary), tmp_path / "control"),
            ["set-password", person["principal_id"]],
            "orders-replacement-password-2026",
        )
        assert changed.returncode == 0
        expired: subprocess.CompletedProcess[str] = invoke(command)
        assert expired.returncode != 0 and not expired.stdout and "revoked" in expired.stderr
        logout: subprocess.CompletedProcess[str] = invoke([binary, "logout", *common])
        assert logout.returncode == 0 and not logout.stdout
        signed_out: subprocess.CompletedProcess[str] = invoke(command)
        assert (
            signed_out.returncode != 0
            and not signed_out.stdout
            and "not signed in" in signed_out.stderr
        )
        assert not list(empty.iterdir())
