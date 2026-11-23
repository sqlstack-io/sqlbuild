from __future__ import annotations

from pathlib import Path

from sqlbuild._native import (
    CompiledAudit,
    CompiledFunction,
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSeed,
    CompiledSource,
    CompiledSqlScenario,
    CompiledSqlTest,
    CompileModelConfig,
    DiscoveredAuditBlock,
    DiscoveredAuditFile,
    DiscoveredLoaderFunction,
    DiscoveredSchemaFile,
    DiscoveredSeedFile,
    DiscoveredSourceFile,
    DiscoveredSqlScenarioFile,
    DiscoveredSqlTestBlock,
    DiscoveredSqlTestFile,
    FunctionArgument,
    ProjectGraph,
    SchemaColumn,
    SchemaModelEntry,
    SchemaSeedEntry,
    SourceColumnEntry,
    SourceEntry,
)
from sqlbuild.compiler.auditing.types import AuditSeverity
from sqlbuild.compiler.compile.types import (
    AttachedAuditTargetKind,
    CompiledProjectView,
    FunctionLanguage,
)
from tests.unit.src.sqlbuild.compiler.graph.helpers import project_graph_from_indexes


def build_dag_artifact_test_graph() -> ProjectGraph:
    def shared_order_feed() -> list[dict[str, object]]:
        return []

    def raw_orders_loader() -> list[dict[str, object]]:
        return []

    source_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.SOURCE, "raw_orders")
    seed_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.SEED, "country_codes")
    function_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.UDF, "normalize_email")
    model_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.MODEL, "orders")
    test_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.SQL_TEST, "orders_test")
    audit_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.AUDIT, "orders_audit")
    scenario_key: CompiledObjectKey = CompiledObjectKey(
        CompiledResourceType.SQL_SCENARIO, "orders_scenario"
    )

    source_entry: SourceEntry = SourceEntry(
        name="raw_orders",
        schema="raw",
        table="orders",
        loader="raw_orders_loader",
        description="Raw order events",
        columns=(SourceColumnEntry(name="order_id", type="INTEGER"),),
    )
    source_file: DiscoveredSourceFile = DiscoveredSourceFile(
        file_path=Path("sql/sources/raw.yml"),
        relative_path=Path("sql/sources/raw.yml"),
        contents="",
        source_entries=(source_entry,),
    )
    schema_file: DiscoveredSchemaFile = DiscoveredSchemaFile(
        file_path=Path("sql/models/schema.yml"),
        relative_path=Path("sql/models/schema.yml"),
        contents="",
        model_entries=(),
        seed_entries=(),
    )
    seed_file: DiscoveredSeedFile = DiscoveredSeedFile(
        file_path=Path("sql/seeds/country_codes.csv"),
        relative_path=Path("sql/seeds/country_codes.csv"),
    )
    model_target: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="analytics",
        name="orders",
        qualified_name="analytics.orders",
    )
    function_target: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="analytics_dev",
        name="normalize_email",
        qualified_name="analytics_dev.normalize_email",
        logical_schema="analytics",
    )
    project: CompiledProjectView = CompiledProject(
        run_id="run-1",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        effective_target_schema="raw",
        loader_functions=(
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/orders.py"),
                relative_path=Path("python/nodes/loaders/orders.py"),
                name="shared_order_feed",
                function=shared_order_feed,
            ),
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/orders.py"),
                relative_path=Path("python/nodes/loaders/orders.py"),
                name="raw_orders_loader",
                function=raw_orders_loader,
                depends_on=(shared_order_feed,),
            ),
        ),
        sources=(
            CompiledSource(
                key=source_key,
                deps=(),
                name="raw_orders",
                source_entry=source_entry,
                source_file=source_file,
            ),
        ),
        seeds=(
            CompiledSeed(
                key=seed_key,
                deps=(),
                name="country_codes",
                seed_file=seed_file,
                schema_entry=SchemaSeedEntry(
                    name="country_codes",
                    description="Country lookup",
                    tags=("reference",),
                    columns=(SchemaColumn(name="country_code", type="TEXT"),),
                ),
                schema_file=schema_file,
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics_dev",
                    name="country_codes",
                    qualified_name="analytics_dev.country_codes",
                    logical_schema="analytics",
                ),
            ),
        ),
        functions=(
            CompiledFunction(
                key=function_key,
                deps=(),
                name="normalize_email",
                relative_path=Path("sql/functions/normalize_email.sql"),
                arguments=(FunctionArgument(name="email", type="TEXT"),),
                returns="TEXT",
                body_sql="lower(email)",
                destination=function_target,
                fingerprint_destination=function_target,
                language=FunctionLanguage.SQL,
                tags=("normalization",),
            ),
        ),
        models=(
            CompiledModel(
                key=model_key,
                deps=(source_key, seed_key, function_key),
                name="orders",
                relative_path=Path("sql/models/orders.sql"),
                query_sql="SELECT 1 AS order_id",
                authored_sql="MODEL (materialized table);\n\nSELECT 1 AS order_id",
                config=CompileModelConfig(values={"materialized": "table", "tags": ["daily"]}),
                destination=model_target,
                schema_entry=SchemaModelEntry(
                    name="orders",
                    description="Clean orders",
                    columns=(SchemaColumn(name="order_id", type="INTEGER"),),
                ),
            ),
        ),
        sql_tests=(
            CompiledSqlTest(
                key=test_key,
                scope_deps=(model_key,),
                name="orders_test",
                test_file=DiscoveredSqlTestFile(
                    file_path=Path("tests/orders.sql"),
                    relative_path=Path("tests/orders.sql"),
                    contents="",
                    blocks=(),
                ),
                test_block=DiscoveredSqlTestBlock(test_index=0, header_values={}, sql_body=""),
                sql_body="",
            ),
        ),
        audits=(
            CompiledAudit(
                key=audit_key,
                scope_deps=(model_key,),
                name="orders_audit",
                definition_name="test_audit",
                audit_file=DiscoveredAuditFile(
                    file_path=Path("sql/audits/orders.sql"),
                    relative_path=Path("sql/audits/orders.sql"),
                    contents="",
                    blocks=(),
                ),
                audit_block=DiscoveredAuditBlock(audit_index=0, header_values={}, sql_body=""),
                sql_body="",
                attached_target_kind=AttachedAuditTargetKind.MODEL,
                attached_target_name="orders",
                attached_column_name="order_id",
                severity=AuditSeverity.WARN,
            ),
        ),
        sql_scenarios=(
            CompiledSqlScenario(
                key=scenario_key,
                name="orders_scenario",
                scenario_file=DiscoveredSqlScenarioFile(
                    file_path=Path("tests/scenarios/orders.sql"),
                    relative_path=Path("tests/scenarios/orders.sql"),
                    contents="",
                    header_values={},
                    sql_body="",
                    name="orders_scenario",
                ),
                sql_body="",
                expected_model_names=("orders",),
                assertion_names=("orders_have_ids",),
                source_fixture_names=("raw_orders",),
            ),
        ),
    )
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        source_key: (),
        seed_key: (),
        function_key: (),
        model_key: (source_key, seed_key, function_key),
    }
    return project_graph_from_indexes(
        project=project,
        upstream_deps=upstream_deps,
        downstream_deps={
            source_key: (model_key,),
            seed_key: (model_key,),
            function_key: (model_key,),
            model_key: (),
        },
        tag_index={"daily": frozenset((model_key,))},
        path_index={model_key: ""},
        all_keys={
            "raw_orders": source_key,
            "country_codes": seed_key,
            "normalize_email": function_key,
            "orders": model_key,
        },
    )
