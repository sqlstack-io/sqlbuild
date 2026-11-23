"""Unit coverage for fingerprinting column-migration candidates."""

from __future__ import annotations

from datetime import UTC, datetime
from pathlib import Path
from unittest.mock import Mock

import pytest

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    DeferralInputs,
    Dialect,
    Fingerprint,
    PlannerInputError,
    PlannerOverrides,
    PlannerRuntime,
    PlannerScope,
    WarehouseFingerprints,
    WarehouseSnapshot,
    live_reads,
    plan_column_migrations,
)
from sqlbuild.adapter.contract.models import ColumnInfo, RelationInfo
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.compiler.planner._helpers.migrations._test_types import (
    UnfingerprintableCandidateTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.migrations.helpers import BASE_CONFIG


@pytest.mark.parametrize(
    "test_case",
    [
        UnfingerprintableCandidateTestCase(
            description="unterminated string literal",
            query_sql="SELECT 'open AS order_status FROM raw_orders",
            expected_message="Model 'orders' SQL cannot be fingerprinted",
            expected_code="S025",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_untokenizable_tracked_model_when_planning_column_migrations_then_error_names_model(
    test_case: UnfingerprintableCandidateTestCase,
) -> None:
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=(),
        name="orders",
        relative_path=Path("sql/models/orders.sql"),
        query_sql=test_case.query_sql,
        config=CompileModelConfig(values=dict(BASE_CONFIG)),
        destination=CompiledRelationLocation(
            database=None, schema="analytics", name="orders", qualified_name=None
        ),
    )
    adapter: Mock = Mock()
    adapter.sql_analysis_dialect.return_value = "duckdb"
    runtime: PlannerRuntime = PlannerRuntime(
        project=CompiledProject(
            run_id="run-1",
            effective_target_name="test",
            effective_connection={},
            effective_vars={},
            models=(model,),
        ),
        dialect=Dialect(adapter),
        reads=live_reads(adapter=adapter, connection=object()),
    )
    scope: PlannerScope = PlannerScope(
        upstream_deps={model.key: ()},
        downstream_deps={model.key: ()},
        all_keys={"orders": model.key},
        models_by_name={"orders": model},
        selected_keys=frozenset({model.key}),
        execution_order=(model.key,),
    )
    snapshot: WarehouseSnapshot = WarehouseSnapshot(
        existing_relations={
            "orders": RelationInfo(
                database=None, schema="analytics", name="orders", relation_type="BASE TABLE"
            )
        },
        existing_columns={"orders": (ColumnInfo(name="order_status", type="VARCHAR"),)},
        fingerprints=WarehouseFingerprints(
            models={
                "orders": Fingerprint(
                    node_type="model",
                    node_name="orders",
                    target_database=None,
                    target_schema="analytics",
                    target_name="orders",
                    run_id="run-0",
                    definition_hash="previous",
                    schema_fingerprint="",
                    definition="SELECT 'open' AS order_status FROM raw_orders",
                    ts=datetime(2026, 1, 15, tzinfo=UTC),
                )
            }
        ),
        column_dialect="duckdb",
    )

    with pytest.raises(PlannerInputError, match=test_case.expected_message) as raised:
        _ = plan_column_migrations(
            runtime=runtime,
            scope=scope,
            snapshot=snapshot,
            full_refresh_model_names=frozenset(),
            physical_relations={},
            overrides=PlannerOverrides(),
            deferral=DeferralInputs(),
            source_columns={},
        )

    assert raised.value.code == test_case.expected_code
