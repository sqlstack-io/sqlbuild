from __future__ import annotations

from pathlib import Path
from typing import Any

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompilePipelineOptions,
    CompilePipelineResult,
    DiscoveredHookFunction,
    DiscoveredProjectInputs,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    ScenarioArtifactKind,
    ScenarioAssertionExpectationPlan,
    ScenarioExecutionPlan,
    ScenarioExpectedExpectationPlan,
    ScenarioFixturePlan,
    ScenarioGraphPlan,
    ScenarioRelationMap,
    ScenarioRelationPlan,
    discover_project_inputs,
    run_compile_pipeline,
)
from sqlbuild.executor.run.models import HookContext
from sqlbuild.runtime.contracts.models import ConnectionHooks
from tests.integration.src.sqlbuild.executor.build.snowflake_metadata.helpers import (
    SimulatedSnowflakeWarehouse,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeColumn,
    FakeRelation,
    OfflineSnowflakeAdapter,
)

SCENARIO_NAME: str = "revenue__customer_refund"
SCHEMA_NAME: str = "scenario_schema"
HASH_PREFIX: str = "51b385aebe20"


def insert_scenario_hook_log(ctx: HookContext, model_name: str) -> None:
    ctx.execute_sql(
        f"CREATE TABLE {ctx.destination.schema}.scenario_hook_log AS "
        f"SELECT '{model_name}' AS model_name, '{ctx.phase}' AS phase"
    )


def build_duckdb_fixture_plans() -> tuple[ScenarioFixturePlan, ...]:
    return (
        _fixture_plan(
            kind=ScenarioArtifactKind.SOURCE,
            logical_name="raw__orders",
            physical_name="__sqb_51b385aebe20__source__raw__orders",
            sql=(
                "WITH helper_orders AS ("
                "SELECT 1 AS order_id, 10 AS customer_id, 'US' AS country_code"
                ") SELECT * FROM helper_orders"
            ),
        ),
        _fixture_plan(
            kind=ScenarioArtifactKind.REF,
            logical_name="stg_customers",
            physical_name="__sqb_51b385aebe20__ref__stg_customers",
            sql="SELECT 10 AS customer_id, 'Ada' AS customer_name",
        ),
        _fixture_plan(
            kind=ScenarioArtifactKind.SEED,
            logical_name="country_codes",
            physical_name="__sqb_51b385aebe20__seed__country_codes",
            sql="SELECT 'US' AS country_code, 'United States' AS country_name",
        ),
    )


def build_duckdb_invalid_fixture_plan() -> ScenarioFixturePlan:
    return _fixture_plan(
        kind=ScenarioArtifactKind.SOURCE,
        logical_name="raw__orders",
        physical_name="__sqb_51b385aebe20__source__raw__orders",
        sql="SELECT * FROM missing_relation",
    )


def build_duckdb_cleanup_plan() -> ScenarioExecutionPlan:
    fixture_plans: tuple[ScenarioFixturePlan, ...] = build_duckdb_fixture_plans()
    ref_fixture: ScenarioFixturePlan = fixture_plans[1]
    model_target: CompiledRelationLocation = _target(
        physical_name="__sqb_51b385aebe20__model__daily_revenue"
    )
    return ScenarioExecutionPlan(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name=SCENARIO_NAME,
        ),
        name=SCENARIO_NAME,
        graph_plan=ScenarioGraphPlan(
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.SQL_SCENARIO,
                name=SCENARIO_NAME,
            ),
            name=SCENARIO_NAME,
            model_names=("daily_revenue",),
        ),
        relation_plan=ScenarioRelationPlan(
            scenario_name=SCENARIO_NAME,
            relation_map=ScenarioRelationMap(
                scenario_name=SCENARIO_NAME,
                hash_prefix=HASH_PREFIX,
            ),
            model_locations={
                "daily_revenue": model_target,
                "stg_customers": ref_fixture.destination,
            },
            ref_fixture_locations={"stg_customers": ref_fixture.destination},
        ),
        fixture_plans=fixture_plans,
    )


