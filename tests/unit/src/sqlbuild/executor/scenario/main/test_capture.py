from __future__ import annotations

from pathlib import Path
from typing import Any, ClassVar

import pytest

from sqlbuild._native import ScenarioArtifactKind, ScenarioExecutionPlan
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ColumnInfo, QueryResult
from sqlbuild.executor.scenario._helpers.snapshots.core import (
    build_scenario_snapshot_capture_plan,
    build_scenario_snapshot_manifest_shell,
)
from sqlbuild.executor.scenario.constants import (
    SCENARIO_EXEC_CAPTURE_FAILED,
    SCENARIO_EXEC_CAPTURE_LIMIT_EXCEEDED,
)
from sqlbuild.executor.scenario.main._capture import execute_scenario_snapshot_capture
from sqlbuild.executor.scenario.models import (
    ScenarioSnapshotCaptureLimits,
    ScenarioSnapshotCapturePlan,
    ScenarioSnapshotCaptureRelationResult,
    ScenarioSnapshotCaptureResult,
    ScenarioSnapshotColumn,
    ScenarioSnapshotManifest,
    ScenarioSnapshotRelation,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.unit.src.sqlbuild.executor.scenario.main._test_types import (
    ExecuteScenarioSnapshotCaptureLimitTestCase,
    ExecuteScenarioSnapshotCaptureTestCase,
)
from tests.unit.src.sqlbuild.executor.scenario.main.helpers import (
    build_scenario_cleanup_test_plan_with_project_seed,
)

SCENARIO_PLAN: ScenarioExecutionPlan = build_scenario_cleanup_test_plan_with_project_seed()


class ScenarioSnapshotCaptureTestAdapter(BaseAdapter):
    adapter_name: ClassVar[str] = "scenario-snapshot-test"

    def __init__(self, *, fail_on_relation: str | None = None) -> None:
        self.fail_on_relation: str | None = fail_on_relation
        self.queries: list[str] = []

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def close(self, connection: object) -> None:
        del connection

    def _execute(self, connection: object, sql: str) -> object:
        del connection, sql
        return object()

    def query(self, connection: Any, sql: str, *, limit: int | None) -> QueryResult:
        del connection, limit
        self.queries.append(sql)
        if self.fail_on_relation is not None and self.fail_on_relation in sql:
            raise RuntimeError("warehouse read failed")
        if "COUNT(*)" in sql and "__sqb_51b385aebe20__source__raw__orders" in sql:
            return QueryResult(columns=("count",), rows=((2,),))
        if "COUNT(*)" in sql and "__sqb_51b385aebe20__ref__stg_customers" in sql:
            return QueryResult(columns=("count",), rows=((1,),))
        if "COUNT(*)" in sql and "__sqb_51b385aebe20__seed__country_codes" in sql:
            return QueryResult(columns=("count",), rows=((1,),))
        if "__sqb_51b385aebe20__source__raw__orders" in sql:
            return QueryResult(
                columns=("order_id", "amount"),
                rows=((1, 10.5), (2, 20)),
            )
        if "__sqb_51b385aebe20__ref__stg_customers" in sql:
            return QueryResult(columns=("customer_id",), rows=((10,),))
        if "__sqb_51b385aebe20__seed__country_codes" in sql:
            return QueryResult(columns=("country_code",), rows=(("US",),))
        raise RuntimeError(f"unexpected query: {sql}")

    def describe_relation(self, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        del connection
        if "__sqb_51b385aebe20__source__raw__orders" in relation:
            return (
                ColumnInfo(name="order_id", type="INTEGER"),
                ColumnInfo(name="amount", type="DECIMAL(10,2)"),
            )
        if "__sqb_51b385aebe20__ref__stg_customers" in relation:
            return (ColumnInfo(name="customer_id", type="DECIMAL(38,0)"),)
        if "__sqb_51b385aebe20__seed__country_codes" in relation:
            return (ColumnInfo(name="country_code", type="VARCHAR"),)
        raise RuntimeError(f"unexpected describe: {relation}")

    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        if database is not None and schema is not None:
            return f"`{database}.{schema}.{name}`"
        if schema is not None:
            return f"`{schema}.{name}`"
        return super().render_qualified_name(database=database, schema=schema, name=name)


@pytest.mark.parametrize(
    "test_case",
    [
        ExecuteScenarioSnapshotCaptureTestCase(
            description="captures materialized scenario inputs into JSONL and manifest",
            expected_result=ScenarioSnapshotCaptureResult(
                scenario_name="revenue__customer_refund",
                status=ExecutionStatus.SUCCESS,
                manifest_path=Path(
                    "/repo/tests/_scenario_snapshots/revenue__customer_refund/scenario.json"
                ),
                manifest=ScenarioSnapshotManifest(
                    version=1,
                    scenario_name="revenue__customer_refund",
                    captured_at="2026-05-09T00:00:00Z",
                    capture_adapter="duckdb",
                    capture_dialect="duckdb",
                    sqlbuild_version="0.1.0",
                    input_fingerprint="placeholder",
                    total_rows=4,
                    total_bytes=97,
                    relations=(
                        ScenarioSnapshotRelation(
                            kind=SCENARIO_PLAN.fixture_plans[1].kind,
                            logical_name="stg_customers",
                            file_path=Path("refs/stg_customers.jsonl"),
                            row_count=1,
                            byte_count=19,
                            columns=(
                                ScenarioSnapshotColumn(
                                    name="customer_id",
                                    warehouse_type="DECIMAL(38,0)",
                                    local_type="DECIMAL(38,0)",
                                ),
                            ),
                        ),
                        ScenarioSnapshotRelation(
                            kind=ScenarioArtifactKind.SEED,
                            logical_name="country_codes",
                            file_path=Path("seeds/country_codes.jsonl"),
                            row_count=1,
                            byte_count=22,
                            columns=(
                                ScenarioSnapshotColumn(
                                    name="country_code",
                                    warehouse_type="VARCHAR",
                                    local_type="VARCHAR",
                                ),
                            ),
                        ),
                        ScenarioSnapshotRelation(
                            kind=SCENARIO_PLAN.fixture_plans[0].kind,
                            logical_name="raw__orders",
                            file_path=Path("sources/raw__orders.jsonl"),
                            row_count=2,
                            byte_count=56,
                            columns=(
                                ScenarioSnapshotColumn(
                                    name="order_id",
                                    warehouse_type="INTEGER",
                                    local_type="INT",
                                ),
                                ScenarioSnapshotColumn(
                                    name="amount",
                                    warehouse_type="DECIMAL(10,2)",
                                    local_type="DECIMAL(10,2)",
                                ),
                            ),
                        ),
                    ),
                ),
                relation_results=(
                    ScenarioSnapshotCaptureRelationResult(
                        kind=SCENARIO_PLAN.fixture_plans[1].kind,
                        logical_name="stg_customers",
                        source_relation=(
                            "`scenario_schema.__sqb_51b385aebe20__ref__stg_customers`"
                        ),
                        file_path=Path("refs/stg_customers.jsonl"),
                        status=ExecutionStatus.SUCCESS,
                        row_count=1,
                        byte_count=19,
                    ),
                    ScenarioSnapshotCaptureRelationResult(
                        kind=ScenarioArtifactKind.SEED,
                        logical_name="country_codes",
                        source_relation=(
                            "`scenario_schema.__sqb_51b385aebe20__seed__country_codes`"
                        ),
                        file_path=Path("seeds/country_codes.jsonl"),
                        status=ExecutionStatus.SUCCESS,
                        row_count=1,
                        byte_count=22,
                    ),
                    ScenarioSnapshotCaptureRelationResult(
                        kind=SCENARIO_PLAN.fixture_plans[0].kind,
                        logical_name="raw__orders",
                        source_relation=(
                            "`scenario_schema.__sqb_51b385aebe20__source__raw__orders`"
                        ),
                        file_path=Path("sources/raw__orders.jsonl"),
                        status=ExecutionStatus.SUCCESS,
                        row_count=2,
                        byte_count=56,
                    ),
                ),
            ),
            expected_jsonl_files={
                Path("refs/stg_customers.jsonl"): '{"customer_id":10}\n',
                Path("seeds/country_codes.jsonl"): '{"country_code":"US"}\n',
                Path("sources/raw__orders.jsonl"): (
                    '{"amount":10.5,"order_id":1}\n{"amount":20,"order_id":2}\n'
                ),
            },
            expected_manifest_fragment='"total_rows": 4',
            expected_query_fragments=(
                "SELECT * FROM `scenario_schema.__sqb_51b385aebe20__source__raw__orders`",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_capture_plan_when_executing_snapshot_capture_then_writes_jsonl_and_manifest(
    test_case: ExecuteScenarioSnapshotCaptureTestCase,
    tmp_path: Path,
) -> None:
    capture_plan: ScenarioSnapshotCapturePlan = build_scenario_snapshot_capture_plan(
        project_dir=tmp_path,
        scenario_plan=SCENARIO_PLAN,
    )
    manifest: ScenarioSnapshotManifest = build_scenario_snapshot_manifest_shell(
        capture_plan=capture_plan,
        captured_at="2026-05-09T00:00:00Z",
        capture_adapter="duckdb",
        capture_dialect="duckdb",
        sqlbuild_version="0.1.0",
    )
    adapter: ScenarioSnapshotCaptureTestAdapter = ScenarioSnapshotCaptureTestAdapter()

    result: ScenarioSnapshotCaptureResult = execute_scenario_snapshot_capture(
        capture_plan=capture_plan,
        manifest=manifest,
        adapter=adapter,
        connection=object(),
    )

    assert result.status == test_case.expected_result.status
    assert result.scenario_name == test_case.expected_result.scenario_name
    assert result.manifest is not None
    expected_manifest: ScenarioSnapshotManifest | None = test_case.expected_result.manifest
    assert expected_manifest is not None
    assert result.manifest.total_rows == expected_manifest.total_rows
    assert result.manifest.total_bytes == expected_manifest.total_bytes
    assert result.manifest.relations == expected_manifest.relations
    assert (
        tuple(
            ScenarioSnapshotCaptureRelationResult(
                kind=relation.kind,
                logical_name=relation.logical_name,
                source_relation=relation.source_relation,
                file_path=relation.file_path,
                status=relation.status,
                row_count=relation.row_count,
                byte_count=relation.byte_count,
            )
            for relation in result.relation_results
        )
        == test_case.expected_result.relation_results
    )
    for relative_path, expected_contents in test_case.expected_jsonl_files.items():
        assert (capture_plan.snapshot_root / relative_path).read_text(
            encoding="utf-8"
        ) == expected_contents
    assert test_case.expected_manifest_fragment in capture_plan.manifest_path.read_text(
        encoding="utf-8"
    )
    expected_query_fragment: str
    for expected_query_fragment in test_case.expected_query_fragments:
        assert any(expected_query_fragment in query for query in adapter.queries)


@pytest.mark.parametrize(
    "test_case",
    (
        ExecuteScenarioSnapshotCaptureLimitTestCase(
            description="row limit fails before downloading oversized relation",
            limits=ScenarioSnapshotCaptureLimits(max_rows_per_relation=1),
            expected_error_fragment="exceeding the per-relation capture limit",
            expected_missing_relative_path=Path("sources/raw__orders.jsonl"),
            expected_query_fragment="SELECT COUNT(*)",
            unexpected_query_fragment=(
                "SELECT * FROM `scenario_schema.__sqb_51b385aebe20__source__raw__orders`"
            ),
        ),
        ExecuteScenarioSnapshotCaptureLimitTestCase(
            description="byte limit removes partial jsonl",
            limits=ScenarioSnapshotCaptureLimits(max_bytes_per_relation=1),
            expected_error_fragment="would exceed the 1 byte limit",
            expected_missing_relative_path=Path("refs/stg_customers.jsonl"),
            expected_query_fragment="SELECT *",
            unexpected_query_fragment="not-a-query-fragment",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_capture_limit_when_executing_snapshot_capture_then_fails_safely(
    test_case: ExecuteScenarioSnapshotCaptureLimitTestCase,
    tmp_path: Path,
) -> None:
    capture_plan: ScenarioSnapshotCapturePlan = build_scenario_snapshot_capture_plan(
        project_dir=tmp_path,
        scenario_plan=SCENARIO_PLAN,
    )
    manifest: ScenarioSnapshotManifest = build_scenario_snapshot_manifest_shell(
        capture_plan=capture_plan,
        captured_at="2026-05-09T00:00:00Z",
        capture_adapter="duckdb",
        capture_dialect="duckdb",
        sqlbuild_version="0.1.0",
    )
    adapter: ScenarioSnapshotCaptureTestAdapter = ScenarioSnapshotCaptureTestAdapter()

    result: ScenarioSnapshotCaptureResult = execute_scenario_snapshot_capture(
        capture_plan=capture_plan,
        manifest=manifest,
        adapter=adapter,
        connection=object(),
        limits=test_case.limits,
    )

    assert result.status == ExecutionStatus.FAILED
    assert result.error_code == SCENARIO_EXEC_CAPTURE_LIMIT_EXCEEDED
    assert result.error_message is not None
    assert test_case.expected_error_fragment in result.error_message
    assert not (capture_plan.snapshot_root / test_case.expected_missing_relative_path).exists()
    assert any(test_case.expected_query_fragment in query for query in adapter.queries)
    assert not any(test_case.unexpected_query_fragment in query for query in adapter.queries)


@pytest.mark.parametrize(
    "test_case",
    [
        ExecuteScenarioSnapshotCaptureTestCase(
            description="returns failed result with relation context when warehouse read fails",
            expected_result=ScenarioSnapshotCaptureResult(
                scenario_name="revenue__customer_refund",
                status=ExecutionStatus.FAILED,
                manifest_path=Path(
                    "/repo/tests/_scenario_snapshots/revenue__customer_refund/scenario.json"
                ),
                relation_results=(
                    ScenarioSnapshotCaptureRelationResult(
                        kind=SCENARIO_PLAN.fixture_plans[1].kind,
                        logical_name="stg_customers",
                        source_relation=(
                            "`scenario_schema.__sqb_51b385aebe20__ref__stg_customers`"
                        ),
                        file_path=Path("refs/stg_customers.jsonl"),
                        status=ExecutionStatus.FAILED,
                        error_code=SCENARIO_EXEC_CAPTURE_FAILED,
                        error_help=(
                            "Check the materialized scenario input relation and rerun capture "
                            "with --retain to inspect warehouse artifacts."
                        ),
                        error_message=(
                            "Failed to capture ref 'stg_customers': warehouse read failed"
                        ),
                    ),
                ),
                error_code=SCENARIO_EXEC_CAPTURE_FAILED,
                error_help=(
                    "Check the materialized scenario input relation and rerun capture "
                    "with --retain to inspect warehouse artifacts."
                ),
                error_message="Failed to capture ref 'stg_customers': warehouse read failed",
            ),
            expected_jsonl_files={},
            expected_manifest_fragment="",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_capture_relation_read_failure_when_executing_then_returns_failed_result(
    test_case: ExecuteScenarioSnapshotCaptureTestCase,
    tmp_path: Path,
) -> None:
    capture_plan: ScenarioSnapshotCapturePlan = build_scenario_snapshot_capture_plan(
        project_dir=tmp_path,
        scenario_plan=SCENARIO_PLAN,
    )
    manifest: ScenarioSnapshotManifest = build_scenario_snapshot_manifest_shell(
        capture_plan=capture_plan,
        captured_at="2026-05-09T00:00:00Z",
        capture_adapter="duckdb",
        capture_dialect="duckdb",
        sqlbuild_version="0.1.0",
    )
    adapter: ScenarioSnapshotCaptureTestAdapter = ScenarioSnapshotCaptureTestAdapter(
        fail_on_relation="stg_customers"
    )

    result: ScenarioSnapshotCaptureResult = execute_scenario_snapshot_capture(
        capture_plan=capture_plan,
        manifest=manifest,
        adapter=adapter,
        connection=object(),
    )

    assert result.status == test_case.expected_result.status
    assert result.relation_results == test_case.expected_result.relation_results
    assert result.error_message == test_case.expected_result.error_message
