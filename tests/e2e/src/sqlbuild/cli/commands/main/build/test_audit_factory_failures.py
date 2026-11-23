"""E2E coverage for factory-attached audit promotion gates."""

from __future__ import annotations

import subprocess
from pathlib import Path

import duckdb
import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    AuditFactoryFailureBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [AuditFactoryFailureBuildE2ETestCase("failure blocks promotion", 1, ((1, 50),))],
    ids=lambda case: case.description,
)
def test_given_failing_factory_audit_when_building_then_existing_table_is_untouched(
    test_case: AuditFactoryFailureBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="audit_factory_failure_project",
        repo_files={
            "sqlbuild_project.toml": """
name = "audit_factory_failure_project"
adapter = "duckdb"

[connection]
database = "audit_factory_failures.duckdb"

[defaults]
materialized = "table"
""",
            "sql/models/orders.sql": """
MODEL (description "Test model orders.", audit_factories [order_quality]);
SELECT 2 AS id, -10 AS amount
""",
            "sql/audits/generic/expression_is_true.sql": """
AUDIT ();
SELECT * FROM __ref("@model") WHERE NOT (@expression)
""",
            "python/nodes/factories/quality.py": """
from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory

@audit_factory
def order_quality():
    return [
        AuditCase(
            name="positive_amount",
            definition="expression_is_true",
            arguments={"expression": "amount > 0"},
            severity=AuditSeverity.ERROR,
        ),
    ]
""",
        },
    )
    database_path: Path = project_dir / "audit_factory_failures.duckdb"
    with duckdb.connect(str(database_path)) as connection:
        connection.execute("CREATE TABLE orders AS SELECT 1 AS id, 50 AS amount")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert "positive_amount" in result.stdout
    with duckdb.connect(str(database_path)) as connection:
        assert tuple(connection.execute("SELECT id, amount FROM orders").fetchall()) == (
            test_case.expected_rows
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