def build_duckdb_model_execution_plan(
    *,
    stg_orders_pre_hooks: object = None,
    hook_functions: tuple[DiscoveredHookFunction, ...] = (),
) -> ScenarioExecutionPlan:
    source_target: CompiledRelationLocation = _target(
        physical_name="__sqb_51b385aebe20__source__raw__orders"
    )
    ref_target: CompiledRelationLocation = _target(
        physical_name="__sqb_51b385aebe20__ref__stg_customers"
    )
    seed_target: CompiledRelationLocation = _target(
        physical_name="__sqb_51b385aebe20__seed__country_codes"
    )
    stg_orders_target: CompiledRelationLocation = _target(
        physical_name="__sqb_51b385aebe20__model__stg_orders"
    )
    daily_revenue_target: CompiledRelationLocation = _target(
        physical_name="__sqb_51b385aebe20__model__daily_revenue"
    )
    stg_orders: ModelPlanEntry = _model_entry(
        name="stg_orders",
        target=stg_orders_target,
        sql=(
            "SELECT o.order_id, o.customer_id, c.country_name "
            "FROM scenario_schema.__sqb_51b385aebe20__source__raw__orders o "
            "JOIN scenario_schema.__sqb_51b385aebe20__seed__country_codes c "
            "ON o.country_code = c.country_code"
        ),
        pre_hooks=stg_orders_pre_hooks,
    )
    daily_revenue: ModelPlanEntry = _model_entry(
        name="daily_revenue",
        target=daily_revenue_target,
        sql=(
            "SELECT o.order_id, c.customer_name, o.country_name "
            "FROM scenario_schema.__sqb_51b385aebe20__model__stg_orders o "
            "JOIN scenario_schema.__sqb_51b385aebe20__ref__stg_customers c "
            "ON o.customer_id = c.customer_id"
        ),
    )
    return ScenarioExecutionPlan(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name=SCENARIO_NAME,
        ),
        name=SCENARIO_NAME,
        graph_plan=ScenarioGraphPlan(
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.SQL_SCENARIO,
                name=SCENARIO_NAME,
            ),
            name=SCENARIO_NAME,
            model_names=("stg_orders", "daily_revenue"),
            source_fixture_names=("raw__orders",),
            ref_fixture_names=("stg_customers",),
            seed_names=("country_codes",),
        ),
        relation_plan=ScenarioRelationPlan(
            scenario_name=SCENARIO_NAME,
            relation_map=ScenarioRelationMap(
                scenario_name=SCENARIO_NAME,
                hash_prefix=HASH_PREFIX,
            ),
            model_locations={
                "stg_orders": stg_orders_target,
                "daily_revenue": daily_revenue_target,
                "stg_customers": ref_target,
            },
            seed_locations={"country_codes": seed_target},
            source_fixture_locations={"raw__orders": source_target},
            ref_fixture_locations={"stg_customers": ref_target},
        ),
        fixture_plans=build_duckdb_fixture_plans()[:2],
        model_entries=(stg_orders, daily_revenue),
        hook_functions=hook_functions,
    )


def build_duckdb_expected_check_plan(*, expected_sql: str) -> ScenarioExecutionPlan:
    model_target: CompiledRelationLocation = _target(
        physical_name="__sqb_51b385aebe20__model__daily_revenue"
    )
    return ScenarioExecutionPlan(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name=SCENARIO_NAME,
        ),
        name=SCENARIO_NAME,
        graph_plan=ScenarioGraphPlan(
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.SQL_SCENARIO,
                name=SCENARIO_NAME,
            ),
            name=SCENARIO_NAME,
            model_names=("daily_revenue",),
        ),
        relation_plan=ScenarioRelationPlan(
            scenario_name=SCENARIO_NAME,
            relation_map=ScenarioRelationMap(
                scenario_name=SCENARIO_NAME,
                hash_prefix=HASH_PREFIX,
            ),
            model_locations={"daily_revenue": model_target},
        ),
        expected_expectations=(
            ScenarioExpectedExpectationPlan(
                model_name="daily_revenue",
                actual_destination=model_target,
                expected_sql=expected_sql,
            ),
        ),
    )


def build_duckdb_assertion_check_plan(*, assertion_sql: str) -> ScenarioExecutionPlan:
    return ScenarioExecutionPlan(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name=SCENARIO_NAME,
        ),
        name=SCENARIO_NAME,
        graph_plan=ScenarioGraphPlan(
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.SQL_SCENARIO,
                name=SCENARIO_NAME,
            ),
            name=SCENARIO_NAME,
            model_names=("daily_revenue",),
        ),
        relation_plan=ScenarioRelationPlan(
            scenario_name=SCENARIO_NAME,
            relation_map=ScenarioRelationMap(
                scenario_name=SCENARIO_NAME,
                hash_prefix=HASH_PREFIX,
            ),
        ),
        assertion_expectations=(
            ScenarioAssertionExpectationPlan(
                name="no_unknown_customers",
                sql=assertion_sql,
            ),
        ),
    )


