"""Round-trip budget guard for planning a large multi-schema Snowflake project offline.

The project is compiled and planned through the real pipeline; only the network is replaced by
a recording warehouse that answers metadata SQL from in-memory synthetic catalog rows.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import CompilePipelineResult, PlanReason
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.relations.constants import INSPECTION_IN_LIST_LIMIT
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    SnowflakeCursorBoundsBudgetTestCase,
    SnowflakeFreshTargetTestCase,
    SnowflakeInspectionBudgetTestCase,
    SnowflakeManySchemasTestCase,
    SnowflakeMissingDatabaseTestCase,
    SnowflakeReplanTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner.main.helpers import (
    offline_cursor_bound_relations,
    offline_information_schema_queries,
    offline_metadata_queries,
    offline_metadata_reads_by_schema,
    plan_offline_snowflake_project,
    relations_in_database,
    relations_in_schemas,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    OfflineSnowflakeAdapter,
    RecordedQuery,
    RecordingSnowflakeWarehouse,
    SyntheticSnowflakeProject,
    write_synthetic_snowflake_project,
)

_CURSOR_BATCH_SIZE: int = 50
_ONE_READ_PER_SCHEMA: dict[str, int] = {"STAGING": 1, "INTERMEDIATE": 1, "MARTS": 1, "RAW": 1}
_SOURCES: int = 260
_SHOW_READS_PER_SCHEMA: int = 3
_FRESHNESS_READS: int = -(-_SOURCES // INSPECTION_IN_LIST_LIMIT)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeInspectionBudgetTestCase(
            description="about 2,500 relations",
            unmanaged_relations_per_schema=450,
            expected_schema_reads=_ONE_READ_PER_SCHEMA,
            expected_metadata_budget=_SHOW_READS_PER_SCHEMA * 4 + _FRESHNESS_READS,
            expected_in_list_limit=INSPECTION_IN_LIST_LIMIT,
            expected_freshness_reads=_FRESHNESS_READS,
        ),
        SnowflakeInspectionBudgetTestCase(
            description="about 4,300 relations",
            unmanaged_relations_per_schema=900,
            expected_schema_reads=_ONE_READ_PER_SCHEMA,
            expected_metadata_budget=_SHOW_READS_PER_SCHEMA * 4 + _FRESHNESS_READS,
            expected_in_list_limit=INSPECTION_IN_LIST_LIMIT,
            expected_freshness_reads=_FRESHNESS_READS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_large_multi_schema_project_when_planning_then_metadata_reads_stay_in_budget(
    test_case: SnowflakeInspectionBudgetTestCase, tmp_path: Path
) -> None:
    project: SyntheticSnowflakeProject = write_synthetic_snowflake_project(
        project_dir=tmp_path / "orders_platform",
        models_per_schema=60,
        incremental_every=2,
        sources=_SOURCES,
        unmanaged_relations_per_schema=test_case.unmanaged_relations_per_schema,
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=project.relations
    )

    result: CompilePipelineResult = plan_offline_snowflake_project(
        project=project, warehouse=warehouse
    )

    metadata: tuple[RecordedQuery, ...] = offline_metadata_queries(warehouse)
    assert len(result.plan_output.model_entries) == len(project.model_schemas)
    warehouse_queries: tuple[RecordedQuery, ...] = offline_information_schema_queries(warehouse)
    assert (
        offline_metadata_reads_by_schema(warehouse=warehouse, kind="show_tables")
        == test_case.expected_schema_reads
    )
    assert (
        offline_metadata_reads_by_schema(warehouse=warehouse, kind="show_views")
        == test_case.expected_schema_reads
    )
    assert (
        offline_metadata_reads_by_schema(warehouse=warehouse, kind="show_schema_columns")
        == test_case.expected_schema_reads
    )
    assert warehouse.queries_of_kind("show_columns") == ()
    assert warehouse.queries_of_kind("other_metadata") == ()
    assert len(warehouse_queries) == test_case.expected_freshness_reads
    assert all("last_altered" in query.sql for query in warehouse_queries)
    assert max(query.largest_in_list for query in metadata) <= test_case.expected_in_list_limit
    assert len({(query.sql, query.params) for query in metadata}) == len(metadata)
    assert len(metadata) <= test_case.expected_metadata_budget


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeCursorBoundsBudgetTestCase(
            description="batched statements read each relation once with bounded parallelism",
            statement_latency_seconds=0.002,
            expected_max_concurrency=OfflineSnowflakeAdapter.metadata_inspection_concurrency,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_incremental_models_when_planning_then_cursor_bounds_run_in_parallel_batches(
    test_case: SnowflakeCursorBoundsBudgetTestCase, tmp_path: Path
) -> None:
    project: SyntheticSnowflakeProject = write_synthetic_snowflake_project(
        project_dir=tmp_path / "orders_platform",
        models_per_schema=60,
        incremental_every=2,
        unmanaged_relations_per_schema=50,
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=project.relations, statement_latency_seconds=test_case.statement_latency_seconds
    )

    _ = plan_offline_snowflake_project(project=project, warehouse=warehouse)

    bounds: tuple[RecordedQuery, ...] = warehouse.queries_of_kind("cursor_bounds")
    relations: tuple[str, ...] = offline_cursor_bound_relations(warehouse)
    assert len(relations) >= len(project.incremental_model_names)
    assert len(relations) == len(set(relations))
    assert len(bounds) == -(-len(relations) // _CURSOR_BATCH_SIZE)
    assert 1 < warehouse.max_concurrent_cursor_bounds <= test_case.expected_max_concurrency


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeReplanTestCase(
            description="a second invocation re-reads bounds and listings",
            expected_listing_reads=4,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_planned_project_when_planning_again_then_cursor_bounds_are_read_again(
    test_case: SnowflakeReplanTestCase, tmp_path: Path
) -> None:
    project: SyntheticSnowflakeProject = write_synthetic_snowflake_project(
        project_dir=tmp_path / "orders_platform", unmanaged_relations_per_schema=10
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=project.relations
    )
    _ = plan_offline_snowflake_project(project=project, warehouse=warehouse)
    first: int = len(warehouse.queries_of_kind("cursor_bounds"))
    warehouse.reset()

    _ = plan_offline_snowflake_project(project=project, warehouse=warehouse, no_cache=False)

    assert len(warehouse.queries_of_kind("cursor_bounds")) == first
    assert len(warehouse.queries_of_kind("show_tables")) == test_case.expected_listing_reads


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeManySchemasTestCase(
            description="many small schemas read model columns once per schema",
            schema_count=24,
            models_per_schema=2,
            expected_per_relation_column_reads=30,
            expected_schema_column_reads=24,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_many_small_schemas_when_planning_then_columns_are_read_per_schema(
    test_case: SnowflakeManySchemasTestCase, tmp_path: Path
) -> None:
    project: SyntheticSnowflakeProject = write_synthetic_snowflake_project(
        project_dir=tmp_path / "orders_platform",
        model_schemas=tuple(f"region_{index}" for index in range(test_case.schema_count)),
        models_per_schema=test_case.models_per_schema,
        sources=30,
        unmanaged_relations_per_schema=5,
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=project.relations
    )

    result: CompilePipelineResult = plan_offline_snowflake_project(
        project=project, warehouse=warehouse
    )

    assert len(result.plan_output.model_entries) == len(project.model_schemas)
    assert (
        len(warehouse.queries_of_kind("show_columns"))
        == test_case.expected_per_relation_column_reads
    )
    assert (
        len(warehouse.queries_of_kind("show_schema_columns"))
        == test_case.expected_schema_column_reads
    )
    assert warehouse.queries_of_kind("columns") == ()


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeFreshTargetTestCase(
            description="target schemas created by the first build do not exist yet",
            warehouse_schemas=frozenset({"RAW"}),
            expected_reason=PlanReason.FIRST_RUN,
            expected_schema_checks=3,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fresh_target_when_planning_then_every_model_is_a_first_run(
    test_case: SnowflakeFreshTargetTestCase, tmp_path: Path
) -> None:
    project: SyntheticSnowflakeProject = write_synthetic_snowflake_project(
        project_dir=tmp_path / "orders_platform", unmanaged_relations_per_schema=5
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=relations_in_schemas(project=project, schemas=test_case.warehouse_schemas)
    )

    result: CompilePipelineResult = plan_offline_snowflake_project(
        project=project, warehouse=warehouse
    )

    assert {entry.reason for entry in result.plan_output.model_entries} == {
        test_case.expected_reason
    }
    assert len(result.plan_output.model_entries) == len(project.model_schemas)
    assert len(warehouse.queries_of_kind("show_schemas")) == test_case.expected_schema_checks
    assert warehouse.queries_of_kind("columns") == ()


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeMissingDatabaseTestCase(
            description="target database absent from the account",
            warehouse_database="ARCHIVE",
            expected_error_fragment="database analytics does not exist",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_database_when_planning_then_plan_fails_naming_it(
    test_case: SnowflakeMissingDatabaseTestCase, tmp_path: Path
) -> None:
    project: SyntheticSnowflakeProject = write_synthetic_snowflake_project(
        project_dir=tmp_path / "orders_platform", unmanaged_relations_per_schema=5
    )
    warehouse: RecordingSnowflakeWarehouse = RecordingSnowflakeWarehouse(
        relations=relations_in_database(project=project, database=test_case.warehouse_database)
    )

    with pytest.raises(AdapterUserError, match=test_case.expected_error_fragment):
        _ = plan_offline_snowflake_project(project=project, warehouse=warehouse)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
