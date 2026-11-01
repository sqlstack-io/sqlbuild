"""Real DuckDB measurement audit build coverage."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import AuditOutcome
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.runtime.observability.main.identity_scope import identity_scope
from sqlbuild.runtime.observability.models import ExecutionIdentity
from tests.integration.src.sqlbuild.executor.build._test_types import BuildExecutionTestCase
from tests.integration.src.sqlbuild.executor.build.helpers import (
    run_build_for_project,
    write_build_project_files,
)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildExecutionTestCase(
            description="measurement audit outcomes",
            project_files={},
            expected_status=BuildStatus.FAILED,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_measurement_outcomes_when_building_then_gates_and_persists_canonical_history(
    test_case: BuildExecutionTestCase,
    tmp_path: Path,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    model_sql = """
        MODEL (description "Test model.",
          materialized table,
          audits [
            configured_rate (
              name pass_rate, measured_value 100, sample_count 10,
              minimum_samples 5,
              thresholds (warn (below 99), error (below 90))
            ),
            configured_rate (
              name warn_rate, measured_value 95, sample_count 10,
              minimum_samples 5,
              thresholds (warn (below 99), error (below 90))
            ),
            configured_rate (
              name error_rate, measured_value 80, sample_count 10,
              minimum_samples 5,
              thresholds (warn (below 99), error (below 90))
            ),
             configured_rate (
               name insufficient_rate, measured_value 80, sample_count 1,
               minimum_samples 5,
               thresholds (warn (below 99), error (below 90))
             ),
             empty_rate (
               name empty_rate,
               minimum_samples 5,
               thresholds (warn (below 99), error (below 90))
             )
          ]
        );
        WITH orders(order_id) AS (VALUES (1), (2), (3))
        SELECT orders.order_id FROM orders
    """
    audit_sql = """
        AUDIT (
          evaluation measurement,
          value measured_value,
          sample_count samples,
          sample_unit rows
        );
        MEASURE (
          SELECT
            CAST(@measured_value AS DOUBLE) AS measured_value,
            CAST(@sample_count AS INTEGER) AS samples
          FROM @relation
          LIMIT 1
        );
        EVIDENCE (
          SELECT order_id FROM @relation ORDER BY order_id
        );
    """
    write_build_project_files(
        project_dir=tmp_path,
        project_files={
            "sqlbuild_project.toml": 'name = "measurement_build"\nadapter = "duckdb"\n',
            "models/orders.sql": model_sql,
            "audits/generic/configured_rate.sql": audit_sql,
            "audits/generic/empty_rate.sql": """
            AUDIT (
              evaluation measurement,
              value measured_value,
              sample_count samples,
              sample_unit rows
            );
            MEASURE (
              SELECT CAST(NULL AS DOUBLE) AS measured_value, 0 AS samples
              FROM @relation
              LIMIT 1
            );
        """,
        },
    )
    with identity_scope(ExecutionIdentity(invocation_id="integration", run_id="test_run")):
        result: Any = run_build_for_project(
            test_case=test_case,
            project_dir=tmp_path,
            adapter=adapter,
            connection=connection,
        )

    assert result.status == test_case.expected_status
    outcomes: dict[str, AuditOutcome] = {
        audit.audit_name: audit.outcome for audit in result.model_results[0].audit_results
    }
    assert outcomes == {
        "pass_rate": AuditOutcome.PASS,
        "warn_rate": AuditOutcome.WARN,
        "error_rate": AuditOutcome.ERROR,
        "insufficient_rate": AuditOutcome.INSUFFICIENT,
        "empty_rate": AuditOutcome.INSUFFICIENT,
    }
    assert connection.execute(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'orders'"
    ).fetchone() == (0,)
    history: list[tuple[Any, ...]] = connection.execute(
        """
        SELECT audit_name, outcome, measured_value, sample_count,
               violation_count, thresholds_json, evidence_count
        FROM main._sqlbuild_audit_results
        ORDER BY audit_name
        """
    ).fetchall()
    assert len(history) == 5
    by_name: dict[str, tuple[Any, ...]] = {row[0]: row for row in history}
    assert by_name["pass_rate"][1:5] == ("pass", "100.0", 10, None)
    assert by_name["warn_rate"][1] == "warn"
    assert by_name["warn_rate"][6] == 3
    assert by_name["error_rate"][1] == "error"
    assert by_name["insufficient_rate"][1] == "insufficient"
    assert by_name["empty_rate"][1:5] == ("insufficient", None, 0, None)
    assert '"operator":"below"' in by_name["error_rate"][5]
