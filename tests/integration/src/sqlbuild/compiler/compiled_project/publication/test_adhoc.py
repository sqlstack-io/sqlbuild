"""People check and launch ad-hoc commands through `sqb server`; workers run exactly what was resolved."""

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
    security_audit_rows,
    wait_for_run,
    worker_statements,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    person_login,
    person_read,
    person_request,
    provision_person,
)

WAREHOUSE_REFUSAL: str = (
    "the requested warehouse is not configured for this target on this location; "
    "no warehouse statement ran"
)


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
            description="an unconfigured warehouse is refused before any statement, "
            "while a published job keeps its authored warehouse",
            person="admin",
            command=["build", "--select", "customers", "--warehouse", "ANALYST_XS"],
            expected_permissions=[["run.adhoc", True], ["run.warehouse", True]],
            expected_status="failed",
            expected_reason=WAREHOUSE_REFUSAL,
            expected_statements=False,
            expected_customers_tables=0,
            # DuckDB has no session warehouse, so the CLI refuses the job's warehouse: the
            # published job reached the CLI instead of being refused by the worker.
            job_settings="settings={'warehouse': 'ANALYST_XS'}, ",
            expected_scheduled_status="failed",
            expected_worker_log="error[C260]: --warehouse ANALYST_XS was given",
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
        }
        | {
            "orchestration/orders.py": (
                "from sqlbuild.orchestration import job, schedule\n"
                'annual_orders = job("annual_orders", select="orders", '
                f'description="Build orders.", {test_case.job_settings}'
                'schedule=schedule(cron="0 0 * * *", timezone="UTC", '
                'starts_at="2000-01-01T00:00:00Z"))\n'
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
        assert scheduled["status"] == test_case.expected_scheduled_status, worker.log.read_text()
        assert scheduled["status_reason"] != WAREHOUSE_REFUSAL, "published jobs are not refused"
        assert test_case.expected_worker_log in worker.log.read_text()
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
