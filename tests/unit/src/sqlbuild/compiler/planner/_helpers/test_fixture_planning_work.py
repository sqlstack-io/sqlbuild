from __future__ import annotations

from dataclasses import replace

import pytest

from sqlbuild import _native as fixture_completion
from sqlbuild._native import (
    CompiledObjectKey,
    CompiledResourceType,
    CompiledSqlTest,
    CompileSqlReference,
    Dialect,
    PlanWarning,
    RelationFixtureCompletion,
    RelationFixturePlanningContext,
    SettingsConfig,
    SqlReferenceKind,
    SqlTestPlanEntry,
    SqlTestSelection,
    build_selected_test_entries,
    compiled_model_record,
    fixture_inference_count,
    project_with,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    CompleteFixturePlanningTestCase,
    FixturePlanningWorkTestCase,
    PlanTestChainTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import PlannerTestAdapter
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly.helpers import (
    build_test_and_project,
)


@pytest.mark.parametrize(
    "test_case",
    (
        FixturePlanningWorkTestCase(
            description="shared project metadata",
            test_count=250,
            expected_metadata_builds=1,
            expected_analysis_calls=1,
            expected_topology_builds=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_many_tests_when_building_entries_then_project_fixture_metadata_is_built_once(
    test_case: FixturePlanningWorkTestCase,
) -> None:
    compiled_test, project = build_test_and_project(
        PlanTestChainTestCase(
            description=test_case.description,
            model_queries={"orders": 'SELECT order_id FROM __source("raw_orders")'},
            mock_ref_ctes={},
            mock_source_ctes={"raw_orders": "SELECT 1 AS order_id"},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_cte_bodies={"orders": "SELECT 1 AS order_id"},
        )
    )
    source_reference: CompileSqlReference = CompileSqlReference(
        ref_kind=SqlReferenceKind.SOURCE,
        ref_name="raw_orders",
    )
    model: CompiledModelView = replace(
        compiled_model_record(project.models[0]),
        references=(source_reference,),
    )
    tests: tuple[CompiledSqlTest, ...] = tuple(
        replace(
            compiled_test,
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.SQL_TEST,
                name=f"orders_case_{index:04d}",
            ),
            name=f"orders_case_{index:04d}",
        )
        for index in range(test_case.test_count)
    )
    test_project: CompiledProjectView = project_with(
        project=project,
        models=(model,),
        sql_tests=tests,
        settings=SettingsConfig(sql_analysis=True),
    )
    fixture_completion.fixture_read_cache_info(reset=True)
    fixture_completion.fixture_planning_work_info(reset=True)

    entries: list[SqlTestPlanEntry]
    warnings: list[PlanWarning]
    entries, warnings = build_selected_test_entries(
        sql_test_selection=SqlTestSelection(),
        project=test_project,
        dialect=Dialect(PlannerTestAdapter()),
        selected_keys=frozenset((model.key,)),
    )

    assert len(entries) == test_case.test_count
    assert warnings == []
    metadata_builds: int
    topology_builds: int
    metadata_builds, topology_builds = fixture_completion.fixture_planning_work_info()
    assert metadata_builds == test_case.expected_metadata_builds
    assert fixture_completion.fixture_read_cache_info()[0] == test_case.expected_analysis_calls
    assert topology_builds == test_case.expected_topology_builds


@pytest.mark.parametrize(
    "test_case",
    (
        CompleteFixturePlanningTestCase(
            description="complete authoritative fixture",
            fixture_sql="SELECT 1 AS order_id",
            expected_analysis_calls=0,
            expected_inference_calls=1,
            expected_fixture_sql="SELECT 1 AS order_id",
        ),
        CompleteFixturePlanningTestCase(
            description="explicit typed null fixture",
            fixture_sql="SELECT CAST(NULL AS INTEGER) AS order_id WHERE FALSE",
            expected_analysis_calls=0,
            expected_inference_calls=1,
            expected_fixture_sql="SELECT CAST(NULL AS INTEGER) AS order_id WHERE FALSE",
        ),
        CompleteFixturePlanningTestCase(
            description="untyped null fixture",
            fixture_sql="SELECT NULL AS order_id WHERE FALSE",
            expected_analysis_calls=0,
            expected_inference_calls=1,
            expected_fixture_sql=(
                'SELECT\n  CAST("__sqlbuild_partial_fixture".order_id AS INTEGER) '
                "AS order_id\nFROM (\nSELECT NULL AS order_id WHERE FALSE\n) "
                'AS "__sqlbuild_partial_fixture"'
            ),
        ),
        CompleteFixturePlanningTestCase(
            description="untyped null before typed union value",
            fixture_sql=(
                "SELECT NULL AS order_id UNION ALL SELECT CAST(1.5 AS DOUBLE) AS order_id"
            ),
            expected_analysis_calls=0,
            expected_inference_calls=1,
            expected_fixture_sql=(
                "SELECT NULL AS order_id UNION ALL SELECT CAST(1.5 AS DOUBLE) AS order_id"
            ),
        ),
        CompleteFixturePlanningTestCase(
            description="unquoted mixed-case null fixture",
            fixture_sql="SELECT NULL AS Order_ID WHERE FALSE",
            expected_analysis_calls=0,
            expected_inference_calls=1,
            expected_fixture_sql=(
                'SELECT\n  CAST("__sqlbuild_partial_fixture".Order_ID AS INTEGER) '
                "AS Order_ID\nFROM (\nSELECT NULL AS Order_ID WHERE FALSE\n) "
                'AS "__sqlbuild_partial_fixture"'
            ),
        ),
        CompleteFixturePlanningTestCase(
            description="quoted mixed-case null fixture",
            fixture_sql='SELECT NULL AS "Order_ID" WHERE FALSE',
            expected_analysis_calls=0,
            expected_inference_calls=1,
            expected_fixture_sql=(
                'SELECT\n  CAST("__sqlbuild_partial_fixture"."Order_ID" AS INTEGER) '
                'AS "Order_ID"\nFROM (\nSELECT NULL AS "Order_ID" WHERE FALSE\n) '
                'AS "__sqlbuild_partial_fixture"'
            ),
        ),
        CompleteFixturePlanningTestCase(
            description="unknown non-null expression fixture",
            fixture_sql="SELECT custom_value() AS order_id",
            expected_analysis_calls=0,
            expected_inference_calls=1,
            expected_fixture_sql="SELECT custom_value() AS order_id",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_complete_authoritative_fixture_when_completing_then_skips_column_analysis(
    test_case: CompleteFixturePlanningTestCase,
) -> None:
    _, project = build_test_and_project(
        PlanTestChainTestCase(
            description=test_case.description,
            model_queries={"orders": 'SELECT order_id FROM __source("raw_orders")'},
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
        )
    )
    source_reference: CompileSqlReference = CompileSqlReference(
        ref_kind=SqlReferenceKind.SOURCE,
        ref_name="raw_orders",
    )
    model: CompiledModelView = replace(
        compiled_model_record(project.models[0]), references=(source_reference,)
    )
    source_key: tuple[CompiledResourceType, str] = (
        CompiledResourceType.SOURCE,
        "raw_orders",
    )
    context: RelationFixturePlanningContext = RelationFixturePlanningContext(
        models_by_name={model.name: model},
        relations={
            source_key: ((("order_id", "INTEGER", False),), True),
            (CompiledResourceType.MODEL, model.name): ((), False),
        },
    )
    fixture_completion.fixture_read_cache_info(reset=True)
    fixture_inference_count(reset=True)

    completed: RelationFixtureCompletion = fixture_completion.build_relation_fixture_completion(
        project=project_with(project=project, models=(model,)),
        dialect=Dialect(PlannerTestAdapter()),
        request=(
            (model.name,),
            ((CompiledResourceType.SOURCE, {"raw_orders": test_case.fixture_sql}),),
        ),
        planning_context=context,
    )

    assert completed.diagnostics == ()
    assert completed.fixture_sql_by_key[source_key] == test_case.expected_fixture_sql
    assert fixture_completion.fixture_read_cache_info()[0] == test_case.expected_analysis_calls
    assert fixture_inference_count() == test_case.expected_inference_calls


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
