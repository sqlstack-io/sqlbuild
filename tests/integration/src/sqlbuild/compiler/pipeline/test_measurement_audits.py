from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import AuditPlanEntry, CompilePipelineResult
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import AuditEvaluationMode, ThresholdOperator
from sqlbuild.compiler.compile.types import CompiledAuditView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    MeasurementCompileErrorIntegrationTestCase,
    MeasurementCompileIntegrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    run_compile_pipeline_for_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        MeasurementCompileIntegrationTestCase(
            description="attached measurement full contract",
            expected_minimum_samples=2,
            expected_severity="error",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_attached_measurement_audit_when_compiling_then_full_contract_reaches_plan(
    test_case: MeasurementCompileIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": (
                'name = "measurement_demo"\nadapter = "duckdb"\n\n'
                "[connection]\n"
                'database = ":memory:"\n'
            ),
            "sql/models/orders.sql": """
                MODEL (description "Test model orders.",
                  materialized table,
                  audits [
                    valid_order_rate (
                      condition "order_id IS NOT NULL",
                      minimum_samples 2,
                      evidence_limit 7,
                      thresholds (
                        warn (below 100),
                        error (below 99.9)
                      )
                    )
                  ]
                );

                SELECT 1 AS order_id
            """,
            "sql/audits/generic/valid_order_rate.sql": """
                AUDIT (
                  evaluation measurement,
                  value valid_rate,
                  sample_count total_rows,
                  sample_unit rows
                );

                MEASURE (
                  SELECT
                    COUNT(*) AS total_rows,
                    100.0 * AVG(CASE WHEN @condition THEN 1 ELSE 0 END) AS valid_rate
                  FROM @relation
                );

                EVIDENCE (
                  SELECT * FROM @relation WHERE NOT (@condition)
                );
            """,
        },
    )

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
    )

    compiled: CompiledAuditView = result.project.audits[0]
    planned: AuditPlanEntry = result.plan_output.audit_entries[0]
    assert compiled.evaluation_mode == AuditEvaluationMode.MEASUREMENT
    assert compiled.measurement_contract is not None
    assert compiled.measurement_contract.value_column == "valid_rate"
    assert compiled.measurement_contract.sample_count_column == "total_rows"
    assert compiled.measurement_contract.sample_unit == "rows"
    assert compiled.thresholds is not None and compiled.thresholds.error is not None
    assert compiled.thresholds.error.operator == ThresholdOperator.BELOW
    assert compiled.minimum_samples == test_case.expected_minimum_samples
    assert compiled.evidence_limit == 7
    assert "order_id IS NOT NULL" in compiled.sql_body
    assert (
        compiled.evidence_sql is not None and "NOT (order_id IS NOT NULL)" in compiled.evidence_sql
    )

    assert planned.evaluation_mode == AuditEvaluationMode.MEASUREMENT
    assert planned.value_column == "valid_rate"
    assert planned.sample_count_column == "total_rows"
    assert planned.sample_unit == "rows"
    assert planned.thresholds == compiled.thresholds
    assert planned.minimum_samples == test_case.expected_minimum_samples
    assert planned.evidence_limit == 7
    assert planned.severity.value == test_case.expected_severity
    assert "main.orders" in planned.resolved_sql
    assert "__ref" in planned.unresolved_sql
    assert planned.evidence_resolved_sql is not None
    assert "main.orders" in planned.evidence_resolved_sql
    assert planned.evidence_unresolved_sql == compiled.evidence_sql


@pytest.mark.parametrize(
    "test_case",
    [
        MeasurementCompileErrorIntegrationTestCase(
            description="generic definition owns policy",
            expected_error_fragment="attachment owns policy",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generic_measurement_definition_with_policy_when_compiling_then_definition_policy_is_rejected(
    test_case: MeasurementCompileErrorIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', audits [rate (thresholds (warn (below 100)))]); SELECT 1",
            "sql/audits/generic/rate.sql": """
                AUDIT (evaluation measurement, value rate, thresholds (warn (below 100)));
                MEASURE (SELECT 100 AS rate FROM @relation);
            """,
        },
    )

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        run_compile_pipeline_for_project(project_dir=tmp_path, adapter=DuckDbAdapter())


@pytest.mark.parametrize(
    "test_case",
    [
        MeasurementCompileErrorIntegrationTestCase(
            description="attachment missing thresholds",
            expected_error_fragment="must define at least one threshold",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_measurement_attachment_without_thresholds_when_compiling_then_policy_is_required(
    test_case: MeasurementCompileErrorIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', audits [rate]); SELECT 1",
            "sql/audits/generic/rate.sql": """
                AUDIT (evaluation measurement, value rate);
                MEASURE (SELECT 100 AS rate FROM @relation);
            """,
        },
    )

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        run_compile_pipeline_for_project(project_dir=tmp_path, adapter=DuckDbAdapter())


@pytest.mark.parametrize(
    "test_case",
    [
        MeasurementCompileErrorIntegrationTestCase(
            description="attachment severity conflict",
            expected_error_fragment="must not define severity",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_measurement_attachment_with_severity_when_compiling_then_conflict_is_rejected(
    test_case: MeasurementCompileErrorIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.', audits [rate (severity warn, thresholds (warn (below 100)))]); SELECT 1"
            ),
            "sql/audits/generic/rate.sql": """
                AUDIT (evaluation measurement, value rate);
                MEASURE (SELECT 100 AS rate FROM @relation);
            """,
        },
    )

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        run_compile_pipeline_for_project(project_dir=tmp_path, adapter=DuckDbAdapter())


@pytest.mark.parametrize(
    "test_case",
    [
        MeasurementCompileIntegrationTestCase(
            description="standalone header policy",
            expected_minimum_samples=5,
            expected_severity="warn",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_standalone_measurement_with_header_policy_when_compiling_then_it_is_allowed(
    test_case: MeasurementCompileIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id",
            "sql/models/customers.sql": "MODEL (description 'Test model customers.');\nSELECT 1 AS order_id",
            "sql/audits/singular/rate.sql": """
                AUDIT (
                  evaluation measurement,
                  value rate,
                  minimum_samples 5,
                  thresholds (warn (outside 95 100))
                );
                MEASURE (
                  SELECT 99 AS rate
                  FROM __ref("orders") JOIN __ref("customers") USING (order_id)
                );
            """,
        },
    )

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path, adapter=DuckDbAdapter()
    )

    audit: CompiledAuditView = result.project.audits[0]
    assert audit.thresholds is not None and audit.thresholds.warn is not None
    assert audit.thresholds.warn.operator == ThresholdOperator.OUTSIDE
    assert audit.minimum_samples == test_case.expected_minimum_samples
    assert audit.severity is not None and audit.severity.value == test_case.expected_severity
