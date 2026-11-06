"""Test helpers for planner helpers tests."""

from __future__ import annotations

from collections.abc import Callable, Iterator
from dataclasses import replace
from datetime import UTC, datetime
from itertools import chain
from pathlib import Path
from types import MappingProxyType, SimpleNamespace
from typing import Any, ClassVar
from unittest.mock import Mock

import sqlbuild._native as _native
from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    ChangeDetectionResult,
    CompiledAudit,
    CompiledDirectLogicSqlTestPayload,
    CompiledFunction,
    CompiledModel,
    CompiledModelSqlTestPayload,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSeed,
    CompiledSource,
    CompiledSqlScenario,
    CompiledSqlTest,
    CompileModelConfig,
    CompilePipelineResult,
    CompileSqlReference,
    CompileSqlScenarioCte,
    CompileSqlTestCte,
    DefaultsConfig,
    DiscoveredAuditBlock,
    DiscoveredAuditFile,
    DiscoveredLoaderFunction,
    DiscoveredSchemaFile,
    DiscoveredSeedFile,
    DiscoveredSourceFile,
    DiscoveredSqlScenarioFile,
    DiscoveredSqlTestBlock,
    DiscoveredSqlTestFile,
    Fingerprint,
    FunctionArgument,
    PlannerChangeResults,
    PlannerScope,
    PlanOutput,
    PlanReason,
    ScenarioArtifactIdentity,
    ScenarioRelationMap,
    SchemaColumn,
    SchemaModelEntry,
    SchemaSeedEntry,
    SeedCsvSettings,
    SourceColumnEntry,
    SourceEntry,
    SourceWriteStrategy,
    SqlLexicalSyntax,
    SqlReferenceKind,
    WarehouseSnapshot,
    build_downstream_deps,
    build_scenario_relation_map,
    compiled_model_record,
    compute_query_hash,
    detect_changes,
    project_with,
    scan_sql_reference_calls,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.compiler.auditing.types import AuditSeverity
from sqlbuild.compiler.compile.types import (
    AttachedAuditTargetKind,
    CompiledAuditView,
    CompiledFunctionView,
    CompiledModelView,
    CompiledProjectView,
    CompiledSeedView,
    CompiledSourceView,
    SqlTestMode,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    BuildModelWarningsTestCase,
    IncrementalStrategyErrorTestCase,
    MicrobatchCursorEndPlanTestCase,
    PlanAuditTestCase,
    PlanScenarioGraphErrorTestCase,
    PlanScenarioGraphTestCase,
    ResolveModelPlanActionTestCase,
    SourceCursorInputColumnsTestCase,
)

_GENERIC_SQL_SYNTAX: SqlLexicalSyntax = SqlLexicalSyntax()


def microbatch_model_with_cursor_end(
    test_case: MicrobatchCursorEndPlanTestCase,
) -> CompiledModelView:
    """Build a microbatch model with a configured terminal cursor bound."""

    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),
        deps=(),
        name="orders",
        relative_path=Path("models/orders.sql"),
        query_sql="SELECT 1",
        references=(),
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "incremental_strategy": "delete_insert",
                "incremental_mode": "microbatch",
                "microbatch_strategy": "watermark",
                "batch_size": test_case.batch_size,
                "cursor": "cursor_value",
                "cursor_type": test_case.cursor_type,
                "cursor_end": test_case.cursor_end,
                **test_case.authored_cursor_grain,
            }
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="orders",
            qualified_name="main.orders",
        ),
    )


class PlannerTestAdapter(BaseAdapter):
    """Minimal adapter for planner helper tests."""

    adapter_name: ClassVar[str] = "planner-test"

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def _execute(self, connection: object, sql: str) -> object:
        del connection, sql
        return object()

    def close(self, connection: object) -> None:
        del connection


def _append_relation_info(listed: list[RelationInfo], relation: RelationInfo) -> None:
    listed.append(relation)


def _skip_relation_info(listed: list[RelationInfo], relation: RelationInfo) -> None:
    del listed, relation


_RELATION_INFO_COLLECTORS: MappingProxyType[
    bool, Callable[[list[RelationInfo], RelationInfo], None]
] = MappingProxyType({False: _skip_relation_info, True: _append_relation_info})


def model_key(name: str) -> CompiledObjectKey:
    """Build a model object key."""

    return CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name)


def build_selection_model(
    *,
    key: CompiledObjectKey,
    name: str,
    materialized: str,
) -> CompiledModelView:
    """Build a minimal model for selection helper tests."""

    values: dict[str, object] = {"materialized": materialized}
    return CompiledModel(
        key=key,
        deps=(),
        name=name,
        relative_path=Path(f"models/{name}.sql"),
        query_sql="SELECT 1",
        config=CompileModelConfig(values=values),
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name=name,
            qualified_name=f"main.{name}",
        ),
    )


def source_key(name: str) -> CompiledObjectKey:
    """Build a source object key."""

    return CompiledObjectKey(resource_type=CompiledResourceType.SOURCE, name=name)


def fetch_orders(_ctx: object) -> list[dict[str, object]]:
    return []


def load_orders(_ctx: object) -> list[dict[str, object]]:
    return []