def relation_rows(connection: Any, relation_name: str) -> tuple[tuple[object, ...], ...]:
    rows: list[Any] = connection.execute(f"SELECT * FROM {SCHEMA_NAME}.{relation_name}").fetchall()
    return tuple(tuple(row) for row in rows)


def relation_exists(connection: Any, relation_name: str) -> bool:
    row: Any | None = connection.execute(
        "SELECT 1 FROM information_schema.tables WHERE table_schema = ? AND table_name = ?",
        [SCHEMA_NAME, relation_name],
    ).fetchone()
    return row is not None


def create_table(connection: Any, relation_name: str, sql: str = "SELECT 1 AS value") -> None:
    connection.execute(f"CREATE SCHEMA IF NOT EXISTS {SCHEMA_NAME}")
    connection.execute(f"CREATE TABLE {SCHEMA_NAME}.{relation_name} AS {sql}")


def _fixture_plan(
    *,
    kind: ScenarioArtifactKind,
    logical_name: str,
    physical_name: str,
    sql: str,
) -> ScenarioFixturePlan:
    return ScenarioFixturePlan(
        kind=kind,
        logical_name=logical_name,
        destination=_target(physical_name=physical_name),
        sql=sql,
    )


def _target(*, physical_name: str) -> CompiledRelationLocation:
    return CompiledRelationLocation(
        database=None,
        schema=SCHEMA_NAME,
        name=physical_name,
        qualified_name=f"{SCHEMA_NAME}.{physical_name}",
    )


def _model_entry(
    *,
    name: str,
    target: CompiledRelationLocation,
    sql: str,
    pre_hooks: object = None,
    post_hooks: object = None,
) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        materialization_type=MaterializationType.TABLE,
        action=PlanAction.CREATE_TABLE,
        reason=PlanReason.FIRST_RUN,
        destination=target,
        fingerprint_query_sql=sql,
        resolved_sql=sql,
        logical_ddl="",
        pre_hooks=pre_hooks,
        post_hooks=post_hooks,
    )


def write_partial_fixture_snowflake_project(
    *, project_dir: Path, fixture_sql: str, helper_ctes: str = ""
) -> None:
    """Write a Snowflake project whose scenario mocks a contracted model with ``fixture_sql``."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "shop"\nadapter = "snowflake"\n\n[connection]\naccount = "example"\n'
            'user = "example"\ndatabase = "analytics"\nschema = "marts"\n\n'
            '[defaults]\ndatabase = "analytics"\nschema = "marts"\nmaterialized = "table"\n'
        ),
        "sql/models/upstream.sql": (
            "MODEL (description 'Upstream.', contract enforced,\n"
            "  columns (id (type INT), label (type VARCHAR)));\n"
            "SELECT 1 AS id, CAST('a' AS VARCHAR) AS label\n"
        ),
        "sql/models/downstream.sql": (
            "MODEL (description 'Downstream.');\n"
            'SELECT u.id, u.label FROM __ref("upstream") AS u\n'
        ),
        "tests/scenarios/downstream_labels.sql": (
            "SCENARIO (description 'Downstream labels.');\n"
            f"WITH\n{helper_ctes}__ref__upstream AS ({fixture_sql}),\n"
            "__expected__downstream AS (SELECT 1 AS id, CAST(NULL AS VARCHAR) AS label)\n"
            "SELECT 1\n"
        ),
    }
    relative: str
    content: str
    for relative, content in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")


def compile_offline_snowflake_project(*, project_dir: Path) -> CompilePipelineResult:
    """Compile and plan a Snowflake project against an offline warehouse."""

    inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    warehouse: SimulatedSnowflakeWarehouse = SimulatedSnowflakeWarehouse(
        relations=(
            FakeRelation(
                database="ANALYTICS",
                schema="MARTS",
                name="UNRELATED",
                columns=(FakeColumn(name="ID", data_type="NUMBER"),),
            ),
        )
    )
    return run_compile_pipeline(
        discovered_inputs=inputs,
        adapter=OfflineSnowflakeAdapter(warehouse=warehouse),
        options=CompilePipelineOptions(
            no_cache=True, connection_config=dict(inputs.project_config.connection)
        ),
        hooks=ConnectionHooks(),
    )
