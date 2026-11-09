"""People check and launch ad-hoc commands through `sqb server`; workers run exactly what was resolved."""

import json
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from uuid import uuid4

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    AdhocRunTestCase,
    AdhocVerbTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    duckdb_rows,
    scheduled_worker,
    security_audit_rows,
    sqlite_rows,
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


SEED: list[str] = ["seed", "--select", "regions"]
BUILD: list[str] = ["build", "--select", "customers"]
CUSTOMERS_SQL: str = (
    "MODEL (description 'Customers.', materialized table, "
    "columns (id (type INTEGER, audits [not_null])));\n"
    'SELECT count(*)::INTEGER AS id FROM __seed("regions")\n'
)
SEEDS_YML: str = (
    "seeds:\n  - name: regions\n    description: Regions.\n    columns:\n"
    "      - name: region\n        type: {region_type}\n"
    "      - name: label\n        type: VARCHAR\n"
)
SOURCES_YML: str = (
    "sources:\n  - name: raw_orders\n    description: Raw orders\n"
    "    schema: main\n    table: {table}\n    freshness:\n"
    "      column: loaded_at\n    columns:\n"
    "      - name: loaded_at\n        type: TIMESTAMP\n"
)
CUSTOMERS_TEST: str = (
    'TEST (name "customers_have_ids");\n'
    "WITH __seed__regions AS (SELECT 'EU' AS region, 'Europe' AS label),\n"
    "__expected__customers AS (SELECT {expected} AS id)\n"
)
# Project Python is imported during discovery after the SQL and YAML were read, so once the test
# writes the flag, the final run's worker rewrites its checkout after verification, as a deploy
# landing mid-run would.
REWRITE_CHECKOUT: str = (
    "import json, pathlib\n"
    "root = pathlib.Path(__file__).resolve().parent.parent\n"
    "flag = root / 'rewrite.json'\n"
    "if flag.exists():\n"
    "    for name, text in json.loads(flag.read_text()).items():\n"
    "        (root / name).write_text(text)\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        AdhocVerbTestCase(
            description="seed loads only the selected seeds, as verified",
            prerequisites=[],
            command=SEED,
            expected_selection=["seed:regions"],
            expected_run_nodes=[("seed:regions", "node", "success")],
            rewrites={"seeds/seeds.yml": SEEDS_YML.format(region_type="INTEGER")},
        ),
        AdhocVerbTestCase(
            description="audit reports each verified audit and its node's worst result",
            prerequisites=[SEED, BUILD],
            command=["audit", "--select", "customers"],
            expected_selection=["model:customers"],
            expected_run_nodes=[
                ("audit:not_null:model:customers:id", "check", "success"),
                ("model:customers", "node", "success"),
            ],
            rewrites={"models/customers.sql": CUSTOMERS_SQL.replace("not_null", "unique_x")},
        ),
        AdhocVerbTestCase(
            description="test reports each verified SQL test and the model it tests",
            prerequisites=[],
            command=["test", "--select", "customers"],
            expected_selection=["model:customers"],
            expected_run_nodes=[
                ("model:customers", "node", "success"),
                ("sql_test:customers_have_ids", "check", "success"),
            ],
            rewrites={"tests/unit/test_customers.sql": CUSTOMERS_TEST.format(expected=2)},
        ),
        AdhocVerbTestCase(
            description="freshness observes the verified sources upstream of the selection",
            prerequisites=[],
            command=["freshness", "--select", "raw_orders"],
            expected_selection=["source:raw_orders"],
            expected_run_nodes=[("source:raw_orders", "node", "success")],
            rewrites={"sources/raw.yml": SOURCES_YML.format(table="missing_orders")},
        ),
        AdhocVerbTestCase(
            description="a refused test run leaves its unchecked node skipped",
            prerequisites=[],
            command=["test", "--select", "customers", "--warehouse", "ANALYST_XS"],
            expected_selection=["model:customers"],
            expected_run_nodes=[("model:customers", "node", "skipped")],
            expected_status="failed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_ad_hoc_verb_when_the_checkout_changes_after_verification_then_the_worker_runs_the_verified_project(
    test_case: AdhocVerbTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    worker_case: WorkerExecutionTestCase = WorkerExecutionTestCase(
        description=test_case.description,
        model_sql="SELECT 5 AS amount",
        warehouse_query="SELECT amount FROM orders",
        expected_status="success",
        expected_rows=[(5,)],
        files={
            "models/customers.sql": CUSTOMERS_SQL,
            "tests/unit/test_customers.sql": CUSTOMERS_TEST.format(expected=1),
            "seeds/regions.csv": "region,label\nEU,Europe\n",
            "seeds/seeds.yml": SEEDS_YML.format(region_type="VARCHAR"),
            "sources/raw.yml": SOURCES_YML.format(table="raw_orders"),
            "python/rewrite_checkout.py": REWRITE_CHECKOUT,
        },
    )
    with scheduled_worker(
        tmp_path,
        monkeypatch,
        worker_case,
        server_arguments=("--password-origin", "https://control.example.com"),
    ) as worker:
        _ = wait_for_run(worker.api, worker.process, worker.log, job_key="annual_orders")
        admin: dict[str, Any] = provision_person(
            tmp_path / "control", "create-user", "orders-admin"
        )
        session: tuple[str, str] = person_login(worker.api, admin["name"])
        scope: dict[str, Any] = {"location_id": worker.api.location_id, "target": "development"}
        with duckdb.connect(str(worker.project / "warehouse.duckdb")) as warehouse:
            _ = warehouse.execute("CREATE TABLE raw_orders AS SELECT now() AS loaded_at")

        def launch(command: list[str]) -> tuple[dict[str, Any], str, dict[str, Any]]:
            check: dict[str, Any] = person_request(
                worker.api,
                "/api/control/runs/adhoc/check",
                body=scope | {"command": command, "priority": 0},
                session=session,
            )
            assert (check["error"], check["launchable"]) == (None, True), command
            run_id: str = str(uuid4())
            assert person_request(
                worker.api,
                "/api/control/runs/adhoc",
                body=scope
                | {"run_id": run_id, "command": command, "reason": "Check verbs", "priority": 0},
                session=session,
            ) == {"applied": True}
            return (
                check,
                run_id,
                wait_for_run(worker.api, worker.process, worker.log, run_id=run_id),
            )

        for command in test_case.prerequisites:
            assert launch(command)[2]["status"] == "success", (command, worker.log.read_text())
        _ = (worker.project / "rewrite.json").write_text(json.dumps(test_case.rewrites))
        check, run_id, completed = launch(test_case.command)
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        assert check["selection"] == test_case.expected_selection
        assert (
            sqlite_rows(
                worker.control_store,
                "SELECT node_key, kind, status FROM sqb_run_nodes "
                f"WHERE run_id = '{run_id}' ORDER BY node_key",
            )
            == test_case.expected_run_nodes
        )
        for name, text in test_case.rewrites.items():
            assert (worker.project / name).read_text() == text, "the checkout changed under the run"