def build_source_load_nodes_project() -> CompiledProjectView:
    raw_orders_entry: SourceEntry = SourceEntry(
        name="raw_orders",
        table="orders",
        loader="load_orders",
        write_strategy=SourceWriteStrategy.MERGE,
        cursor_column="updated_at",
        unique_key=("order_id",),
    )
    return CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        sources=(
            CompiledSource(
                key=source_key("raw_orders"),
                deps=(),
                name="raw_orders",
                source_entry=raw_orders_entry,
                source_file=DiscoveredSourceFile(
                    file_path=Path("sources/raw.yml"),
                    relative_path=Path("sources/raw.yml"),
                    contents="",
                    source_entries=(raw_orders_entry,),
                ),
            ),
        ),
        loader_functions=(
            DiscoveredLoaderFunction(
                file_path=Path("python/loaders/raw.py"),
                relative_path=Path("python/loaders/raw.py"),
                name="fetch_orders",
                function=fetch_orders,
                destination="staging_fetch_orders",
                write_strategy=SourceWriteStrategy.TABLE,
            ),
            DiscoveredLoaderFunction(
                file_path=Path("python/loaders/raw.py"),
                relative_path=Path("python/loaders/raw.py"),
                name="load_orders",
                function=load_orders,
                depends_on=(fetch_orders,),
            ),
        ),
    )


def seed_key(name: str) -> CompiledObjectKey:
    """Build a seed object key."""

    return CompiledObjectKey(resource_type=CompiledResourceType.SEED, name=name)


def function_key(name: str) -> CompiledObjectKey:
    """Build a function object key."""

    return CompiledObjectKey(resource_type=CompiledResourceType.UDF, name=name)


def dbt_ref_key(name: str) -> CompiledObjectKey:
    """Build a dbt ref object key."""

    return CompiledObjectKey(resource_type=CompiledResourceType.DBT_REF, name=name)


def build_test_project(
    *,
    model_deps: dict[str, tuple[str, ...]] | None = None,
    model_paths: dict[str, str] | None = None,
    source_names: tuple[str, ...] = (),
    seed_names: tuple[str, ...] = (),
    dbt_ref_names: tuple[str, ...] = (),
    function_names: tuple[str, ...] = (),
    sql_test_expected_model_names: tuple[str, ...] = (),
    table_fn_test_function_names: tuple[str, ...] = (),
    audit_model_source_deps: dict[str, tuple[str, ...]] | None = None,
) -> CompiledProjectView:
    """Build a minimal CompiledProject for graph tests."""

    effective_paths: dict[str, str] = model_paths or {}
    source_name_set: set[str] = set(source_names)
    seed_name_set: set[str] = set(seed_names)
    dbt_ref_name_set: set[str] = set(dbt_ref_names)
    function_name_set: set[str] = set(function_names)
    models: list[CompiledModelView] = []
    model_name: str
    dep_names: tuple[str, ...]
    for model_name, dep_names in (model_deps or {}).items():
        deps: tuple[CompiledObjectKey, ...] = tuple(
            _resolve_dep_key(
                d,
                source_name_set,
                seed_name_set,
                dbt_ref_name_set,
                function_name_set,
            )
            for d in dep_names
        )
        rel_path: str = effective_paths.get(model_name, f"models/{model_name}.sql")
        models.append(
            CompiledModel(
                key=model_key(model_name),
                deps=deps,
                name=model_name,
                relative_path=Path(rel_path),
                query_sql=f"SELECT * FROM {model_name}",
                config=CompileModelConfig(),
                destination=CompiledRelationLocation(
                    database=None, schema=None, name=model_name, qualified_name=None
                ),
            )
        )

    sources: list[CompiledSourceView] = []
    source_name: str
    for source_name in source_names:
        source_entry: SourceEntry = SourceEntry(
            name=source_name, schema="public", table=source_name
        )
        sources.append(
            CompiledSource(
                key=source_key(source_name),
                deps=(),
                name=source_name,
                source_entry=source_entry,
                source_file=DiscoveredSourceFile(
                    file_path=Path(f"sources/{source_name}.yml"),
                    relative_path=Path(source_name),
                    contents="",
                    source_entries=(source_entry,),
                ),
            )
        )

    seeds: list[CompiledSeedView] = []
    seed_name: str
    for seed_name in seed_names:
        seeds.append(
            CompiledSeed(
                key=seed_key(seed_name),
                deps=(),
                name=seed_name,
                seed_file=DiscoveredSeedFile(
                    file_path=Path(f"seeds/{seed_name}.csv"),
                    relative_path=Path(f"seeds/{seed_name}.csv"),
                ),
                schema_entry=SchemaSeedEntry(name=seed_name, columns=()),
                schema_file=_stub_schema_file(),
                destination=CompiledRelationLocation(
                    database=None, schema=None, name=seed_name, qualified_name=None
                ),
            )
        )

    functions: list[CompiledFunctionView] = []
    function_name: str
    for function_name in function_names:
        functions.append(
            CompiledFunction(
                key=function_key(function_name),
                deps=(),
                name=function_name,
                relative_path=Path(f"functions/sql/{function_name}.sql"),
                arguments=(),
                returns="BOOLEAN",
                body_sql="SELECT TRUE",
                destination=CompiledRelationLocation(
                    database=None, schema=None, name=function_name, qualified_name=None
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None, schema=None, name=function_name, qualified_name=None
                ),
            )
        )

    sql_tests: list[CompiledSqlTest] = []
    model_sql_test: CompiledSqlTest = CompiledSqlTest(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_TEST,
            name="test_models",
        ),
        scope_deps=tuple(model_key(name) for name in sql_test_expected_model_names),
        name="test_models",
        test_file=DiscoveredSqlTestFile(
            file_path=Path("tests/test_models.sql"),
            relative_path=Path("tests/test_models.sql"),
            contents="SELECT 1",
            blocks=(),
        ),
        test_block=DiscoveredSqlTestBlock(
            test_index=0,
            header_values={},
            sql_body="SELECT 1",
        ),
        sql_body="SELECT 1",
        payload=CompiledModelSqlTestPayload(
            expected_model_names=sql_test_expected_model_names,
        ),
    )
    sql_tests.extend(((), (model_sql_test,))[bool(sql_test_expected_model_names)])
    table_function_sql_test: CompiledSqlTest = CompiledSqlTest(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_TEST,
            name="test_table_functions",
        ),
        scope_deps=tuple(function_key(name) for name in table_fn_test_function_names),
        name="test_table_functions",
        test_file=DiscoveredSqlTestFile(
            file_path=Path("tests/test_table_functions.sql"),
            relative_path=Path("tests/test_table_functions.sql"),
            contents="SELECT 1",
            blocks=(),
        ),
        test_block=DiscoveredSqlTestBlock(
            test_index=0,
            header_values={"mode": "table_fn"},
            sql_body="SELECT 1",
        ),
        sql_body="SELECT 1",
        mode=SqlTestMode.TABLE_FN,
        payload=CompiledDirectLogicSqlTestPayload(
            mode=SqlTestMode.TABLE_FN,
            actual_cte=CompileSqlTestCte(
                name="__table_fn_actual__",
                sql_body='SELECT * FROM __table_fn("customer_orders")(42)',
            ),
            expected_cte=CompileSqlTestCte(
                name="__table_fn_expected__",
                sql_body="SELECT 1",
            ),
            tested_resource_names=table_fn_test_function_names,
        ),
    )
    sql_tests.extend(((), (table_function_sql_test,))[bool(table_fn_test_function_names)])

    audits: list[CompiledAuditView] = []
    model_name: str
    audit_source_names: tuple[str, ...]
    for model_name, audit_source_names in (audit_model_source_deps or {}).items():
        audits.append(
            CompiledAudit(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.AUDIT,
                    name=f"{model_name}_audit",
                ),
                scope_deps=(
                    model_key(model_name),
                    *(source_key(source_name) for source_name in audit_source_names),
                ),
                name=f"{model_name}_audit",
                definition_name="test_audit",
                audit_file=DiscoveredAuditFile(
                    file_path=Path("audits/generic/test.sql"),
                    relative_path=Path("audits/generic/test.sql"),
                    contents="",
                    blocks=(),
                ),
                audit_block=DiscoveredAuditBlock(
                    audit_index=0,
                    header_values={},
                    sql_body="SELECT 1",
                ),
                sql_body="SELECT 1",
                attached_target_kind=AttachedAuditTargetKind.MODEL,
                attached_target_name=model_name,
            )
        )

    return CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=tuple(models),
        sources=tuple(sources),
        seeds=tuple(seeds),
        functions=tuple(functions),
        audits=tuple(audits),
        sql_tests=tuple(sql_tests),
    )


