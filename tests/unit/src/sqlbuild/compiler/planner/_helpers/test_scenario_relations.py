from __future__ import annotations

from collections import defaultdict
from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledSqlScenario,
    CompileSqlScenarioCte,
    Dialect,
    DiscoveredHookFunction,
    PlannerInputError,
    ScenarioExecutionPlan,
    ScenarioFixturePlan,
    ScenarioGraphPlan,
    ScenarioRelationPlan,
    SqlLexicalSyntax,
    build_scenario_execution_plan,
    build_scenario_fixture_plans,
    build_scenario_relation_plan,
    project_with,
    resolve_scenario_check_sql,
)
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    ScenarioCheckSqlResolutionErrorTestCase,
    ScenarioCheckSqlResolutionTestCase,
    ScenarioDialectCheckSqlResolutionTestCase,
    ScenarioExecutionPlanTestCase,
    ScenarioFixturePlanTestCase,
    ScenarioFixtureSqlResolutionErrorTestCase,
    ScenarioRelationPlanErrorTestCase,
    ScenarioRelationPlanTestCase,
    ScenarioUnmockedSeedExecutionPlanTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import (
    PlannerTestAdapter,
    build_compiled_function,
    build_scenario_relation_test_map,
    build_scenario_relation_test_project,
    build_scenario_relation_test_project_with_unused_seed,
    build_scenario_relation_test_scenario,
    quoting_render_qualified_name,
)

