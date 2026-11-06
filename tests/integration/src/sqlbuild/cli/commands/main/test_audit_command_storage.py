"""Real CLI coverage for where a standalone audit stores its history."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    PreservedTargetAuditTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import query_duckdb_rows

_PROJECT: str = (
    'name = "shop"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connections.local]\n"
    'database = "shop.duckdb"\n\n'
    "[targets.dev]\n"
    'connection = "local"\n'
)
_ORDERS_MODEL: str = (
    'MODEL (\n  description "One row per order",\n  materialized table,\n  schema sales,\n'
    "  columns (order_id (audits [not_null])),\n);\n\n"
    "SELECT 1 AS order_id\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PreservedTargetAuditTestCase(
            description="a preserved schema stores history beside the models",
            target_toml='schema = "preserve"\n',
            expected_history_schema="sales",
        ),
        PreservedTargetAuditTestCase(
            description="a named schema stores history in it",
            target_toml='schema = "dev"\n',
            expected_history_schema="dev",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_target_schema_when_auditing_alone_then_history_is_stored_where_a_build_stores_it(
    test_case: PreservedTargetAuditTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(
        _PROJECT + test_case.target_toml, encoding="utf-8"
    )
    (tmp_path / "models").mkdir()
    _ = (tmp_path / "models" / "orders.sql").write_text(_ORDERS_MODEL, encoding="utf-8")
    assert main(["--no-color", "--project-dir", str(tmp_path), "build"]) == 0
    _ = capsys.readouterr()

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "audit", "--json"])

    captured: tuple[str, str] = capsys.readouterr()
    output: str = "".join(captured)
    assert exit_code == 0, output
    assert "warning:" not in output
    payload: dict[str, object] = json.loads(captured[0])
    assert payload["command"] == "audit"
    assert payload["status"] == "success"
    assert payload["summary"] == {
        "pass_count": 1,
        "warn_count": 0,
        "fail_count": 0,
        "total_count": 1,
    }
    assert payload["audit_result_projection"] == {
        "attempted_count": 1,
        "written_count": 1,
        "failed_count": 0,
    }
    assert "projection_degraded" not in payload
    db_path: Path = tmp_path / "shop.duckdb"
    assert query_duckdb_rows(
        db_path=db_path,
        sql="SELECT table_schema FROM information_schema.tables "
        "WHERE table_name = '_sqlbuild_audit_results'",
    ) == ((test_case.expected_history_schema,),)
    assert query_duckdb_rows(
        db_path=db_path,
        sql=f"SELECT COUNT(*) FROM {test_case.expected_history_schema}._sqlbuild_audit_results",
    ) == ((2,),)
    assert query_duckdb_rows(
        db_path=db_path,
        sql="SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = 'preserve'",
    ) == ((0,),)