def build_snapshot_from_relation_names(relation_names: tuple[str, ...]) -> WarehouseSnapshot:
    """Build a minimal WarehouseSnapshot with the given relation names."""

    existing_relations: dict[str, RelationInfo] = {
        name: RelationInfo(database=None, schema="public", name=name, relation_type="BASE TABLE")
        for name in relation_names
    }
    return WarehouseSnapshot(existing_relations=existing_relations)


def build_scenario_from_test_case(
    test_case: PlanScenarioGraphTestCase | PlanScenarioGraphErrorTestCase,
) -> CompiledSqlScenario:
    """Build a minimal compiled SQL scenario from a graph test case."""

    scenario_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SQL_SCENARIO,
        name="revenue__customer_refund",
    )
    assertion_ctes: tuple[CompileSqlScenarioCte, ...] = tuple(
        CompileSqlScenarioCte(name=f"__assert__assertion_{index}", sql_body=sql_body)
        for index, sql_body in enumerate(test_case.assertion_sql_bodies)
    )
    expected_ctes: tuple[CompileSqlScenarioCte, ...] = tuple(
        CompileSqlScenarioCte(name=f"__expected__{model_name}", sql_body="SELECT 1")
        for model_name in test_case.expected_model_names
    )
    assertion_target_model_names: tuple[str, ...] = _assertion_ref_targets(
        test_case.assertion_sql_bodies
    )
    return CompiledSqlScenario(
        key=scenario_key,
        name="revenue__customer_refund",
        scenario_file=DiscoveredSqlScenarioFile(
            file_path=Path("tests/scenarios/revenue__customer_refund.sql"),
            relative_path=Path("tests/scenarios/revenue__customer_refund.sql"),
            contents="",
            header_values={},
            name="revenue__customer_refund",
            sql_body="SELECT 1",
        ),
        sql_body="SELECT 1",
        expected_ctes=expected_ctes,
        assertion_ctes=assertion_ctes,
        source_fixture_names=test_case.source_fixture_names,
        ref_fixture_names=test_case.ref_fixture_names,
        seed_fixture_names=test_case.seed_fixture_names,
        dbt_ref_fixture_names=test_case.dbt_ref_fixture_names,
        expected_model_names=test_case.expected_model_names,
        assertion_names=tuple(cte.name.removeprefix("__assert__") for cte in assertion_ctes),
        assertion_target_model_names=assertion_target_model_names,
        target_model_names=tuple(
            dict.fromkeys((*test_case.expected_model_names, *assertion_target_model_names))
        ),
    )


