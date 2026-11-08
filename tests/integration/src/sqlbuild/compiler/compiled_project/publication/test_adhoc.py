"""People check and launch ad-hoc commands through `sqb server`; workers run exactly what was resolved."""

import json
import sqlite3
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    AdhocRunTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    duckdb_rows,
    scheduled_worker,
    wait_for_run,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_read,
    person_request,
    provision_person,
)

WAREHOUSE_REFUSAL: str = (
    "warehouse ANALYST_XS is not configured for target development on this location; "
    "no warehouse statement ran"
)


def worker_statements(control_store: Path, run_id: str) -> int:
    """Statements the run's worker invocations recorded, as the run's statement read selects them."""
    with sqlite3.connect(control_store) as control:
        invocations: list[str] = [
            invocation
            for (invocation, context) in control.execute(
                "SELECT invocation_id, external_context FROM invocations "
                "WHERE invocation_id LIKE 'worker/%' AND external_context IS NOT NULL"
            ).fetchall()
            if json.loads(context).get("run_id") == run_id
        ]
        assert invocations, f"run {run_id} has no worker invocation"
        return sum(
            control.execute(
                "SELECT count(*) FROM statements WHERE invocation_id=? OR invocation_id LIKE ?",
                (invocation, invocation + "/%"),
            ).fetchone()[0]
            for invocation in invocations
        )


def security_audit_rows(control_store: Path) -> int:
    with sqlite3.connect(control_store) as control:
        return control.execute("SELECT count(*) FROM sqb_security_audit").fetchone()[0]


@pytest.mark.parametrize(
    "test_case",
    [
        AdhocRunTestCase(
            description="a developer builds exactly the resolved selection",
            person="developer",
            command=["build", "--select", "customers"],
            expected_permissions=[["run.adhoc", True]],
            expected_status="success",
            expected_reason="worker executor completed and closed its connections",
            expected_statements=True,
            expected_customers_tables=1,
        ),
        AdhocRunTestCase(
            description="an unconfigured warehouse is refused before any statement, with its reason",
            person="admin",
            command=["build", "--select", "customers", "--warehouse", "ANALYST_XS"],
            expected_permissions=[["run.adhoc", True], ["run.warehouse", True]],
            expected_status="failed",
            expected_reason=WAREHOUSE_REFUSAL,
            expected_statements=False,
            expected_customers_tables=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_ad_hoc_command_when_checked_and_launched_then_the_worker_runs_the_resolved_selection(
    test_case: AdhocRunTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    worker_case: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 5 AS amount",
        warehouse_query="SELECT amount FROM orders",
        expected_status="success",
        expected_rows=[(5,)],
        files={
            "models/customers.sql": (
                "MODEL (description 'Customers.', materialized table);\nSELECT 1 AS id\n"
            )
        },
    )
    with scheduled_worker(
        tmp_path,
        monkeypatch,
        worker_case,
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as worker:
        scheduled: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders"
        )
        assert scheduled["status"] == "success", worker.log.read_text()
        admin: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        developer: dict[str, Any] = provision_person(
            tmp_path / "control", "create-principal", "orders-developer"
        )
        sessions: dict[str, tuple[str, str]] = {
            "admin": person_login(worker.api, admin["name"]),
            "developer": person_login(worker.api, developer["name"]),
        }
        assert person_request(
            worker.api,
            "/api/admin/roles",
            body={
                "principal_id": developer["principal_id"],
                "role": "developer",
                "location_id": worker.api.location_id,
                "target": "development",
            },
            session=sessions["admin"],
        ) == {"applied": True}
        session: tuple[str, str] = sessions[test_case.person]
        scope: dict[str, Any] = {"location_id": worker.api.location_id, "target": "development"}
        audited: int = security_audit_rows(worker.control_store)
        check: dict[str, Any] = person_request(
            worker.api,
            "/api/control/runs/adhoc/check",
            body=scope | {"command": test_case.command, "priority": 0},
            session=session,
        )
        assert (check["selection"], check["write_set"], check["error"], check["launchable"]) == (
            ["model:customers"],
            ["model:customers"],
            None,
            True,
        )
        assert [
            [permission["action"], permission["allowed"]] for permission in check["permissions"]
        ] == test_case.expected_permissions
        assert security_audit_rows(worker.control_store) == audited, "a dry run records nothing"
        with pytest.raises(HTTPError) as refused:
            person_request(
                worker.api,
                "/api/control/runs/adhoc",
                body=scope
                | {
                    "run_id": str(uuid4()),
                    "command": ["build", "--vars", "{}"],
                    "reason": "Try variables",
                    "priority": 0,
                },
                session=session,
            )
        assert refused.value.code == 400
        assert "--vars is not available for ad-hoc runs" in refused.value.read().decode()
        run_id: str = str(uuid4())
        launch: dict[str, Any] = scope | {
            "run_id": run_id,
            "command": test_case.command,
            "reason": "Refresh customers",
            "priority": 0,
        }
        for expected_applied in [True, False]:
            assert person_request(
                worker.api, "/api/control/runs/adhoc", body=launch, session=session
            ) == {"applied": expected_applied}
        completed: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, run_id=run_id
        )
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        assert completed["selection_json"] == ["model:customers"]
        listed: list[dict[str, Any]] = person_read(
            worker.api,
            f"/api/control/locations/{worker.api.location_id}/targets/development/runs",
            session=session,
        )
        assert {run["run_id"]: run["status_reason"] for run in listed}[run_id] == (
            test_case.expected_reason
        ), "the person sees why the run ended"
        assert (worker_statements(worker.control_store, run_id) > 0) == (
            test_case.expected_statements
        )
        assert duckdb_rows(
            worker.project / "warehouse.duckdb",
            "SELECT count(*) FROM information_schema.tables WHERE table_name='customers'",
        ) == [(test_case.expected_customers_tables,)]
