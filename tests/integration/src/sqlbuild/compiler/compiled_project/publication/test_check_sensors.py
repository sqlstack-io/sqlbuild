"""A real job build reports each audit and SQL test it runs, so check and run sensors see them."""

import json
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CheckSensorsTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    sensor_calls,
    wait_for_run,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.sensors.helpers import (
    recording_sensors,
    wait_until,
)


@pytest.mark.skipif(sys.platform != "linux", reason="Isolated Linux worker acceptance")
@pytest.mark.parametrize(
    "test_case",
    [
        CheckSensorsTestCase(
            description="a run that ignores a failing audit succeeds, and both audits reach the sensors",
            worker=WorkerExecutionTestCase(
                "orders with a failing and a warning audit",
                "SELECT 7 AS amount, 'z' AS code",
                "SELECT amount FROM orders",
                "success",
                [(7,)],
                files={
                    "sql/models/orders.sql": (
                        "MODEL (description 'Orders.', materialized table, columns (\n"
                        "  amount (type INTEGER, audits [accepted_values (values [1], severity error)]),\n"
                        "  code (type VARCHAR, audits [accepted_values (values ['a'], severity warn)])));\n"
                        "SELECT 7 AS amount, 'z' AS code\n"
                    ),
                    "orchestration/jobs/annual_orders_job.toml": (
                        'name = "annual_orders_job"\ndescription = "Build orders."\nselect = "orders"\n'
                        'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                        'starts_at = "2000-01-01T00:00:00Z" }\n'
                        "\n[settings]\nignore_audit_failures = true\n"
                    ),
                    **recording_sensors("orders"),
                },
                default_job_files={},
            ),
            expected_check_events=[
                ("audit:accepted_values:model:orders:amount", "failed"),
                ("audit:accepted_values:model:orders:code", "warn"),
            ],
            expected_failed_checks=["audit:accepted_values:model:orders:amount"],
            expected_failed_nodes=[],
        ),
        CheckSensorsTestCase(
            description="a failing SQL test fails its run and still reaches the sensors",
            worker=WorkerExecutionTestCase(
                "orders with a failing SQL test",
                "SELECT 3 AS amount",
                "SELECT count(*) FROM duckdb_tables() WHERE table_name = 'stg_orders'",
                "partial",
                [(0,)],
                files={
                    "sql/models/raw_orders.sql": (
                        "MODEL (description 'Raw orders.', materialized table);\n"
                        "SELECT 1 AS order_id, 10 AS amount\n"
                    ),
                    "sql/models/stg_orders.sql": (
                        "MODEL (description 'Staged orders.', materialized table);\n"
                        'SELECT order_id, amount * 2 AS amount FROM __ref("raw_orders")\n'
                    ),
                    "tests/unit/test_stg_orders.sql": (
                        'TEST (name "stg_orders__doubles_amounts");\n\n'
                        "WITH __ref__raw_orders AS (SELECT 1 AS order_id, 10 AS amount),\n"
                        "__expected__stg_orders AS (SELECT 1 AS order_id, 99 AS amount)\n"
                        "SELECT 1\n"
                    ),
                    "orchestration/jobs/annual_orders_job.toml": (
                        'name = "annual_orders_job"\ndescription = "Build orders."\n'
                        'select = "orders raw_orders stg_orders"\n'
                        'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                        'starts_at = "2000-01-01T00:00:00Z" }\n'
                    ),
                    **recording_sensors("stg_orders"),
                },
                default_job_files={},
            ),
            expected_check_events=[("sql_test:stg_orders__doubles_amounts", "failed")],
            expected_failed_checks=["sql_test:stg_orders__doubles_amounts"],
            expected_failed_nodes=["model:stg_orders"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_job_build_with_audits_and_sql_tests_when_sensors_watch_it_then_each_failed_or_warned_check_is_an_event(
    test_case: CheckSensorsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    calls: Path = tmp_path / "calls.jsonl"
    calls.touch()
    monkeypatch.setenv("SQLBUILD_SENSOR_CALLS", str(calls))
    with scheduled_worker(tmp_path, monkeypatch, test_case.worker) as worker:
        run: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job"
        )
        assert run["status"] == test_case.worker.expected_status, worker.log.read_text()
        finished: list[dict[str, Any]] = wait_until(
            lambda: sensor_calls(calls, {"sensor": "orders_run_sensor"}),
            "the run sensor is called with the finished run",
        )
        checks: list[dict[str, Any]] = wait_until(
            lambda: (
                sensor_calls(calls, {"sensor": "orders_checks_sensor"})[
                    len(test_case.expected_check_events) - 1 :
                ]
                and sensor_calls(calls, {"sensor": "orders_checks_sensor"})
            ),
            "the check sensor is called with each failed or warned check",
        )
        assert sorted({(call["check"], call["status"]) for call in checks}) == (
            test_case.expected_check_events
        ), json.dumps(checks)
        assert (finished[0]["failed_checks"], finished[0]["failed_nodes"]) == (
            test_case.expected_failed_checks,
            test_case.expected_failed_nodes,
        ), json.dumps(finished)