def build_compiled_scenario_with_name(name: str) -> CompiledSqlScenario:
    """Build a minimal compiled SQL scenario with a specific name."""

    return CompiledSqlScenario(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name=name,
        ),
        name=name,
        scenario_file=DiscoveredSqlScenarioFile(
            file_path=Path(f"tests/scenarios/{name}.sql"),
            relative_path=Path(f"tests/scenarios/{name}.sql"),
            contents="",
            header_values={},
            name=name,
            sql_body="SELECT 1",
        ),
        sql_body="SELECT 1",
    )


def build_scenario_cli_identifier_limit_pipeline(
    *, model_name: str
) -> tuple[CompiledSqlScenario, CompilePipelineResult]:
    """Build a minimal pipeline result for CLI scenario identifier-limit tests."""

    project: CompiledProjectView = build_test_project(
        model_deps={model_name: ("raw__orders",)},
        source_names=("raw__orders",),
    )
    scenario: CompiledSqlScenario = CompiledSqlScenario(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name="long_identifier_scenario",
        ),
        name="long_identifier_scenario",
        scenario_file=DiscoveredSqlScenarioFile(
            file_path=Path("tests/scenarios/long_identifier_scenario.sql"),
            relative_path=Path("tests/scenarios/long_identifier_scenario.sql"),
            contents="",
            header_values={},
            name="long_identifier_scenario",
            sql_body="SELECT 1",
        ),
        sql_body="SELECT 1",
        authored_ctes=(
            CompileSqlScenarioCte(
                name="__source__raw__orders",
                sql_body="SELECT 1 AS order_id",
            ),
        ),
        expected_ctes=(
            CompileSqlScenarioCte(
                name=f"__expected__{model_name}",
                sql_body="SELECT 1 AS order_id",
            ),
        ),
        source_fixture_names=("raw__orders",),
        expected_model_names=(model_name,),
    )
    project = project_with(project=project, sql_scenarios=(scenario,))
    return scenario, CompilePipelineResult(project=project, plan_output=PlanOutput())


def quoting_render_qualified_name(
    *, database: str | None, schema: str | None, name: str
) -> str | None:
    """Render qualified names with distinctive quoting to assert adapter pass-through."""

    return {
        (True, True): f'"{database}"."{schema}"."{name}"',
        (False, True): f'"{schema}"."{name}"',
        (True, False): None,
        (False, False): None,
    }[(database is not None, schema is not None)]


def build_scenario_relation_test_map() -> ScenarioRelationMap:
    """Build a relation map covering scenario relation planning tests."""

    return build_scenario_relation_map(
        scenario_name="revenue__customer_refund",
        hash_prefix="51b385aebe20",
        artifacts=(
            ScenarioArtifactIdentity(kind="source", logical_name="raw__orders"),
            ScenarioArtifactIdentity(kind="ref", logical_name="stg_customers"),
            ScenarioArtifactIdentity(kind="dbt_ref", logical_name="stripe__payments"),
            ScenarioArtifactIdentity(kind="seed", logical_name="country_codes"),
            ScenarioArtifactIdentity(kind="model", logical_name="daily_revenue"),
            ScenarioArtifactIdentity(kind="model", logical_name="customer_revenue"),
        ),
    )


def build_scenario_relation_test_project() -> CompiledProjectView:
    """Build a project covering scenario relation planning tests."""

    project: CompiledProjectView = build_test_project(
        model_deps={
            "daily_revenue": ("raw__orders", "stg_customers", "country_codes"),
            "customer_revenue": ("raw__orders",),
            "stg_customers": ("raw__orders",),
        },
        source_names=("raw__orders",),
        seed_names=("country_codes",),
    )
    models_by_name: dict[str, CompiledModelView] = {model.name: model for model in project.models}
    models_by_name["daily_revenue"] = replace(
        compiled_model_record(models_by_name["daily_revenue"]),
        query_sql=(
            'SELECT * FROM __source("raw__orders") '
            'JOIN __ref("stg_customers") USING (customer_id) '
            'JOIN __seed("country_codes") USING (country_code) '
            'JOIN __dbt_ref("stripe", "payments") USING (customer_id)'
        ),
    )
    models: list[CompiledModelView] = [models_by_name[model.name] for model in project.models]
    return project_with(project=project, models=tuple(models))


def build_scenario_relation_test_project_with_unused_seed() -> CompiledProjectView:
    """Build a scenario relation test project with a seed outside the scenario graph."""

    project: CompiledProjectView = build_scenario_relation_test_project()
    unused_seed_project: CompiledProjectView = build_test_project(seed_names=("unused_seed",))
    return project_with(project=project, seeds=(*project.seeds, unused_seed_project.seeds[0]))


