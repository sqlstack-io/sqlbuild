"""The installed server expires security audit rows only after every configured sink has them."""

import hashlib
import json
import os
import sqlite3
import subprocess
import sys
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    AuditHorizonTestCase,
    StatisticsHorizonRejectionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    audit_actions,
    run_server_until,
)

UNREACHABLE_SINK: str = "http://127.0.0.1:9/orders-audit"


@pytest.mark.parametrize(
    "test_case",
    [
        AuditHorizonTestCase(
            "Configured sink keeps the row it has not received", "undelivered_past_horizon", 1
        )
    ],
    ids=lambda case: case.description,
)
def test_given_expired_audit_rows_when_server_runs_then_only_delivered_rows_are_deleted(
    test_case: AuditHorizonTestCase, tmp_path: Path
) -> None:
    fixture: dict[str, Any] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/security_audit_retention.json"
        ).read_text()
    )[test_case.fixture]
    _ = run_server_until(tmp_path, ("--no-scheduler",), lambda log: "Control API ready" in log)
    now: datetime = datetime.now(UTC)
    sink_id: str = hashlib.sha256(UNREACHABLE_SINK.encode()).hexdigest()
    with sqlite3.connect(tmp_path / ".sqlbuild/control.sqlite3") as connection:
        (last,) = connection.execute(
            "SELECT COALESCE(MAX(storage_id), 0) FROM sqb_security_audit"
        ).fetchone()
        _ = connection.executemany(
            "INSERT INTO sqb_security_audit (event_id, occurred_at, actor, action, granting_role, "
            "request_id, ip_address, scope_json, before_json, after_json, storage_id) "
            "VALUES (?, ?, 'orders-admin', ?, 'admin', ?, NULL, '{}', 'null', '{}', ?)",
            [
                (
                    str(uuid4()),
                    (now - timedelta(days=row["age_days"])).strftime("%Y-%m-%dT%H:%M:%S.%fZ"),
                    row["action"],
                    str(uuid4()),
                    last + offset,
                )
                for offset, row in enumerate(fixture["rows"], start=1)
            ],
        )
        _ = connection.execute(
            "INSERT INTO sqb_security_audit_deliveries (sink_id, storage_id, event_id, delivered_at) "
            "SELECT ?, storage_id, event_id, occurred_at FROM sqb_security_audit "
            "WHERE action IN (SELECT value FROM json_each(?))",
            (sink_id, json.dumps(fixture["delivered_actions"])),
        )
    log: str = run_server_until(
        tmp_path,
        ("--no-scheduler", *fixture["server_arguments"]),
        lambda _: audit_actions(tmp_path) == tuple(fixture["expected_actions"]),
        {"SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL": UNREACHABLE_SINK},
    )
    assert f"Security audit retention: retained past horizon: {test_case.expected_retained}" in log
    with sqlite3.connect(tmp_path / ".sqlbuild/control.sqlite3") as connection:
        deliveries: list[tuple[int]] = connection.execute(
            "SELECT storage_id FROM sqb_security_audit_deliveries"
        ).fetchall()
    assert deliveries == []


@pytest.mark.parametrize(
    "test_case",
    [
        StatisticsHorizonRejectionTestCase(
            "Zero days", "0", "--audit-retention-days must be between 1 and 65535"
        ),
        StatisticsHorizonRejectionTestCase(
            "Beyond the supported range",
            "65536",
            "--audit-retention-days must be between 1 and 65535",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_audit_horizon_when_server_starts_then_it_refuses_before_storage(
    test_case: StatisticsHorizonRejectionTestCase, tmp_path: Path
) -> None:
    environment: dict[str, str] = os.environ.copy()
    environment.pop("SQLBUILD_CONTROL_DATABASE_URL", None)
    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "server",
            "--port",
            "0",
            "--audit-retention-days",
            test_case.days,
        ],
        cwd=tmp_path,
        env=environment,
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert result.returncode != 0
    assert test_case.expected_error in result.stderr
    assert result.stdout == ""
    assert not (tmp_path / ".sqlbuild/control.sqlite3").exists()
