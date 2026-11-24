from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    DiscoveredHookFunction,
    PythonHookEntry,
    ScenarioExecutionPlan,
    ScenarioFixturePlan,
    SeedPlanEntry,
)
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build.models import SeedExecutionResult
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scenario._helpers.execution.model_execution import execute_scenario_models
from sqlbuild.executor.scenario._helpers.lifecycle.expectations import (
    execute_scenario_assertion_expectations,
    execute_scenario_expected_expectations,
)
from sqlbuild.executor.scenario._helpers.lifecycle.fixtures import (
    execute_scenario_fixtures,
    execute_scenario_seed_entries,
)
from sqlbuild.executor.scenario.main._cleanup import execute_scenario_cleanup
from sqlbuild.executor.scenario.main._fixtures import execute_scenario_fixture
from sqlbuild.executor.scenario.models import (
    ScenarioAssertionExpectationExecutionResult,
    ScenarioCleanupExecutionResult,
    ScenarioExpectedExpectationExecutionResult,
    ScenarioFixtureExecutionResult,
)
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS
from tests.integration.src.sqlbuild.executor.scenario._test_types import (
    ScenarioAssertionExpectationIntegrationTestCase,
    ScenarioCleanupIntegrationTestCase,
    ScenarioExpectedExpectationIntegrationTestCase,
    ScenarioFixtureFailureIntegrationTestCase,
    ScenarioFixtureMaterializationIntegrationTestCase,
    ScenarioModelBuildIntegrationTestCase,
    ScenarioProjectSeedLoadIntegrationTestCase,
)
from tests.integration.src.sqlbuild.executor.scenario.helpers import (
    SCENARIO_NAME,
    build_duckdb_assertion_check_plan,
    build_duckdb_cleanup_plan,
    build_duckdb_expected_check_plan,
    build_duckdb_fixture_plans,
    build_duckdb_invalid_fixture_plan,
    build_duckdb_model_execution_plan,
    create_table,
    insert_scenario_hook_log,
    relation_exists,
    relation_rows,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioFixtureMaterializationIntegrationTestCase(
            description="materializes source ref and seed fixtures in duckdb",
            expected_statuses=(
                ExecutionStatus.SUCCESS,
                ExecutionStatus.SUCCESS,
                ExecutionStatus.SUCCESS,
            ),
            expected_rows_by_relation={
                "__sqb_51b385aebe20__source__raw__orders": ((1, 10, "US"),),
                "__sqb_51b385aebe20__ref__stg_customers": ((10, "Ada"),),
                "__sqb_51b385aebe20__seed__country_codes": (("US", "United States"),),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_fixture_plans_when_executing_then_materializes_fixture_tables(
    test_case: ScenarioFixtureMaterializationIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    results: tuple[ScenarioFixtureExecutionResult, ...] = execute_scenario_fixtures(
        scenario_name=SCENARIO_NAME,
        fixture_plans=build_duckdb_fixture_plans(),
        adapter=adapter,
        connection=connection,
    )

    assert tuple(result.status for result in results) == test_case.expected_statuses
    for relation_name, expected_rows in test_case.expected_rows_by_relation.items():
        assert relation_rows(connection, relation_name) == expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCleanupIntegrationTestCase(
            description="drops planned relations and leaves unrelated prefix relation",
            expected_status=ExecutionStatus.SUCCESS,
            planned_relation_names=(
                "__sqb_51b385aebe20__source__raw__orders",
                "__sqb_51b385aebe20__ref__stg_customers",
                "__sqb_51b385aebe20__seed__country_codes",
                "__sqb_51b385aebe20__model__daily_revenue",
            ),
            retained_relation_name="__sqb_51b385aebe20__model__stale_not_in_plan",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_plan_when_cleaning_up_then_drops_only_planned_relations(
    test_case: ScenarioCleanupIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    scenario_plan: ScenarioExecutionPlan = build_duckdb_cleanup_plan()
    execute_scenario_fixtures(
        scenario_name=SCENARIO_NAME,
        fixture_plans=scenario_plan.fixture_plans,
        adapter=adapter,
        connection=connection,
    )
    create_table(connection, "__sqb_51b385aebe20__model__daily_revenue")
    create_table(connection, test_case.retained_relation_name)

    result: ScenarioCleanupExecutionResult = execute_scenario_cleanup(
        scenario_plan=scenario_plan,
        adapter=adapter,
        connection=connection,
    )

    assert result.status == test_case.expected_status
    for relation_name in test_case.planned_relation_names:
        assert not relation_exists(connection, relation_name)
    assert relation_exists(connection, test_case.retained_relation_name)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioFixtureFailureIntegrationTestCase(
            description="returns failed result for invalid fixture SQL",
            expected_status=ExecutionStatus.FAILED,
            expected_error_fragment="missing_relation",
            expected_log_fragment="revenue__customer_refund:source:raw__orders",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fixture_materialization_failure_when_executing_then_returns_failed_result(
    test_case: ScenarioFixtureFailureIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    fixture_plan: ScenarioFixturePlan = build_duckdb_invalid_fixture_plan()

    result: ScenarioFixtureExecutionResult = execute_scenario_fixture(
        scenario_name=SCENARIO_NAME,
        fixture_plan=fixture_plan,
        adapter=adapter,
        connection=connection,
    )

    assert result.status == test_case.expected_status
    assert result.error_message is not None
    assert test_case.expected_error_fragment in result.error_message
    assert test_case.expected_log_fragment in result.lifecycle_events[-1].content


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioProjectSeedLoadIntegrationTestCase(
            description="loads required unmocked project seed into scenario target",
            expected_statuses=(ExecutionStatus.SUCCESS,),
            expected_rows=(("US", "United States"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_required_unmocked_seed_when_executing_then_loads_project_seed_to_scenario_target(
    test_case: ScenarioProjectSeedLoadIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
    tmp_path: Path,
) -> None:
    seed_file: Path = tmp_path / "country_codes.csv"
    seed_file.write_text("country_code,country_name\nUS,United States\n", encoding="utf-8")
    seed_entry: SeedPlanEntry = SeedPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name="country_codes"),
        name="country_codes",
        destination=CompiledRelationLocation(
            database=None,
            schema="scenario_schema",
            name="__sqb_51b385aebe20__seed__country_codes",
            qualified_name="scenario_schema.__sqb_51b385aebe20__seed__country_codes",
        ),
        file_path=seed_file,
        columns=(
            ColumnInfo(name="country_code", type="VARCHAR"),
            ColumnInfo(name="country_name", type="VARCHAR"),
        ),
        csv_settings=DEFAULT_SEED_CSV_SETTINGS,
    )

    results: tuple[SeedExecutionResult, ...] = execute_scenario_seed_entries(
        scenario_name="project_seed_load",
        seed_entries=(seed_entry,),
        adapter=adapter,
        connection=connection,
    )

    assert tuple(result.status for result in results) == test_case.expected_statuses
    assert (
        relation_rows(connection, "__sqb_51b385aebe20__seed__country_codes")
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioModelBuildIntegrationTestCase(
            description="builds scenario model graph from fixture and seed relations",
            expected_statuses=(ExecutionStatus.SUCCESS, ExecutionStatus.SUCCESS),
            expected_rows=((1, "Ada", "United States"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_plan_when_executing_models_then_builds_model_relations(
    test_case: ScenarioModelBuildIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    scenario_plan: ScenarioExecutionPlan = build_duckdb_model_execution_plan(
        stg_orders_pre_hooks=(
            PythonHookEntry(name="insert_scenario_hook_log", kwargs={"model_name": "stg_orders"}),
        ),
        hook_functions=(
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("sql/hooks/python/scenario.py"),
                name="insert_scenario_hook_log",
                function=insert_scenario_hook_log,
            ),
        ),
    )
    fixture_results: tuple[ScenarioFixtureExecutionResult, ...] = execute_scenario_fixtures(
        scenario_name=SCENARIO_NAME,
        fixture_plans=scenario_plan.fixture_plans,
        adapter=adapter,
        connection=connection,
    )

    assert all(result.status == ExecutionStatus.SUCCESS for result in fixture_results)

    connection.execute(
        "CREATE TABLE scenario_schema.__sqb_51b385aebe20__seed__country_codes "
        "AS SELECT 'US' AS country_code, 'United States' AS country_name"
    )

    results: tuple[ModelExecutionResult, ...] = execute_scenario_models(
        scenario_plan=scenario_plan,
        adapter=adapter,
        connection=connection,
        run_id="run-1",
        promotion_mode=TablePromotionMode.STAGED,
    )

    assert tuple(result.status for result in results) == test_case.expected_statuses
    assert (
        relation_rows(connection, "__sqb_51b385aebe20__model__daily_revenue")
        == test_case.expected_rows
    )
    assert relation_rows(connection, "scenario_hook_log") == (("stg_orders", "pre_hooks"),)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioExpectedExpectationIntegrationTestCase(
            description="expected output matches scenario model relation",
            expected_sql=(
                "SELECT 1 AS order_id, 'Ada' AS customer_name, 'United States' AS country_name"
            ),
            expected_status=ExecutionStatus.SUCCESS,
            expected_actual_row_count=1,
            expected_expected_row_count=1,
            expected_mismatched_row_count=0,
        ),
        ScenarioExpectedExpectationIntegrationTestCase(
            description="expected output mismatch fails scenario check",
            expected_sql=(
                "SELECT 1 AS order_id, 'Grace' AS customer_name, 'United States' AS country_name"
            ),
            expected_status=ExecutionStatus.FAILED,
            expected_actual_row_count=1,
            expected_expected_row_count=1,
            expected_mismatched_row_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_expected_check_when_executing_then_returns_comparison_result(
    test_case: ScenarioExpectedExpectationIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    create_table(
        connection,
        "__sqb_51b385aebe20__model__daily_revenue",
        sql="SELECT 1 AS order_id, 'Ada' AS customer_name, 'United States' AS country_name",
    )
    scenario_plan: ScenarioExecutionPlan = build_duckdb_expected_check_plan(
        expected_sql=test_case.expected_sql
    )

    results: tuple[ScenarioExpectedExpectationExecutionResult, ...] = (
        execute_scenario_expected_expectations(
            scenario_plan=scenario_plan,
            adapter=adapter,
            connection=connection,
        )
    )

    assert len(results) == 1
    assert results[0].status == test_case.expected_status
    assert results[0].actual_row_count == test_case.expected_actual_row_count
    assert results[0].expected_row_count == test_case.expected_expected_row_count
    assert results[0].mismatched_row_count == test_case.expected_mismatched_row_count


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioAssertionExpectationIntegrationTestCase(
            description="zero-row assertion passes",
            assertion_sql=(
                "SELECT * FROM scenario_schema.__sqb_51b385aebe20__model__daily_revenue "
                "WHERE customer_name = 'Grace'"
            ),
            expected_status=ExecutionStatus.SUCCESS,
            expected_failing_row_count=0,
            expected_sample_rows=(),
        ),
        ScenarioAssertionExpectationIntegrationTestCase(
            description="assertion returning rows fails with sample rows",
            assertion_sql=(
                "SELECT order_id, customer_name "
                "FROM scenario_schema.__sqb_51b385aebe20__model__daily_revenue "
                "WHERE customer_name = 'Ada'"
            ),
            expected_status=ExecutionStatus.FAILED,
            expected_failing_row_count=1,
            expected_sample_rows=((1, "Ada"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_assertion_check_when_executing_then_returns_zero_row_result(
    test_case: ScenarioAssertionExpectationIntegrationTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    create_table(
        connection,
        "__sqb_51b385aebe20__model__daily_revenue",
        sql="SELECT 1 AS order_id, 'Ada' AS customer_name, 'United States' AS country_name",
    )
    scenario_plan: ScenarioExecutionPlan = build_duckdb_assertion_check_plan(
        assertion_sql=test_case.assertion_sql
    )

    results: tuple[ScenarioAssertionExpectationExecutionResult, ...] = (
        execute_scenario_assertion_expectations(
            scenario_plan=scenario_plan,
            adapter=adapter,
            connection=connection,
        )
    )

    assert len(results) == 1
    assert results[0].status == test_case.expected_status
    assert results[0].failing_row_count == test_case.expected_failing_row_count
    assert results[0].sample_rows == test_case.expected_sample_rows