def build_scenario_relation_test_scenario(
    *, include_seed_fixture: bool = True
) -> CompiledSqlScenario:
    """Build a scenario covering helper CTE and fixture planning tests."""

    authored_ctes: tuple[CompileSqlScenarioCte, ...] = (
        CompileSqlScenarioCte(
            name="helper_orders",
            sql_body="SELECT 1 AS order_id, 10 AS customer_id",
        ),
        CompileSqlScenarioCte(
            name="__source__raw__orders",
            sql_body="SELECT * FROM helper_orders",
        ),
        CompileSqlScenarioCte(
            name="__ref__stg_customers",
            sql_body="SELECT 10 AS customer_id",
        ),
        CompileSqlScenarioCte(
            name="__dbt_ref__stripe__payments",
            sql_body="SELECT 1 AS payment_id, 10 AS customer_id",
        ),
    )
    seed_ctes: tuple[CompileSqlScenarioCte, ...] = (
        *authored_ctes,
        CompileSqlScenarioCte(
            name="__seed__country_codes",
            sql_body="SELECT 'US' AS country_code",
        ),
    )
    authored_ctes = (authored_ctes, seed_ctes)[include_seed_fixture]

    return CompiledSqlScenario(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name="revenue__customer_refund",
        ),
        name="revenue__customer_refund",
        scenario_file=DiscoveredSqlScenarioFile(
            file_path=Path("tests/scenarios/revenue__customer_refund.sql"),
            relative_path=Path("tests/scenarios/revenue__customer_refund.sql"),
            contents="",
            header_values={},
            name="revenue__customer_refund",
            sql_body="SELECT 1",
        ),
        sql_body="SELECT 1",
        authored_ctes=authored_ctes,
        expected_ctes=(
            CompileSqlScenarioCte(
                name="__expected__daily_revenue",
                sql_body='SELECT * FROM __ref("daily_revenue")',
            ),
        ),
        assertion_ctes=(
            CompileSqlScenarioCte(
                name="__assert__no_negative_revenue",
                sql_body='SELECT * FROM __ref("daily_revenue") WHERE revenue < 0',
            ),
        ),
        source_fixture_names=("raw__orders",),
        ref_fixture_names=("stg_customers",),
        dbt_ref_fixture_names=("stripe__payments",),
        seed_fixture_names=((), ("country_codes",))[include_seed_fixture],
        expected_model_names=("daily_revenue",),
        assertion_names=("no_negative_revenue",),
    )


def build_test_project_with_source_entry(source_entry: SourceEntry) -> CompiledProjectView:
    """Build a minimal CompiledProject containing one source entry."""

    source: CompiledSourceView = CompiledSource(
        key=source_key(source_entry.name),
        deps=(),
        name=source_entry.name,
        source_entry=source_entry,
        source_file=DiscoveredSourceFile(
            file_path=Path("sources/raw.yml"),
            relative_path=Path("sources/raw.yml"),
            contents="",
            source_entries=(source_entry,),
        ),
    )
    return CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        sources=(source,),
    )


def build_source_cursor_input_model(
    test_case: SourceCursorInputColumnsTestCase,
) -> CompiledModelView:
    """Build an incremental model for source cursor input column validation."""

    config_values: dict[str, object] = (
        {"materialized": "incremental"}
        | ({}, {"cursor": test_case.cursor_column})[test_case.cursor_column is not None]
        | ({}, {"cursor_inputs": test_case.cursor_inputs})[test_case.cursor_inputs is not None]
    )
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="test_model"),
        deps=(),
        name="test_model",
        relative_path=Path("models/test_model.sql"),
        query_sql="SELECT 1",
        references=(
            CompileSqlReference(
                ref_kind=test_case.reference_kind,
                ref_name=test_case.reference_name,
            ),
        ),
        config=CompileModelConfig(values=config_values),
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name="test_model",
            qualified_name="staging.test_model",
        ),
    )


def build_transitive_watermark_models(
    *, watermark_name: str, watermark_column: str
) -> tuple[CompiledModelView, dict[str, CompiledModelView]]:
    """Build a consumer, direct view, and transitive source watermark lineage."""

    view_model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders_view"),
        deps=(),
        name="orders_view",
        relative_path=Path("models/orders_view.sql"),
        query_sql='SELECT * FROM __source("raw_orders")',
        references=(CompileSqlReference(ref_kind=SqlReferenceKind.SOURCE, ref_name="raw_orders"),),
        config=CompileModelConfig(values={"materialized": "view"}),
        inferred_columns=(),
        fast_lineage_has_star=True,
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name="orders_view",
            qualified_name="staging.orders_view",
        ),
    )
    consumer: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="test_model"),
        deps=(),
        name="test_model",
        relative_path=Path("models/test_model.sql"),
        query_sql='SELECT * FROM __ref("orders_view")',
        references=(CompileSqlReference(ref_kind=SqlReferenceKind.REF, ref_name="orders_view"),),
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "cursor": "event_time",
                "incremental_mode": "microbatch",
                "microbatch_strategy": "watermark",
                "cursor_watermark_mode": "all",
                "cursor_inputs": {
                    "orders_view": {"column": "event_time", "roles": ["filter"]},
                    watermark_name: {"column": watermark_column, "roles": ["watermark"]},
                },
            }
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="analytics",
            name="test_model",
            qualified_name="analytics.test_model",
        ),
    )
    return consumer, {view_model.name: view_model}