HASH_PREFIX: str = "51b385aebe20"
SCENARIO_NAME: str = "revenue__customer_refund"
SCENARIO_PATH: str = "tests/scenarios/customer_refund.sql"
DAILY_REVENUE_TARGET: str = "scenario_schema.__sqb_51b385aebe20__model__daily_revenue"
STG_CUSTOMERS_TARGET: str = "scenario_schema.__sqb_51b385aebe20__ref__stg_customers"


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioRelationPlanTestCase(
            description="builds scenario relation locations for fixtures and models",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                source_fixture_names=("raw__orders",),
                ref_fixture_names=("stg_customers",),
                dbt_ref_fixture_names=("stripe__payments",),
                seed_names=("country_codes",),
                seed_fixture_names=("country_codes",),
            ),
            expected_model_target_names={
                "daily_revenue": "scenario_schema.__sqb_51b385aebe20__model__daily_revenue",
                "stg_customers": "scenario_schema.__sqb_51b385aebe20__ref__stg_customers",
            },
            expected_seed_target_names={
                "country_codes": "scenario_schema.__sqb_51b385aebe20__seed__country_codes",
            },
            expected_source_expressions={
                "raw__orders": None,
            },
            expected_dbt_ref_target_names={
                "stripe__payments": "scenario_schema.__sqb_51b385aebe20__dbt_ref__stripe__payments",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_graph_when_building_relation_plan_then_returns_scenario_targets(
    test_case: ScenarioRelationPlanTestCase,
) -> None:
    result: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=test_case.graph_plan,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    assert {
        name: target.qualified_name for name, target in result.model_locations.items()
    } == test_case.expected_model_target_names
    assert {
        name: target.qualified_name for name, target in result.seed_locations.items()
    } == test_case.expected_seed_target_names
    assert {
        name: source.expression for name, source in result.source_map.items()
    } == test_case.expected_source_expressions
    assert {
        name: target.qualified_name for name, target in result.dbt_ref_fixture_locations.items()
    } == test_case.expected_dbt_ref_target_names


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioRelationPlanTestCase(
            description="renders scenario relation names through the adapter renderer",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                seed_names=("country_codes",),
            ),
            expected_model_target_names={
                "daily_revenue": '"scenario_schema"."__sqb_51b385aebe20__model__daily_revenue"',
            },
            expected_seed_target_names={
                "country_codes": '"scenario_schema"."__sqb_51b385aebe20__seed__country_codes"',
            },
            expected_source_expressions={},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_quoting_renderer_when_building_relation_plan_then_renders_through_adapter(
    test_case: ScenarioRelationPlanTestCase,
) -> None:
    result: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=test_case.graph_plan,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=quoting_render_qualified_name,
        schema="scenario_schema",
    )

    assert {
        name: target.qualified_name for name, target in result.model_locations.items()
    } == test_case.expected_model_target_names
    assert {
        name: target.qualified_name for name, target in result.seed_locations.items()
    } == test_case.expected_seed_target_names


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioFixturePlanTestCase(
            description="wraps each fixture with only the scenario helper CTEs it reads",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                source_fixture_names=("raw__orders",),
                ref_fixture_names=("stg_customers",),
                dbt_ref_fixture_names=("stripe__payments",),
                seed_names=("country_codes",),
                seed_fixture_names=("country_codes",),
            ),
            expected_fixture_sql={
                "source:raw__orders": (
                    "WITH helper_orders AS (SELECT 1 AS order_id, 10 AS customer_id) "
                    "SELECT * FROM helper_orders"
                ),
                "ref:stg_customers": "SELECT 10 AS customer_id",
                "dbt_ref:stripe__payments": "SELECT 1 AS payment_id, 10 AS customer_id",
                "seed:country_codes": "SELECT 'US' AS country_code",
            },
            expected_fixture_targets={
                "source:raw__orders": "scenario_schema.__sqb_51b385aebe20__source__raw__orders",
                "ref:stg_customers": "scenario_schema.__sqb_51b385aebe20__ref__stg_customers",
                "dbt_ref:stripe__payments": (
                    "scenario_schema.__sqb_51b385aebe20__dbt_ref__stripe__payments"
                ),
                "seed:country_codes": "scenario_schema.__sqb_51b385aebe20__seed__country_codes",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_helpers_when_building_fixture_plans_then_fixtures_are_self_contained(
    test_case: ScenarioFixturePlanTestCase,
) -> None:
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=test_case.graph_plan,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    result: tuple[ScenarioFixturePlan, ...] = build_scenario_fixture_plans(
        scenario=build_scenario_relation_test_scenario(),
        graph_plan=test_case.graph_plan,
        relation_plan=relation_plan,
        dialect=Dialect(PlannerTestAdapter()),
        source_lexical_syntax=SqlLexicalSyntax(),
    )

    assert {
        f"{fixture.kind.value}:{fixture.logical_name}": fixture.sql for fixture in result
    } == test_case.expected_fixture_sql
    assert {
        f"{fixture.kind.value}:{fixture.logical_name}": fixture.destination.qualified_name
        for fixture in result
    } == test_case.expected_fixture_targets


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioFixturePlanTestCase(
            description="resolves project source refs in scenario fixture sql",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                source_fixture_names=("raw__orders",),
            ),
            expected_fixture_sql={
                "source:raw__orders": "SELECT * FROM public.raw__orders WHERE order_id <= 10",
            },
            expected_fixture_targets={
                "source:raw__orders": "scenario_schema.__sqb_51b385aebe20__source__raw__orders",
            },
            fixture_sql_body='SELECT * FROM __source("raw__orders") WHERE order_id <= 10',
        ),
        ScenarioFixturePlanTestCase(
            description="resolves project source refs in authored sql without touching literals",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                source_fixture_names=("raw__orders",),
            ),
            expected_fixture_sql={
                "source:raw__orders": (
                    "SELECT '__source(\"raw__orders\")' AS marker_text "
                    'FROM public.raw__orders o -- __source("raw__orders")'
                ),
            },
            expected_fixture_targets={
                "source:raw__orders": "scenario_schema.__sqb_51b385aebe20__source__raw__orders",
            },
            fixture_sql_body=(
                "SELECT '__source(\"raw__orders\")' AS marker_text "
                'FROM __source("raw__orders") o -- __source("raw__orders")'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_source_ref_in_scenario_fixture_when_building_fixture_plan_then_resolves(
    test_case: ScenarioFixturePlanTestCase,
) -> None:
    scenario: CompiledSqlScenario = replace(
        build_scenario_relation_test_scenario(include_seed_fixture=False),
        authored_ctes=(
            CompileSqlScenarioCte(
                name="__source__raw__orders",
                sql_body=test_case.fixture_sql_body or "SELECT 1",
            ),
        ),
        source_fixture_names=("raw__orders",),
        ref_fixture_names=(),
        dbt_ref_fixture_names=(),
        seed_fixture_names=(),
    )
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=test_case.graph_plan,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    result: tuple[ScenarioFixturePlan, ...] = build_scenario_fixture_plans(
        scenario=scenario,
        graph_plan=test_case.graph_plan,
        relation_plan=relation_plan,
        dialect=Dialect(PlannerTestAdapter()),
        source_lexical_syntax=SqlLexicalSyntax(),
    )

    assert {
        f"{fixture.kind.value}:{fixture.logical_name}": fixture.sql for fixture in result
    } == test_case.expected_fixture_sql
    assert {
        f"{fixture.kind.value}:{fixture.logical_name}": fixture.destination.qualified_name
        for fixture in result
    } == test_case.expected_fixture_targets


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioExecutionPlanTestCase(
            description="builds dry run scenario execution plan with scenario targets",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                assertion_target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                source_fixture_names=("raw__orders",),
                ref_fixture_names=("stg_customers",),
                dbt_ref_fixture_names=("stripe__payments",),
                seed_names=("country_codes",),
                seed_fixture_names=("country_codes",),
                function_deps=(build_compiled_function(body_sql="").key,),
            ),
            expected_model_entry_targets={
                "daily_revenue": "scenario_schema.__sqb_51b385aebe20__model__daily_revenue",
            },
            expected_model_entry_sql_fragments={
                "daily_revenue": (
                    "scenario_schema.__sqb_51b385aebe20__source__raw__orders",
                    "scenario_schema.__sqb_51b385aebe20__ref__stg_customers",
                    "scenario_schema.__sqb_51b385aebe20__seed__country_codes",
                    "scenario_schema.__sqb_51b385aebe20__dbt_ref__stripe__payments",
                ),
            },
            expected_fixture_targets={
                "source:raw__orders": "scenario_schema.__sqb_51b385aebe20__source__raw__orders",
                "ref:stg_customers": "scenario_schema.__sqb_51b385aebe20__ref__stg_customers",
                "dbt_ref:stripe__payments": (
                    "scenario_schema.__sqb_51b385aebe20__dbt_ref__stripe__payments"
                ),
                "seed:country_codes": "scenario_schema.__sqb_51b385aebe20__seed__country_codes",
            },
            expected_seed_entry_targets={},
            expected_function_entry_targets={
                "is_completed_order": "main.is_completed_order",
            },
            expected_function_entry_sql_fragments={
                "is_completed_order": (
                    "scenario_schema.__sqb_51b385aebe20__source__raw__orders",
                    "scenario_schema.__sqb_51b385aebe20__model__daily_revenue",
                ),
            },
            expected_expected_actual_destinations={
                "daily_revenue": "scenario_schema.__sqb_51b385aebe20__model__daily_revenue",
            },
            expected_expected_sql={
                "daily_revenue": (
                    "SELECT * FROM scenario_schema.__sqb_51b385aebe20__model__daily_revenue"
                ),
            },
            expected_assertion_sql={
                "no_negative_revenue": (
                    "SELECT * FROM "
                    "scenario_schema.__sqb_51b385aebe20__model__daily_revenue "
                    "WHERE revenue < 0"
                ),
            },
            expected_hook_names=("notify",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_graph_when_building_execution_plan_then_returns_scenario_plan(
    test_case: ScenarioExecutionPlanTestCase,
) -> None:
    base_project: CompiledProjectView = build_scenario_relation_test_project()

    def notify() -> None:
        return None

    project: CompiledProjectView = project_with(
        project=base_project,
        functions=(
            build_compiled_function(
                body_sql=(
                    'EXISTS (SELECT 1 FROM __source("raw__orders")) '
                    'AND EXISTS (SELECT 1 FROM __ref("daily_revenue"))'
                )
            ),
        ),
        hook_functions=(
            DiscoveredHookFunction(
                file_path=Path(__file__),
                relative_path=Path("sql/hooks/python/notify.py"),
                name="notify",
                function=notify,
            ),
        ),
    )
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=project,
        graph_plan=test_case.graph_plan,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    result, warnings = build_scenario_execution_plan(
        scenario=build_scenario_relation_test_scenario(),
        project=project,
        dialect=Dialect(PlannerTestAdapter()),
        source_lexical_syntax=SqlLexicalSyntax(),
        graph_plan=test_case.graph_plan,
        relation_plan=relation_plan,
    )

    assert warnings == ()
    assert isinstance(result, ScenarioExecutionPlan)
    assert {
        entry.name: entry.destination.qualified_name for entry in result.model_entries
    } == test_case.expected_model_entry_targets
    for entry in result.model_entries:
        for expected_fragment in test_case.expected_model_entry_sql_fragments[entry.name]:
            assert expected_fragment in entry.resolved_sql
    assert {
        f"{fixture.kind.value}:{fixture.logical_name}": fixture.destination.qualified_name
        for fixture in result.fixture_plans
    } == test_case.expected_fixture_targets
    assert {
        entry.name: entry.destination.qualified_name for entry in result.seed_entries
    } == test_case.expected_seed_entry_targets
    assert {
        entry.name: entry.destination.qualified_name for entry in result.function_entries
    } == test_case.expected_function_entry_targets
    for entry in result.function_entries:
        for expected_fragment in test_case.expected_function_entry_sql_fragments[entry.name]:
            assert expected_fragment in entry.body_sql
    assert {
        expectation.model_name: expectation.actual_destination.qualified_name
        for expectation in result.expected_expectations
    } == test_case.expected_expected_actual_destinations
    assert {
        expectation.model_name: expectation.expected_sql
        for expectation in result.expected_expectations
    } == test_case.expected_expected_sql
    assert {
        expectation.name: expectation.sql for expectation in result.assertion_expectations
    } == test_case.expected_assertion_sql
    assert tuple(hook.name for hook in result.hook_functions) == test_case.expected_hook_names


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioUnmockedSeedExecutionPlanTestCase(
            description="loads required unmocked seed from project seed file",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                assertion_target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                source_fixture_names=("raw__orders",),
                ref_fixture_names=("stg_customers",),
                seed_names=("country_codes",),
            ),
            project=build_scenario_relation_test_project(),
            expected_seed_fixture_names=frozenset(),
            expected_seed_entry_targets={
                "country_codes": "scenario_schema.__sqb_51b385aebe20__seed__country_codes"
            },
        ),
        ScenarioUnmockedSeedExecutionPlanTestCase(
            description="ignores project seeds outside the scenario graph",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                assertion_target_model_names=("daily_revenue",),
                model_names=("daily_revenue",),
                source_fixture_names=("raw__orders",),
                ref_fixture_names=("stg_customers",),
                seed_names=("country_codes",),
            ),
            project=build_scenario_relation_test_project_with_unused_seed(),
            expected_seed_fixture_names=frozenset(),
            expected_seed_entry_targets={
                "country_codes": "scenario_schema.__sqb_51b385aebe20__seed__country_codes"
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_required_unmocked_seed_when_building_execution_plan_then_loads_project_seed(
    test_case: ScenarioUnmockedSeedExecutionPlanTestCase,
) -> None:
    project: CompiledProjectView = test_case.project
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=project,
        graph_plan=test_case.graph_plan,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    result, warnings = build_scenario_execution_plan(
        scenario=build_scenario_relation_test_scenario(include_seed_fixture=False),
        project=project,
        dialect=Dialect(PlannerTestAdapter()),
        source_lexical_syntax=SqlLexicalSyntax(),
        graph_plan=test_case.graph_plan,
        relation_plan=relation_plan,
    )

    assert warnings == ()
    fixture_names_by_kind: defaultdict[str, set[str]] = defaultdict(set)
    for fixture in result.fixture_plans:
        fixture_names_by_kind[fixture.kind.value].add(fixture.logical_name)
    assert fixture_names_by_kind["seed"] == test_case.expected_seed_fixture_names
    assert {
        entry.name: entry.destination.qualified_name for entry in result.seed_entries
    } == test_case.expected_seed_entry_targets


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCheckSqlResolutionTestCase(
            description="resolves assertion refs to scenario model and ref fixture relations",
            sql=(
                'SELECT * FROM __ref("daily_revenue") dr '
                'JOIN __ref("stg_customers") sc ON dr.customer_id = sc.customer_id'
            ),
            expected_sql=(
                "SELECT * FROM scenario_schema.__sqb_51b385aebe20__model__daily_revenue dr "
                "JOIN scenario_schema.__sqb_51b385aebe20__ref__stg_customers sc "
                "ON dr.customer_id = sc.customer_id"
            ),
        ),
        ScenarioCheckSqlResolutionTestCase(
            description="resolves seed and source markers to scenario fixture relations",
            sql=(
                'SELECT * FROM __seed("country_codes") c '
                'JOIN __source("raw__orders") o ON c.country_code = o.country_code'
            ),
            expected_sql=(
                "SELECT * FROM scenario_schema.__sqb_51b385aebe20__seed__country_codes c "
                "JOIN scenario_schema.__sqb_51b385aebe20__source__raw__orders o "
                "ON c.country_code = o.country_code"
            ),
        ),
        ScenarioCheckSqlResolutionTestCase(
            description="resolves dbt ref markers to scenario fixture relations",
            sql='SELECT * FROM __dbt_ref("stripe", "payments") p',
            expected_sql=(
                "SELECT * FROM scenario_schema.__sqb_51b385aebe20__dbt_ref__stripe__payments p"
            ),
        ),
        ScenarioCheckSqlResolutionTestCase(
            description="check sql resolution keeps strings and comments as authored",
            sql=(
                "SELECT '__ref(\"daily_revenue\")' AS marker_text "
                'FROM __ref("daily_revenue") dr -- __source("raw__orders")'
            ),
            expected_sql=(
                "SELECT '__ref(\"daily_revenue\")' AS marker_text "
                "FROM scenario_schema.__sqb_51b385aebe20__model__daily_revenue dr "
                '-- __source("raw__orders")'
            ),
        ),
        ScenarioCheckSqlResolutionTestCase(
            description="authored function spellings and keyword case are not regenerated",
            sql=(
                "select STARTSWITH(name, 'a') as flagged, IFNULL(total, 0) as total "
                'from __ref("daily_revenue") where total != 0'
            ),
            expected_sql=(
                "select STARTSWITH(name, 'a') as flagged, IFNULL(total, 0) as total "
                "from scenario_schema.__sqb_51b385aebe20__model__daily_revenue where total != 0"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_check_sql_when_resolving_then_uses_scenario_relations(
    test_case: ScenarioCheckSqlResolutionTestCase,
) -> None:
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=ScenarioGraphPlan(
            key=build_scenario_relation_test_project().models[0].key,
            name=SCENARIO_NAME,
            target_model_names=("daily_revenue",),
            model_names=("daily_revenue",),
            source_fixture_names=("raw__orders",),
            ref_fixture_names=("stg_customers",),
            dbt_ref_fixture_names=("stripe__payments",),
            seed_names=("country_codes",),
            seed_fixture_names=("country_codes",),
        ),
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    result: str = resolve_scenario_check_sql(
        sql=test_case.sql,
        relation_plan=relation_plan,
        dialect=Dialect(PlannerTestAdapter()),
        source_lexical_syntax=SqlLexicalSyntax(),
        source_label=SCENARIO_PATH,
    )

    assert result == test_case.expected_sql


BACKSLASH_LEXICAL_SYNTAXES: tuple[SqlLexicalSyntax, ...] = (
    SnowflakeAdapter.sql_lexical_syntax,
    BigQueryAdapter.sql_lexical_syntax,
    DatabricksAdapter.sql_lexical_syntax,
)
ESCAPE_LEXICAL_SYNTAXES: tuple[SqlLexicalSyntax, ...] = (
    DuckDbAdapter.sql_lexical_syntax,
    PostgresAdapter.sql_lexical_syntax,
)
CHECK_SQL_GRAPH_PLAN: ScenarioGraphPlan = ScenarioGraphPlan(
    key=build_scenario_relation_test_project().models[0].key,
    name=SCENARIO_NAME,
    target_model_names=("daily_revenue",),
    model_names=("daily_revenue",),
    source_fixture_names=("raw__orders",),
    ref_fixture_names=("stg_customers",),
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioDialectCheckSqlResolutionTestCase(
            description="backslash-escaped quote before a later marker",
            lexical_syntaxes=BACKSLASH_LEXICAL_SYNTAXES,
            sql=(
                "select * from __ref(\"daily_revenue\") where note = 'O\\'Brien' "
                'union all select * from __ref("stg_customers")'
            ),
            expected_sql=(
                f"select * from {DAILY_REVENUE_TARGET} where note = 'O\\'Brien' "
                f"union all select * from {STG_CUSTOMERS_TARGET}"
            ),
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="even backslash-escaped quotes around a marker",
            lexical_syntaxes=BACKSLASH_LEXICAL_SYNTAXES,
            sql=(
                "select * from __ref(\"daily_revenue\") where note = 'O\\'Brien' "
                "union all select * from __ref(\"stg_customers\") where note = 'D\\'Arcy'"
            ),
            expected_sql=(
                f"select * from {DAILY_REVENUE_TARGET} where note = 'O\\'Brien' "
                f"union all select * from {STG_CUSTOMERS_TARGET} where note = 'D\\'Arcy'"
            ),
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="escape strings around a marker",
            lexical_syntaxes=ESCAPE_LEXICAL_SYNTAXES,
            sql=(
                "select * from __ref(\"daily_revenue\") where note = E'O\\'Brien' "
                "union all select * from __ref(\"stg_customers\") where note = e'D\\'Arcy'"
            ),
            expected_sql=(
                f"select * from {DAILY_REVENUE_TARGET} where note = E'O\\'Brien' "
                f"union all select * from {STG_CUSTOMERS_TARGET} where note = e'D\\'Arcy'"
            ),
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="dollar-quoted text hides quotes and markers",
            lexical_syntaxes=(SnowflakeAdapter.sql_lexical_syntax, *ESCAPE_LEXICAL_SYNTAXES),
            sql='select $$O\'Brien __ref("stg_customers")$$ as note from __ref("daily_revenue")',
            expected_sql=(
                f'select $$O\'Brien __ref("stg_customers")$$ as note from {DAILY_REVENUE_TARGET}'
            ),
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="snowflake quoted identifier ending in a backslash",
            lexical_syntaxes=(SnowflakeAdapter.sql_lexical_syntax,),
            sql='select "note\\", "it""s" from __ref("daily_revenue")',
            expected_sql=f'select "note\\", "it""s" from {DAILY_REVENUE_TARGET}',
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="bigquery backtick identifier with an escaped backtick",
            lexical_syntaxes=(BigQueryAdapter.sql_lexical_syntax,),
            sql='select `note\\`s` from __ref("daily_revenue")',
            expected_sql=f"select `note\\`s` from {DAILY_REVENUE_TARGET}",
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="bigquery triple-quoted string with an apostrophe",
            lexical_syntaxes=(BigQueryAdapter.sql_lexical_syntax,),
            sql="select '''it's''' as note from __ref(\"daily_revenue\")",
            expected_sql=f"select '''it's''' as note from {DAILY_REVENUE_TARGET}",
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="nested block comment with an apostrophe",
            lexical_syntaxes=(
                PostgresAdapter.sql_lexical_syntax,
                DuckDbAdapter.sql_lexical_syntax,
                DatabricksAdapter.sql_lexical_syntax,
                SqlServerAdapter.sql_lexical_syntax,
            ),
            sql='select * /* a /* b */ it\'s __ref("stg_customers") */ from __ref("daily_revenue")',
            expected_sql=(
                f'select * /* a /* b */ it\'s __ref("stg_customers") */ from {DAILY_REVENUE_TARGET}'
            ),
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="bigquery hash line comment with an apostrophe",
            lexical_syntaxes=(BigQueryAdapter.sql_lexical_syntax,),
            sql='select * # it\'s __ref("stg_customers")\nfrom __ref("daily_revenue")',
            expected_sql=f'select * # it\'s __ref("stg_customers")\nfrom {DAILY_REVENUE_TARGET}',
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="snowflake double slash line comment with an apostrophe",
            lexical_syntaxes=(SnowflakeAdapter.sql_lexical_syntax,),
            sql='select * // it\'s __ref("stg_customers")\nfrom __ref("daily_revenue")',
            expected_sql=f'select * // it\'s __ref("stg_customers")\nfrom {DAILY_REVENUE_TARGET}',
        ),
        ScenarioDialectCheckSqlResolutionTestCase(
            description="databricks raw string ending in a backslash",
            lexical_syntaxes=(DatabricksAdapter.sql_lexical_syntax,),
            sql="select r'C:\\' as path from __ref(\"daily_revenue\")",
            expected_sql=f"select r'C:\\' as path from {DAILY_REVENUE_TARGET}",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_lexical_syntax_when_resolving_check_sql_then_replaces_every_marker(
    test_case: ScenarioDialectCheckSqlResolutionTestCase,
) -> None:
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=CHECK_SQL_GRAPH_PLAN,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    results: tuple[str, ...] = tuple(
        resolve_scenario_check_sql(
            sql=test_case.sql,
            relation_plan=relation_plan,
            dialect=Dialect(PlannerTestAdapter()),
            source_lexical_syntax=lexical_syntax,
            source_label=SCENARIO_PATH,
        )
        for lexical_syntax in test_case.lexical_syntaxes
    )

    assert results == tuple(test_case.expected_sql for _ in test_case.lexical_syntaxes)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioCheckSqlResolutionErrorTestCase(
            description="backslash before a quote is not an escape outside escape strings",
            lexical_syntaxes=ESCAPE_LEXICAL_SYNTAXES,
            sql=(
                "select * from __ref(\"daily_revenue\") where note = 'O\\'Brien' "
                'union all select * from __ref("stg_customers")'
            ),
            expected_error_code="S511",
            expected_error_fragment="unclosed quoted string",
        ),
        ScenarioCheckSqlResolutionErrorTestCase(
            description="escaped closing quote leaves the string unclosed",
            lexical_syntaxes=BACKSLASH_LEXICAL_SYNTAXES,
            sql="select * from __ref(\"daily_revenue\") where note = 'O\\'",
            expected_error_code="S511",
            expected_error_fragment="unclosed quoted string",
        ),
        ScenarioCheckSqlResolutionErrorTestCase(
            description="non-nesting block comment ends before the apostrophe",
            lexical_syntaxes=(
                SnowflakeAdapter.sql_lexical_syntax,
                BigQueryAdapter.sql_lexical_syntax,
            ),
            sql='select * /* a /* b */ it\'s */ from __ref("daily_revenue")',
            expected_error_code="S511",
            expected_error_fragment="unclosed quoted string",
        ),
        ScenarioCheckSqlResolutionErrorTestCase(
            description="marker naming a relation outside the scenario",
            lexical_syntaxes=(SnowflakeAdapter.sql_lexical_syntax,),
            sql='select * from __ref("daily_revenue") join __ref("unknown_orders") using (id)',
            expected_error_code="S511",
            expected_error_fragment="references '__ref(\"unknown_orders\")'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unresolvable_check_sql_when_resolving_then_raises_scenario_error(
    test_case: ScenarioCheckSqlResolutionErrorTestCase,
) -> None:
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=CHECK_SQL_GRAPH_PLAN,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    for lexical_syntax in test_case.lexical_syntaxes:
        with pytest.raises(PlannerInputError) as error_info:
            resolve_scenario_check_sql(
                sql=test_case.sql,
                relation_plan=relation_plan,
                dialect=Dialect(PlannerTestAdapter()),
                source_lexical_syntax=lexical_syntax,
                source_label=SCENARIO_PATH,
            )

        assert error_info.value.code == test_case.expected_error_code
        assert SCENARIO_PATH in error_info.value.message
        assert test_case.expected_error_fragment in error_info.value.message


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioFixtureSqlResolutionErrorTestCase(
            description="unclosed string in duckdb fixture sql",
            lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
            fixture_sql="SELECT 'O\\'Brien' AS note FROM __source(\"raw__orders\")",
            expected_error_code="S511",
            expected_error_fragment="unclosed quoted string",
        ),
        ScenarioFixtureSqlResolutionErrorTestCase(
            description="fixture sql naming an unknown project source",
            lexical_syntax=SnowflakeAdapter.sql_lexical_syntax,
            fixture_sql='SELECT * FROM __source("raw__returns")',
            expected_error_code="S511",
            expected_error_fragment="references '__source(\"raw__returns\")'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unresolvable_fixture_sql_when_planning_fixtures_then_raises_naming_file(
    test_case: ScenarioFixtureSqlResolutionErrorTestCase,
) -> None:
    scenario: CompiledSqlScenario = replace(
        build_scenario_relation_test_scenario(include_seed_fixture=False),
        authored_ctes=(
            CompileSqlScenarioCte(name="__source__raw__orders", sql_body=test_case.fixture_sql),
        ),
        source_fixture_names=("raw__orders",),
        ref_fixture_names=(),
        dbt_ref_fixture_names=(),
        seed_fixture_names=(),
        source_path=Path(SCENARIO_PATH),
    )
    graph_plan: ScenarioGraphPlan = replace(CHECK_SQL_GRAPH_PLAN, ref_fixture_names=())
    relation_plan: ScenarioRelationPlan = build_scenario_relation_plan(
        project=build_scenario_relation_test_project(),
        graph_plan=graph_plan,
        relation_map=build_scenario_relation_test_map(),
        render_qualified_name=PlannerTestAdapter().render_qualified_name,
        schema="scenario_schema",
    )

    with pytest.raises(PlannerInputError) as error_info:
        build_scenario_fixture_plans(
            scenario=scenario,
            graph_plan=graph_plan,
            relation_plan=relation_plan,
            dialect=Dialect(PlannerTestAdapter()),
            source_lexical_syntax=test_case.lexical_syntax,
        )

    assert error_info.value.code == test_case.expected_error_code
    assert SCENARIO_PATH in error_info.value.message
    assert test_case.expected_error_fragment in error_info.value.message


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioRelationPlanErrorTestCase(
            description="raises when relation map is missing required artifact",
            graph_plan=ScenarioGraphPlan(
                key=build_scenario_relation_test_project().models[0].key,
                name=SCENARIO_NAME,
                target_model_names=("daily_revenue",),
                model_names=("daily_revenue", "missing_model"),
            ),
            expected_error_fragment="missing model artifact 'missing_model'",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_scenario_artifact_when_building_relation_plan_then_raises(
    test_case: ScenarioRelationPlanErrorTestCase,
) -> None:
    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        build_scenario_relation_plan(
            project=build_scenario_relation_test_project(),
            graph_plan=test_case.graph_plan,
            relation_map=build_scenario_relation_test_map(),
            render_qualified_name=PlannerTestAdapter().render_qualified_name,
            schema="scenario_schema",
        )