def build_function_transitive_watermark_models(
    *, watermark_name: str, watermark_column: str
) -> tuple[CompiledModelView, dict[str, CompiledModelView], dict[str, CompiledFunctionView]]:
    """Build a consumer whose physical watermark is upstream of a table function."""

    consumer: CompiledModelView
    models_by_name: dict[str, CompiledModelView]
    consumer, models_by_name = build_transitive_watermark_models(
        watermark_name=watermark_name,
        watermark_column=watermark_column,
    )
    function_name: str = "orders_for_window"
    function_location: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="analytics",
        name=function_name,
        qualified_name=f"analytics.{function_name}",
    )
    function: CompiledFunctionView = CompiledFunction(
        key=CompiledObjectKey(resource_type=CompiledResourceType.TABLE_FN, name=function_name),
        deps=(),
        name=function_name,
        relative_path=Path(f"functions/sql/{function_name}.sql"),
        arguments=(),
        returns="TABLE (order_id INTEGER)",
        body_sql='SELECT * FROM __ref("orders_view")',
        destination=function_location,
        fingerprint_destination=function_location,
        references=(CompileSqlReference(ref_kind=SqlReferenceKind.REF, ref_name="orders_view"),),
    )
    consumer = replace(
        compiled_model_record(consumer),
        query_sql=f'SELECT * FROM __table_fn("{function_name}")()',
        references=(
            CompileSqlReference(
                ref_kind=SqlReferenceKind.TABLE_FUNCTION,
                ref_name=function_name,
            ),
        ),
    )
    return consumer, models_by_name, {function.name: function}


def build_cursor_input_contract_models(
    test_case: SourceCursorInputColumnsTestCase,
) -> dict[str, CompiledModelView]:
    models: dict[str, CompiledModelView] = {
        test_case.reference_name: CompiledModel(
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.MODEL,
                name=test_case.reference_name,
            ),
            deps=(),
            name=test_case.reference_name,
            relative_path=Path(f"models/{test_case.reference_name}.sql"),
            query_sql="SELECT 1",
            config=CompileModelConfig(values={"contract": test_case.upstream_contract}),
            inferred_columns=test_case.compiled_inferred_columns,
            destination=CompiledRelationLocation(
                database=None,
                schema="staging",
                name=test_case.reference_name,
                qualified_name=f"staging.{test_case.reference_name}",
            ),
            schema_entry=SchemaModelEntry(
                name=test_case.reference_name,
                columns=tuple(
                    SchemaColumn(name=column_name)
                    for column_name in test_case.upstream_declared_columns
                ),
            ),
        )
    }
    include_model: bool = test_case.reference_kind == SqlReferenceKind.REF and (
        test_case.upstream_contract is not None or test_case.upstream_inferred_columns is not None
    )
    return ({}, models)[include_model]


def build_cursor_input_contract_sources(
    test_case: SourceCursorInputColumnsTestCase,
) -> dict[str, SourceEntry]:
    sources: dict[str, SourceEntry] = {
        test_case.reference_name: SourceEntry(
            name=test_case.reference_name,
            contract=test_case.upstream_contract,
            columns=tuple(
                SourceColumnEntry(name=column_name)
                for column_name in test_case.upstream_declared_columns
            ),
        )
    }
    include_source: bool = (
        test_case.reference_kind == SqlReferenceKind.SOURCE
        and test_case.upstream_contract is not None
    )
    return ({}, sources)[include_source]


def _resolve_dep_key(
    name: str,
    source_names: set[str],
    seed_names: set[str],
    dbt_ref_names: set[str],
    function_names: set[str],
) -> CompiledObjectKey:
    """Resolve a dependency name to the correct key type."""

    resource_types_by_name: dict[str, CompiledResourceType] = (
        {item: CompiledResourceType.UDF for item in function_names}
        | {item: CompiledResourceType.DBT_REF for item in dbt_ref_names}
        | {item: CompiledResourceType.SEED for item in seed_names}
        | {item: CompiledResourceType.SOURCE for item in source_names}
    )
    resource_type: CompiledResourceType = resource_types_by_name.get(
        name, CompiledResourceType.MODEL
    )
    return CompiledObjectKey(resource_type=resource_type, name=name)


def build_strategy_model(test_case: ResolveModelPlanActionTestCase) -> CompiledModelView:
    """Build a CompiledModel from an action resolution test case."""

    config_values: dict[str, object] = (
        {"materialized": test_case.materialized}
        | (
            {},
            {"incremental_strategy": test_case.incremental_strategy},
        )[test_case.incremental_strategy is not None]
        | ({}, {"enabled": test_case.enabled})[test_case.enabled is not None]
    )
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="test_model"),
        deps=(),
        name="test_model",
        relative_path=Path("models/test_model.sql"),
        query_sql="SELECT 1",
        config=CompileModelConfig(values=config_values),
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name="test_model",
            qualified_name="staging.test_model",
        ),
    )


def build_strategy_change_result(
    test_case: ResolveModelPlanActionTestCase,
) -> ChangeDetectionResult:
    """Build a ChangeDetectionResult from an action resolution test case."""

    return ChangeDetectionResult(
        model_name="test_model",
        change_kind=test_case.change_kind,
        query_changed=test_case.query_changed,
        schema_findings=test_case.schema_findings,
        backfill=BackfillResult(
            action=test_case.backfill_action,
            duration=test_case.backfill_duration,
        ),
    )


def build_strategy_error_model(test_case: IncrementalStrategyErrorTestCase) -> CompiledModelView:
    """Build a CompiledModel from a strategy error test case."""

    config_values: dict[str, object] = {"materialized": test_case.materialized} | (
        {},
        {"incremental_strategy": test_case.incremental_strategy},
    )[test_case.incremental_strategy is not None]
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="test_model"),
        deps=(),
        name="test_model",
        relative_path=Path("models/test_model.sql"),
        query_sql="SELECT 1",
        config=CompileModelConfig(values=config_values),
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name="test_model",
            qualified_name="staging.test_model",
        ),
    )


def build_strategy_error_change_result(
    test_case: IncrementalStrategyErrorTestCase,
) -> ChangeDetectionResult:
    """Build a ChangeDetectionResult from a strategy error test case."""

    return ChangeDetectionResult(
        model_name="test_model",
        change_kind=test_case.change_kind,
        query_changed=False,
        schema_findings=(),
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
    )


def build_warnings_change_result(
    test_case: BuildModelWarningsTestCase,
) -> ChangeDetectionResult:
    """Build a ChangeDetectionResult from a warnings test case."""

    return ChangeDetectionResult(
        model_name=test_case.model_name,
        change_kind=test_case.change_kind,
        query_changed=test_case.query_changed,
        schema_findings=test_case.schema_findings,
        backfill=BackfillResult(action=test_case.backfill_action),
    )


def build_audit_from_test_case(
    test_case: PlanAuditTestCase,
) -> CompiledAuditView:
    """Build a minimal CompiledAudit from a test case."""

    return CompiledAudit(
        key=CompiledObjectKey(resource_type=CompiledResourceType.AUDIT, name="test_audit"),
        definition_name="test_audit",
        scope_deps=(),
        name="test_audit",
        audit_file=DiscoveredAuditFile(
            file_path=Path("audits/generic/test.sql"),
            relative_path=Path("audits/generic/test.sql"),
            contents="",
            blocks=(),
        ),
        audit_block=DiscoveredAuditBlock(audit_index=0, header_values={}, sql_body=""),
        sql_body=test_case.sql_body,
        always_run=test_case.always_run,
    )


def build_audit_model_locations(
    targets: dict[str, str],
) -> dict[str, CompiledRelationLocation]:
    """Build model target lookup from name -> qualified_name."""

    return {
        name: CompiledRelationLocation(
            database=None,
            schema=None,
            name=name,
            qualified_name=qualified,
        )
        for name, qualified in targets.items()
    }


def build_audit_source_map(
    entries: dict[str, tuple[str | None, str, str | None]],
) -> dict[str, SourceEntry]:
    """Build source map from name -> (database, schema, table)."""

    return {
        name: SourceEntry(
            name=name,
            database=parts[0],
            schema=parts[1],
            table=parts[2],
        )
        for name, parts in entries.items()
    }


def build_cursor_override_model(cursor_type: str | None) -> CompiledModelView:
    """Build a minimal model with optional cursor_type for override resolution tests."""

    config_values: dict[str, object] = {"materialized": "incremental"} | (
        {},
        {"cursor_type": cursor_type},
    )[cursor_type is not None]
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="test_model"),
        deps=(),
        name="test_model",
        relative_path=Path("models/test_model.sql"),
        query_sql="SELECT 1",
        config=CompileModelConfig(values=config_values),
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name="test_model",
            qualified_name="staging.test_model",
        ),
    )


def build_microbatch_lookback_model(
    *,
    incremental_strategy: str,
    batch_size: str,
    lookback: str | None,
) -> CompiledModelView:
    """Build a microbatch model for lookback resolution tests."""

    config_values: dict[str, object] = {
        "materialized": "incremental",
        "incremental_strategy": incremental_strategy,
        "incremental_mode": "microbatch",
        "batch_size": batch_size,
    }
    config_values |= ({}, {"lookback": lookback})[lookback is not None]
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="test_model"),
        deps=(),
        name="test_model",
        relative_path=Path("models/test_model.sql"),
        query_sql="SELECT 1",
        config=CompileModelConfig(values=config_values),
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name="test_model",
            qualified_name="staging.test_model",
        ),
    )


def build_compiled_function(*, body_sql: str, target_schema: str = "main") -> CompiledFunction:
    """Build a minimal compiled function for planner tests."""

    return CompiledFunction(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.UDF,
            name="is_completed_order",
        ),
        deps=(),
        name="is_completed_order",
        relative_path=Path("functions/sql/is_completed_order.sql"),
        arguments=(FunctionArgument(name="order_status", type="STRING"),),
        returns="BOOLEAN",
        body_sql=body_sql,
        destination=CompiledRelationLocation(
            database=None,
            schema=target_schema,
            name="is_completed_order",
            qualified_name=f"{target_schema}.is_completed_order",
        ),
        fingerprint_destination=CompiledRelationLocation(
            database=None,
            schema=target_schema,
            name="is_completed_order",
            qualified_name=f"{target_schema}.is_completed_order",
        ),
    )


def build_fingerprint(*, query_sql: str) -> Fingerprint:
    """Build a fingerprint with a hash matching the supplied query SQL."""

    return Fingerprint(
        node_type="udf",
        node_name="is_completed_order",
        target_database=None,
        target_schema="main",
        target_name="is_completed_order",
        run_id="run-1",
        definition_hash=compute_query_hash(query_sql=query_sql, dialect=None),
        schema_fingerprint="",
        definition=query_sql,
        ts=datetime(2026, 1, 1, tzinfo=UTC),
    )


def _stub_schema_file() -> DiscoveredSchemaFile:
    """Return a minimal schema file stub for seed construction."""

    return DiscoveredSchemaFile(
        file_path=Path("seeds/schema.yml"),
        relative_path=Path("seeds/schema.yml"),
        contents="",
        model_entries=(),
        seed_entries=(),
    )


def build_scheduling_audit(
    *,
    references: tuple[CompileSqlReference, ...],
    attached_target_kind: AttachedAuditTargetKind | None,
    attached_target_name: str | None,
) -> CompiledAuditView:
    """Build a minimal CompiledAudit for scheduling tests."""

    stub_file: DiscoveredAuditFile = DiscoveredAuditFile(
        file_path=Path("audits/singular/check.sql"),
        relative_path=Path("audits/singular/check.sql"),
        contents="",
        blocks=(),
    )
    stub_block: DiscoveredAuditBlock = DiscoveredAuditBlock(
        audit_index=0,
        header_values={},
        sql_body="",
    )
    name: str = "test_audit"
    return CompiledAudit(
        key=CompiledObjectKey(resource_type=CompiledResourceType.AUDIT, name=name),
        definition_name="test_audit",
        scope_deps=native_audit_scope_deps(
            references=references,
            attached_target_kind=attached_target_kind,
            attached_target_name=attached_target_name,
        ),
        name=name,
        audit_file=stub_file,
        audit_block=stub_block,
        sql_body="SELECT 1",
        references=references,
        attached_target_kind=attached_target_kind,
        attached_target_name=attached_target_name,
        severity=AuditSeverity.WARN,
        run_scope="final",
    )


def build_scheduling_graph(
    edges: dict[str, tuple[str, ...]],
) -> tuple[
    dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
    dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]],
]:
    """Build upstream and downstream dep dicts from simple name-based edges."""

    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {}
    name: str
    deps: tuple[str, ...]
    for name, deps in edges.items():
        key: CompiledObjectKey = CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL, name=name
        )
        upstream[key] = tuple(
            CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=d) for d in deps
        )
    downstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = build_downstream_deps(
        upstream
    )
    return upstream, downstream


def build_seed_identity_compiled_seed(
    file_path: Path,
    *,
    csv_settings: SeedCsvSettings | None = None,
) -> CompiledSeedView:
    resolved_csv_settings: SeedCsvSettings = csv_settings or SeedCsvSettings()
    schema_entry: SchemaSeedEntry = SchemaSeedEntry(
        name="orders", csv_settings=resolved_csv_settings
    )
    return CompiledSeed(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SEED, name="orders"),
        deps=(),
        name="orders",
        seed_file=DiscoveredSeedFile(file_path=file_path, relative_path=Path("seeds/orders.csv")),
        schema_entry=schema_entry,
        schema_file=DiscoveredSchemaFile(
            file_path=file_path.parent / "schema.yml",
            relative_path=Path("seeds/schema.yml"),
            contents="",
            model_entries=(),
            seed_entries=(schema_entry,),
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="orders",
            qualified_name="main.orders",
        ),
    )


def native_audit_scope_deps(
    *,
    references: tuple[CompileSqlReference, ...],
    attached_target_kind: AttachedAuditTargetKind | None,
    attached_target_name: str | None,
) -> tuple[CompiledObjectKey, ...]:
    """The audit's scope dependencies as native project assembly derives them."""

    attached: tuple[str, str] | None = {
        True: None,
        False: (str(attached_target_kind), str(attached_target_name)),
    }[attached_target_kind is None or attached_target_name is None]
    audit_row: tuple[list[tuple[str, str, str | None]], tuple[str, str] | None] = (
        [
            (str(reference.ref_kind), reference.ref_name, reference.ref_package)
            for reference in references
        ],
        attached,
    )
    inputs: Any = SimpleNamespace(
        effective_vars={},
        effective_target=None,
        project_config=SimpleNamespace(defaults=DefaultsConfig()),
        seed_inputs=(),
        model_inputs=(),
        source_inputs=(),
        sql_function_inputs=(),
        audit_inputs=(SimpleNamespace(assembly_row=audit_row),),
    )
    audit_deps: tuple[list[tuple[str, str]], ...] = _native.assemble_native_project_resources(
        inputs=inputs, dialect=None, syntax_checks=()
    ).audit_scope_deps
    return tuple(
        CompiledObjectKey(resource_type=CompiledResourceType(kind), name=name)
        for kind, name in audit_deps[0]
    )


def _assertion_ref_targets(assertion_sql: tuple[str, ...]) -> tuple[str, ...]:
    """The unique `__ref` targets the assertion bodies read, in authored order."""

    references: Iterator[CompileSqlReference] = chain.from_iterable(
        scan_sql_reference_calls(sql=sql, syntax=_GENERIC_SQL_SYNTAX).references
        for sql in assertion_sql
    )
    refs: Iterator[CompileSqlReference] = filter(
        lambda reference: reference.ref_kind == SqlReferenceKind.REF, references
    )
    return tuple(dict.fromkeys(reference.ref_name for reference in refs))


def build_unreadable_model() -> CompiledModelView:
    """Build a model view whose every attribute read fails, so change detection cannot run."""

    model: CompiledModelView = Mock(spec=())
    return model


def detect_function_reason(
    *, function: CompiledFunctionView, fingerprints: dict[str, Fingerprint]
) -> PlanReason:
    """Detect one selected function's change reason through planner change detection."""

    project: Mock = Mock(
        functions=(function,),
        settings=SimpleNamespace(query_change_tracking=True),
        sql_analysis_dialect=None,
        hook_functions=(),
    )
    scope: PlannerScope = PlannerScope(
        upstream_deps={},
        downstream_deps={},
        all_keys={},
        selected_keys=frozenset({function.key}),
        execution_order=(),
        models_by_name={},
    )
    snapshot: Mock = Mock(fingerprints=SimpleNamespace(functions=fingerprints), column_dialect=None)
    changes: PlannerChangeResults = detect_changes(
        project=project, scope=scope, snapshot=snapshot, full_refresh=False
    )
    reason: PlanReason = changes.functions[function.name].reason
    return reason
