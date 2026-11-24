from __future__ import annotations

import re
from dataclasses import replace
from pathlib import Path
from typing import Any, cast

import pytest

import sqlbuild._native as _native
from sqlbuild._native import (
    AuditPlanEntry,
    ChainStep,
    CompiledObjectKey,
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    DiscoveredSqlHookFile,
    LoadedMacro,
    MacroMetadata,
    OwnershipRoot,
    PlanOutput,
    ResourceKind,
    SchemaColumn,
    ScopeIndex,
    ScopeKind,
    SourceColumnEntry,
    SqlHookEntry,
    SqlTestPlanEntry,
    build_manifest,
    project_with,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
    CompiledSeedView,
    CompiledSourceView,
)
from tests.unit.src.sqlbuild.compiler.manifest._test_types import (
    FqnTestCase,
    ManifestAuditNodeTestCase,
    ManifestEncodingErrorTestCase,
    ManifestMacroNodeTestCase,
    ManifestModelNodeTestCase,
    ManifestParentMapTestCase,
    ManifestSchemaValidationTestCase,
    ManifestSeedNodeTestCase,
    ManifestSourceNodeTestCase,
    ManifestSqlTestNodeTestCase,
    ManifestTopLevelTestCase,
)
from tests.unit.src.sqlbuild.compiler.manifest.helpers import (
    build_test_audit_plan_entry,
    build_test_model,
    build_test_plan_entry,
    build_test_plan_output,
    build_test_project,
    build_test_seed,
    build_test_source,
    build_test_sql_test_plan_entry,
    manifest_macros,
    manifest_nodes,
    manifest_sources,
    model_key,
    run_manifest,
    source_key,
)

_PROJECT: str = "demo"
_ADAPTER: str = "duckdb"
_DBT_MANIFEST_SCHEMA_VERSION: str = "https://schemas.getdbt.com/dbt/manifest/v12.json"

_MODEL_ORDERS: CompiledModelView = build_test_model(
    name="orders",
    query_sql="SELECT * FROM raw",
    relative_path="sql/models/staging/orders.sql",
    schema="staging",
    config_values={"materialized": "table"},
    description="Order records",
    columns=(SchemaColumn(name="id", type="INTEGER"),),
)

_MODEL_ORPHAN: CompiledModelView = build_test_model(
    name="orphan",
    query_sql="SELECT 1",
    relative_path="sql/models/orphan.sql",
    schema="public",
)

_MODEL_WITH_COLUMNS: CompiledModelView = build_test_model(
    name="typed_model",
    relative_path="sql/models/typed_model.sql",
    description="Has columns",
    columns=(
        SchemaColumn(name="id", type="INTEGER", description="Primary key"),
        SchemaColumn(name="status", type="VARCHAR"),
    ),
)

_MODEL_WITH_DEPS: CompiledModelView = build_test_model(
    name="joined",
    relative_path="sql/models/joined.sql",
    deps=(model_key("orders"), source_key("raw_customers")),
)

_MODEL_WITH_ALIAS: CompiledModelView = build_test_model(
    name="orders_aliased",
    relative_path="sql/models/marts/orders_aliased.sql",
    database="analytics",
    schema="marts",
    alias="fact_orders",
    qualified_name="analytics.marts.fact_orders",
    config_values={"materialized": "incremental", "tags": ["core", "nightly"]},
)

_SOURCE_WITH_COLUMNS: CompiledSourceView = build_test_source(
    name="raw_events",
    database="raw_db",
    schema="events",
    table="clicks",
    description="Raw click events",
    columns=(
        SourceColumnEntry(name="event_id", type="VARCHAR", description="Event identifier"),
        SourceColumnEntry(name="ts", type="TIMESTAMP"),
    ),
)

_SOURCE_RAW_ORDERS: CompiledSourceView = build_test_source(
    name="raw_orders",
    database="raw_db",
    schema="public",
    table="orders_table",
    description="Raw order data",
)
_SOURCE_EVENTS: CompiledSourceView = build_test_source(
    name="events",
    database=None,
    schema="analytics",
)

_SOURCE_FIXTURES: dict[str, Any] = {
    "raw_orders": _SOURCE_RAW_ORDERS,
    "events": _SOURCE_EVENTS,
    "raw_events": _SOURCE_WITH_COLUMNS,
}


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestTopLevelTestCase(
            description="produces correct top-level structure for single model project",
            project=build_test_project(
                models=(_MODEL_ORDERS,),
                sources=(build_test_source(name="raw_src"),),
            ),
            plan_output=build_test_plan_output(
                model_entries=(build_test_plan_entry(name="orders"),),
            ),
            project_name=_PROJECT,
            adapter_type=_ADAPTER,
            upstream_deps={},
            downstream_deps={},
            expected_node_count=1,
            expected_source_count=1,
            expected_macro_count=0,
            expected_metadata_project_name=_PROJECT,
            expected_metadata_adapter_type=_ADAPTER,
            expected_metadata_schema_version=_DBT_MANIFEST_SCHEMA_VERSION,
        ),
        ManifestTopLevelTestCase(
            description="produces correct counts for multi-resource project",
            project=build_test_project(
                models=(_MODEL_ORDERS, _MODEL_ORPHAN),
                sources=(build_test_source(name="raw_src"),),
                seeds=(build_test_seed(name="codes"),),
            ),
            plan_output=build_test_plan_output(
                model_entries=(build_test_plan_entry(name="orders"),),
            ),
            project_name=_PROJECT,
            adapter_type=_ADAPTER,
            upstream_deps={},
            downstream_deps={},
            expected_node_count=3,
            expected_source_count=1,
            expected_macro_count=0,
            expected_metadata_project_name=_PROJECT,
            expected_metadata_adapter_type=_ADAPTER,
            expected_metadata_schema_version=_DBT_MANIFEST_SCHEMA_VERSION,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_when_building_manifest_then_produces_correct_structure(
    test_case: ManifestTopLevelTestCase,
) -> None:
    result: dict[str, Any] = run_manifest(
        project=test_case.project,
        plan_output=test_case.plan_output,
        project_name=test_case.project_name,
        adapter_type=test_case.adapter_type,
        upstream_deps=test_case.upstream_deps,
        downstream_deps=test_case.downstream_deps,
    )

    assert result["metadata"]["project_name"] == test_case.expected_metadata_project_name
    assert result["metadata"]["adapter_type"] == test_case.expected_metadata_adapter_type
    assert result["metadata"]["dbt_schema_version"] == test_case.expected_metadata_schema_version
    assert len(result["nodes"]) == test_case.expected_node_count
    assert len(result["sources"]) == test_case.expected_source_count
    assert len(result["macros"]) == test_case.expected_macro_count
    assert result["exposures"] == {}
    assert result["metrics"] == {}
    assert result["groups"] == {}
    assert result["disabled"] == {}


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestModelNodeTestCase(
            description="produces correct model node with plan entry compiled code",
            model=_MODEL_ORDERS,
            plan_entries=(
                build_test_plan_entry(
                    name="orders", resolved_sql="SELECT * FROM staging.raw_orders"
                ),
            ),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.orders",
            expected_resource_type="model",
            expected_database=None,
            expected_schema="staging",
            expected_alias="orders",
            expected_fqn=[_PROJECT, "models", "staging", "orders"],
            expected_raw_code="SELECT * FROM raw",
            expected_compiled_code="SELECT * FROM staging.raw_orders",
            expected_relation_name="staging.orders",
            expected_description="Order records",
            expected_materialized="table",
            expected_checksum_name="sha256",
        ),
        ManifestModelNodeTestCase(
            description="uses raw_code as compiled_code when no plan entry exists",
            model=_MODEL_ORPHAN,
            plan_entries=(),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.orphan",
            expected_resource_type="model",
            expected_database=None,
            expected_schema="public",
            expected_alias="orphan",
            expected_fqn=[_PROJECT, "models", "orphan"],
            expected_raw_code="SELECT 1",
            expected_compiled_code="SELECT 1",
            expected_relation_name="public.orphan",
            expected_description="",
            expected_materialized="view",
            expected_checksum_name="sha256",
        ),
        ManifestModelNodeTestCase(
            description="model columns appear in manifest node",
            model=_MODEL_WITH_COLUMNS,
            plan_entries=(),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.typed_model",
            expected_resource_type="model",
            expected_database=None,
            expected_schema="public",
            expected_alias="typed_model",
            expected_fqn=[_PROJECT, "models", "typed_model"],
            expected_raw_code="SELECT 1",
            expected_compiled_code="SELECT 1",
            expected_relation_name="public.typed_model",
            expected_description="Has columns",
            expected_materialized="view",
            expected_checksum_name="sha256",
            expected_column_names=("id", "status"),
            expected_column_types={"id": "INTEGER", "status": "VARCHAR"},
        ),
        ManifestModelNodeTestCase(
            description="depends_on includes upstream model and source ids",
            model=_MODEL_WITH_DEPS,
            plan_entries=(),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.joined",
            expected_resource_type="model",
            expected_database=None,
            expected_schema="public",
            expected_alias="joined",
            expected_fqn=[_PROJECT, "models", "joined"],
            expected_raw_code="SELECT 1",
            expected_compiled_code="SELECT 1",
            expected_relation_name="public.joined",
            expected_description="",
            expected_materialized="view",
            expected_checksum_name="sha256",
            expected_depends_on_nodes=(
                f"model.{_PROJECT}.orders",
                f"source.{_PROJECT}.raw_customers",
            ),
        ),
        ManifestModelNodeTestCase(
            description="model with explicit alias database and tags",
            model=_MODEL_WITH_ALIAS,
            plan_entries=(),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.orders_aliased",
            expected_resource_type="model",
            expected_database="analytics",
            expected_schema="marts",
            expected_alias="fact_orders",
            expected_fqn=[_PROJECT, "models", "marts", "orders_aliased"],
            expected_raw_code="SELECT 1",
            expected_compiled_code="SELECT 1",
            expected_relation_name="analytics.marts.fact_orders",
            expected_description="",
            expected_materialized="incremental",
            expected_checksum_name="sha256",
            expected_tags=("core", "nightly"),
        ),
        ManifestModelNodeTestCase(
            description="false model full refresh is exported",
            model=build_test_model(
                name="protected_merge",
                config_values={
                    "materialized": "incremental",
                    "incremental_strategy": "merge",
                    "unique_key": ["id"],
                    "merge_exclude_columns": ["ingested_at"],
                    "full_refresh": False,
                },
            ),
            plan_entries=(),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.protected_merge",
            expected_resource_type="model",
            expected_database=None,
            expected_schema="public",
            expected_alias="protected_merge",
            expected_fqn=[_PROJECT, "models", "test"],
            expected_raw_code="SELECT 1",
            expected_compiled_code="SELECT 1",
            expected_relation_name="public.protected_merge",
            expected_description="",
            expected_materialized="incremental",
            expected_checksum_name="sha256",
            expected_merge_exclude_columns=["ingested_at"],
            expected_full_refresh=False,
        ),
        ManifestModelNodeTestCase(
            description="true model full refresh is exported",
            model=build_test_model(
                name="forced_refresh",
                config_values={
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": True,
                },
            ),
            plan_entries=(),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.forced_refresh",
            expected_resource_type="model",
            expected_database=None,
            expected_schema="public",
            expected_alias="forced_refresh",
            expected_fqn=[_PROJECT, "models", "test"],
            expected_raw_code="SELECT 1",
            expected_compiled_code="SELECT 1",
            expected_relation_name="public.forced_refresh",
            expected_description="",
            expected_materialized="incremental",
            expected_checksum_name="sha256",
            expected_full_refresh=True,
        ),
        ManifestModelNodeTestCase(
            description="named SQL hook exposes definition and preserves authored SQLBuild meta",
            model=build_test_model(
                name="hooked_orders",
                config_values={
                    "pre_hooks": [
                        SqlHookEntry(
                            statement="SELECT 'reader'",
                            name="record_access",
                            relative_path=Path("sql/hooks/sql/record_access.sql"),
                            definition_sql="SELECT @'role'",
                            kwargs={"role": "reader"},
                            description="Record access",
                        )
                    ]
                },
                meta={"sqlbuild": {"owner": "data-platform"}},
            ),
            plan_entries=(),
            project_name=_PROJECT,
            expected_unique_id=f"model.{_PROJECT}.hooked_orders",
            expected_resource_type="model",
            expected_database=None,
            expected_schema="public",
            expected_alias="hooked_orders",
            expected_fqn=[_PROJECT, "models", "test"],
            expected_raw_code="SELECT 1",
            expected_compiled_code="SELECT 1",
            expected_relation_name="public.hooked_orders",
            expected_description="",
            expected_materialized="view",
            expected_checksum_name="sha256",
            expected_meta={
                "sqlbuild": {
                    "owner": "data-platform",
                    "lifecycle_hooks": {
                        "pre_hooks": [
                            {
                                "type": "sql",
                                "statement": "SELECT 'reader'",
                                "name": "record_access",
                                "relative_path": "sql/hooks/sql/record_access.sql",
                                "definition_sql": "SELECT @'role'",
                                "kwargs": {"role": "reader"},
                                "description": "Record access",
                            }
                        ]
                    },
                }
            },
            expected_pre_hooks=[{"sql": "SELECT 'reader'", "transaction": True, "index": 0}],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_when_building_manifest_then_produces_correct_node(
    test_case: ManifestModelNodeTestCase,
) -> None:
    project: CompiledProjectView = build_test_project(models=(test_case.model,))
    plan_output: PlanOutput = build_test_plan_output(model_entries=test_case.plan_entries)

    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps={},
        downstream_deps={},
    )

    node: dict[str, Any] = manifest_nodes(result)[test_case.expected_unique_id]

    assert node["unique_id"] == test_case.expected_unique_id
    assert node["resource_type"] == test_case.expected_resource_type
    assert node["database"] == test_case.expected_database
    assert node["schema"] == test_case.expected_schema
    assert node["alias"] == test_case.expected_alias
    assert node["fqn"] == test_case.expected_fqn
    assert node["raw_code"] == test_case.expected_raw_code
    assert node["compiled_code"] == test_case.expected_compiled_code
    assert node["relation_name"] == test_case.expected_relation_name
    assert node["description"] == test_case.expected_description
    assert node["config"]["materialized"] == test_case.expected_materialized
    assert node["checksum"]["name"] == test_case.expected_checksum_name
    assert len(node["checksum"]["checksum"]) == 64
    column_name: str
    for column_name in test_case.expected_column_names:
        assert column_name in node["columns"]
    expected_col: str
    expected_type: str | None
    for expected_col, expected_type in test_case.expected_column_types.items():
        assert node["columns"][expected_col]["data_type"] == expected_type
    dep_id: str
    for dep_id in test_case.expected_depends_on_nodes:
        assert dep_id in node["depends_on"]["nodes"]
    tag: str
    for tag in test_case.expected_tags:
        assert tag in node["tags"]
    assert node["config"]["merge_exclude_columns"] == test_case.expected_merge_exclude_columns
    assert node["config"]["full_refresh"] is test_case.expected_full_refresh
    assert node["meta"] == test_case.expected_meta
    assert node["config"]["pre-hook"] == test_case.expected_pre_hooks


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestSourceNodeTestCase(
            description="produces correct source node fields",
            project_name=_PROJECT,
            expected_unique_id=f"source.{_PROJECT}.raw_orders",
            expected_resource_type="source",
            expected_database="raw_db",
            expected_schema="public",
            expected_identifier="orders_table",
            expected_description="Raw order data",
            expected_source_name="raw_orders",
        ),
        ManifestSourceNodeTestCase(
            description="uses source name as identifier when table is absent",
            project_name=_PROJECT,
            expected_unique_id=f"source.{_PROJECT}.events",
            expected_resource_type="source",
            expected_database=None,
            expected_schema="analytics",
            expected_identifier="events",
            expected_description="",
            expected_source_name="events",
        ),
        ManifestSourceNodeTestCase(
            description="source columns appear in manifest source node",
            project_name=_PROJECT,
            expected_unique_id=f"source.{_PROJECT}.raw_events",
            expected_resource_type="source",
            expected_database="raw_db",
            expected_schema="events",
            expected_identifier="clicks",
            expected_description="Raw click events",
            expected_source_name="raw_events",
            expected_column_names=("event_id", "ts"),
            expected_column_types={"event_id": "VARCHAR", "ts": "TIMESTAMP"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_when_building_manifest_then_produces_correct_node(
    test_case: ManifestSourceNodeTestCase,
) -> None:
    source: CompiledSourceView = _SOURCE_FIXTURES[test_case.expected_source_name]
    project: CompiledProjectView = build_test_project(sources=(source,))
    plan_output: PlanOutput = build_test_plan_output()

    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps={},
        downstream_deps={},
    )

    node: dict[str, Any] = manifest_sources(result)[test_case.expected_unique_id]

    assert node["unique_id"] == test_case.expected_unique_id
    assert node["resource_type"] == test_case.expected_resource_type
    assert node["database"] == test_case.expected_database
    assert node["schema"] == test_case.expected_schema
    assert node["identifier"] == test_case.expected_identifier
    assert node["description"] == test_case.expected_description
    col_name: str
    for col_name in test_case.expected_column_names:
        assert col_name in node["columns"]
    expected_col: str
    expected_type: str | None
    for expected_col, expected_type in test_case.expected_column_types.items():
        assert node["columns"][expected_col]["data_type"] == expected_type


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestSeedNodeTestCase(
            description="produces correct seed node with materialized seed",
            project_name=_PROJECT,
            expected_unique_id=f"seed.{_PROJECT}.country_codes",
            expected_resource_type="seed",
            expected_materialized="seed",
            expected_tags=("reference", "daily"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seed_when_building_manifest_then_produces_correct_node(
    test_case: ManifestSeedNodeTestCase,
) -> None:
    seed: CompiledSeedView = build_test_seed(name="country_codes", tags=test_case.expected_tags)
    project: CompiledProjectView = build_test_project(seeds=(seed,))
    plan_output: PlanOutput = build_test_plan_output()

    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps={},
        downstream_deps={},
    )

    node: dict[str, Any] = manifest_nodes(result)[test_case.expected_unique_id]

    assert node["unique_id"] == test_case.expected_unique_id
    assert node["resource_type"] == test_case.expected_resource_type
    assert node["config"]["materialized"] == test_case.expected_materialized
    assert tuple(node["tags"]) == test_case.expected_tags
    assert tuple(node["config"]["tags"]) == test_case.expected_tags


@pytest.mark.parametrize(
    "test_case",
    [
        FqnTestCase(
            description="builds fqn from nested relative path",
            project_name=_PROJECT,
            relative_path="sql/models/staging/orders.sql",
            expected_fqn=[_PROJECT, "models", "staging", "orders"],
        ),
        FqnTestCase(
            description="builds fqn from top-level relative path",
            project_name=_PROJECT,
            relative_path="sql/models/orders.sql",
            expected_fqn=[_PROJECT, "models", "orders"],
        ),
        FqnTestCase(
            description="builds fqn from seed path",
            project_name=_PROJECT,
            relative_path="sql/seeds/codes.csv",
            expected_fqn=[_PROJECT, "seeds", "codes"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relative_path_when_building_manifest_then_fqn_has_expected_parts(
    test_case: FqnTestCase,
) -> None:
    model: CompiledModelView = build_test_model(
        name="orders", relative_path=test_case.relative_path
    )

    result: dict[str, Any] = run_manifest(
        project=build_test_project(models=(model,)),
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps={},
        downstream_deps={},
    )

    node: dict[str, Any] = manifest_nodes(result)[f"model.{test_case.project_name}.orders"]
    assert node["fqn"] == test_case.expected_fqn


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestParentMapTestCase(
            description="parent_map reflects upstream dependencies",
            project_name=_PROJECT,
            expected_parent_entry=(
                f"model.{_PROJECT}.orders",
                [f"source.{_PROJECT}.raw_orders"],
            ),
            expected_child_entry=(
                f"source.{_PROJECT}.raw_orders",
                [f"model.{_PROJECT}.orders"],
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deps_when_building_manifest_then_parent_map_correct(
    test_case: ManifestParentMapTestCase,
) -> None:
    source: CompiledSourceView = build_test_source(name="raw_orders")
    model: CompiledModelView = build_test_model(name="orders", deps=(source_key("raw_orders"),))
    project: CompiledProjectView = build_test_project(models=(model,), sources=(source,))
    plan_output: PlanOutput = build_test_plan_output(
        model_entries=(build_test_plan_entry(name="orders"),),
    )

    orders_k: CompiledObjectKey = model_key("orders")
    raw_k: CompiledObjectKey = source_key("raw_orders")
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        orders_k: (raw_k,),
        raw_k: (),
    }
    downstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        raw_k: (orders_k,),
        orders_k: (),
    }

    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps=upstream,
        downstream_deps=downstream,
    )

    parent_key: str = test_case.expected_parent_entry[0]
    parent_values: list[str] = test_case.expected_parent_entry[1]
    assert result["parent_map"][parent_key] == parent_values

    child_key: str = test_case.expected_child_entry[0]
    child_values: list[str] = test_case.expected_child_entry[1]
    assert result["child_map"][child_key] == child_values


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestMacroNodeTestCase(
            description="macro appears in manifest macros dict",
            project_name=_PROJECT,
            expected_unique_id=f"macro.{_PROJECT}.my_macro",
            expected_name="my_macro",
            expected_resource_type="macro",
            expected_macro_sql="def my_macro(): return 'SELECT 1'",
            expected_path="sql/macros/python/helpers.py",
        ),
        ManifestMacroNodeTestCase(
            description="scoped macro exposes visibility metadata",
            project_name=_PROJECT,
            expected_unique_id=f"macro.{_PROJECT}.my_macro",
            expected_name="my_macro",
            expected_resource_type="macro",
            expected_macro_sql="def my_macro(): return 'SELECT 1'",
            expected_path="sql/models/sales/macros/python/helpers.py",
            expected_visibility="inherited",
            expected_scope_path="sql/models/sales",
            expected_dependencies=("base_macro",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_when_building_manifest_then_macro_node_present(
    test_case: ManifestMacroNodeTestCase,
) -> None:
    macro: LoadedMacro = LoadedMacro(
        name=test_case.expected_name,
        file_path=Path(f"/project/{test_case.expected_path}"),
        relative_path=Path(test_case.expected_path),
        raw_source=test_case.expected_macro_sql,
        function=lambda: "SELECT 1",
    )
    dependency_identities: tuple[DeclarationIdentity, ...] = tuple(
        DeclarationIdentity(DeclarationKind.MACRO, name) for name in test_case.expected_dependencies
    )
    declaration: DeclarationRecord = DeclarationRecord(
        identity=DeclarationIdentity(DeclarationKind.MACRO, test_case.expected_name),
        path=test_case.expected_path,
        line=1,
        column=1,
        scope=ScopeKind(test_case.expected_visibility),
        ownership_root=OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
        owning_path=test_case.expected_scope_path,
        macro=MacroMetadata(dependencies=dependency_identities),
    )
    dependency_records: tuple[DeclarationRecord, ...] = tuple(
        DeclarationRecord(
            identity=identity,
            path=f"sql/macros/{identity.name}.py",
            line=1,
            column=1,
            scope=ScopeKind.GLOBAL,
            ownership_root=OwnershipRoot("macros"),
            macro=MacroMetadata(),
        )
        for identity in dependency_identities
    )
    loaded_macros: dict[str, LoadedMacro] = {test_case.expected_name: macro}
    for dependency_name in test_case.expected_dependencies:
        loaded_macros[dependency_name] = LoadedMacro(
            name=dependency_name,
            file_path=Path(f"/project/macros/{dependency_name}.py"),
            relative_path=Path(f"sql/macros/{dependency_name}.py"),
            raw_source=f"def {dependency_name}(): return 'SELECT 1'",
            function=lambda: "SELECT 1",
        )
    project: CompiledProjectView = build_test_project(
        loaded_macros=loaded_macros,
    )
    project = project_with(
        project=project,
        scope_index=ScopeIndex(declarations=(declaration, *dependency_records)),
    )
    plan_output: PlanOutput = build_test_plan_output()

    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps={},
        downstream_deps={},
    )

    macro_node: dict[str, Any] = manifest_macros(result)[test_case.expected_unique_id]

    assert macro_node["name"] == test_case.expected_name
    assert macro_node["resource_type"] == test_case.expected_resource_type
    assert macro_node["macro_sql"] == test_case.expected_macro_sql
    assert macro_node["path"] == test_case.expected_path
    assert macro_node["meta"] == {
        "sqlbuild_visibility": test_case.expected_visibility,
        "sqlbuild_scope_path": test_case.expected_scope_path,
    }
    assert macro_node["depends_on"]["macros"] == [
        f"macro.{test_case.project_name}.{name}" for name in test_case.expected_dependencies
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestAuditNodeTestCase(
            description="audit plan entry produces dbt test node with test_metadata",
            project_name=_PROJECT,
            expected_unique_id=f"test.{_PROJECT}.not_null_orders_id",
            expected_resource_type="test",
            expected_name="not_null_orders_id",
            expected_sqlbuild_test_type="audit",
            expected_compiled_code_fragment="WHERE id IS NULL",
            expected_depends_on_nodes=(f"model.{_PROJECT}.orders",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_when_building_manifest_then_produces_test_node(
    test_case: ManifestAuditNodeTestCase,
) -> None:
    model: CompiledModelView = build_test_model(name="orders")
    project: CompiledProjectView = build_test_project(models=(model,))
    audit_entry: AuditPlanEntry = build_test_audit_plan_entry(
        name="not_null_orders_id",
        resolved_sql="SELECT id FROM staging.orders WHERE id IS NULL",
        scope_deps=(model_key("orders"),),
        attached_target_name="orders",
        attached_column_name="id",
    )
    plan_output: PlanOutput = build_test_plan_output(
        model_entries=(build_test_plan_entry(name="orders"),),
        audit_entries=(audit_entry,),
    )

    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps={},
        downstream_deps={},
    )

    node: dict[str, Any] = manifest_nodes(result)[test_case.expected_unique_id]

    assert node["unique_id"] == test_case.expected_unique_id
    assert node["resource_type"] == test_case.expected_resource_type
    assert node["name"] == test_case.expected_name
    assert node["meta"]["sqlbuild_test_type"] == test_case.expected_sqlbuild_test_type
    assert test_case.expected_compiled_code_fragment in node["compiled_code"]
    dep_id: str
    for dep_id in test_case.expected_depends_on_nodes:
        assert dep_id in node["depends_on"]["nodes"]


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestSqlTestNodeTestCase(
            description="sql test plan entry produces dbt test node with chain steps",
            project_name=_PROJECT,
            expected_unique_id=f"test.{_PROJECT}.test_orders",
            expected_resource_type="test",
            expected_name="test_orders",
            expected_sqlbuild_test_type="sql_native",
            expected_compiled_code_fragment="-- step: orders",
            expected_depends_on_nodes=(f"model.{_PROJECT}.orders",),
            unexpected_compiled_code_fragments=("-- step: stg_orders",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_test_when_building_manifest_then_produces_test_node(
    test_case: ManifestSqlTestNodeTestCase,
) -> None:
    model: CompiledModelView = build_test_model(name="orders")
    project: CompiledProjectView = build_test_project(models=(model,))
    chain: tuple[ChainStep, ...] = (
        ChainStep(model_name="stg_orders", resolved_sql=""),
        ChainStep(
            model_name="orders",
            resolved_sql="SELECT 1 AS id",
            expected_cte_sql="SELECT 1 AS id",
        ),
    )
    test_entry: SqlTestPlanEntry = build_test_sql_test_plan_entry(
        name="test_orders",
        chain=chain,
        scope_deps=(model_key("orders"),),
    )
    test_entry = replace(
        test_entry,
        source_path=Path("tests/unit/orders.sql"),
        block_index=1,
        parent_name="test_orders",
        case_name="first",
        case_index=0,
        case_fingerprint="f" * 64,
    )
    plan_output: PlanOutput = build_test_plan_output(
        model_entries=(build_test_plan_entry(name="orders"),),
        test_entries=(test_entry,),
    )

    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=_ADAPTER,
        upstream_deps={test_entry.key: (model.key,)},
        downstream_deps={model.key: (test_entry.key,)},
    )

    node: dict[str, Any] = manifest_nodes(result)[test_case.expected_unique_id]

    assert node["unique_id"] == test_case.expected_unique_id
    assert node["resource_type"] == test_case.expected_resource_type
    assert node["name"] == test_case.expected_name
    assert node["meta"]["sqlbuild_test_type"] == test_case.expected_sqlbuild_test_type
    assert test_case.expected_compiled_code_fragment in node["compiled_code"]
    for fragment in test_case.unexpected_compiled_code_fragments:
        assert fragment not in node["compiled_code"]
    assert node["checksum"] == {"name": "sha256", "checksum": "f" * 64}
    assert node["meta"]["sqlbuild_case_index"] == 0
    assert node["meta"]["sqlbuild_case_fingerprint"] == "f" * 64
    assert result["parent_map"][test_case.expected_unique_id] == [f"model.{_PROJECT}.orders"]
    assert f"model.{_PROJECT}.test_orders" not in result["parent_map"]
    assert test_case.expected_compiled_code_fragment in node["compiled_code"]
    dep_id: str
    for dep_id in test_case.expected_depends_on_nodes:
        assert dep_id in node["depends_on"]["nodes"]


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestSchemaValidationTestCase(
            description="full manifest validates against dbt v12 json schema",
            project_name=_PROJECT,
            adapter_type=_ADAPTER,
            expected_validation_error_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_full_project_when_building_manifest_then_validates_against_dbt_schema(
    test_case: ManifestSchemaValidationTestCase,
) -> None:
    import json

    from jsonschema import Draft202012Validator

    schema_path: Path = (
        Path(__file__).resolve().parents[5] / "fixtures" / "dbt_manifest_v12_schema.json"
    )
    schema: dict[str, Any] = json.loads(schema_path.read_text(encoding="utf-8"))

    model: CompiledModelView = build_test_model(
        name="orders",
        query_sql="SELECT id FROM raw",
        relative_path="sql/models/staging/orders.sql",
        schema="staging",
        config_values={"materialized": "table", "tags": ["core"]},
        description="Order records",
        columns=(SchemaColumn(name="id", type="INTEGER"),),
        deps=(source_key("raw_orders"),),
    )
    source: CompiledSourceView = build_test_source(
        name="raw_orders",
        database="raw_db",
        schema="public",
        table="orders",
    )
    seed: CompiledSeedView = build_test_seed(name="codes")
    project: CompiledProjectView = build_test_project(
        models=(model,),
        sources=(source,),
        seeds=(seed,),
        sql_hook_files=(
            DiscoveredSqlHookFile(
                file_path=Path("/project/hooks/sql/record_access.sql"),
                relative_path=Path("sql/hooks/sql/record_access.sql"),
                contents="HOOK (description 'Test hook record_access.');\n\nSELECT @'role'\n",
                header_values={"description": "Record access"},
                sql_body="SELECT @'role'",
                name="record_access",
                description="Record access",
            ),
        ),
    )
    plan_output: PlanOutput = build_test_plan_output(
        model_entries=(
            build_test_plan_entry(
                name="orders", resolved_sql="SELECT id FROM raw_db.public.orders"
            ),
        ),
        audit_entries=(
            build_test_audit_plan_entry(
                name="not_null_orders_id",
                scope_deps=(model_key("orders"),),
            ),
        ),
        test_entries=(
            build_test_sql_test_plan_entry(
                name="test_orders",
                chain=(
                    ChainStep(
                        model_name="orders", resolved_sql="SELECT 1", expected_cte_sql="SELECT 1"
                    ),
                ),
                scope_deps=(model_key("orders"),),
            ),
        ),
    )

    orders_key: CompiledObjectKey = model_key("orders")
    raw_key: CompiledObjectKey = source_key("raw_orders")
    upstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        orders_key: (raw_key,),
        raw_key: (),
    }
    downstream: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        raw_key: (orders_key,),
        orders_key: (),
    }

    macro: LoadedMacro = LoadedMacro(
        name="my_macro",
        file_path=Path("/project/macros/python/helpers.py"),
        relative_path=Path("sql/macros/python/helpers.py"),
        raw_source="def my_macro(): return 'SELECT 1'",
        function=lambda: "SELECT 1",
    )

    project = project_with(project=project, loaded_macros={"my_macro": macro})
    result: dict[str, Any] = run_manifest(
        project=project,
        plan_output=plan_output,
        project_name=test_case.project_name,
        adapter_type=test_case.adapter_type,
        upstream_deps=upstream,
        downstream_deps=downstream,
    )

    validator: Any = Draft202012Validator(schema)
    errors: list[Any] = list(validator.iter_errors(result))
    error_messages: list[str] = [f"{list(e.absolute_path)}: {e.message}" for e in errors]

    assert len(errors) == test_case.expected_validation_error_count, (
        "Schema validation errors:\n" + "\n".join(error_messages)
    )
    sql_hook_macro: dict[str, Any] = result["macros"][
        f"macro.{_PROJECT}__sqlbuild_hooks.record_access"
    ]
    assert sql_hook_macro["original_file_path"] == "sql/hooks/sql/record_access.sql"
    assert sql_hook_macro["meta"] == {
        "sqlbuild_resource_type": "sql_hook",
        "sqlbuild_hook_name": "record_access",
    }


@pytest.mark.parametrize(
    "test_case",
    [
        ManifestEncodingErrorTestCase(
            description="unencodable meta value raises when the manifest is written",
            meta={"owners": {"orders"}},
            expected_message="Object of type set is not JSON serializable",
        ),
        ManifestEncodingErrorTestCase(
            description="unencodable meta key raises when the manifest is written",
            meta={("orders", 1): "owner"},
            expected_message="keys must be str, int, float, bool or None, not tuple",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unencodable_meta_when_writing_manifest_then_raises_json_type_error(
    test_case: ManifestEncodingErrorTestCase,
) -> None:
    model: CompiledModelView = build_test_model(
        name="orders", meta=cast(dict[str, object], test_case.meta)
    )
    manifest: _native.NativeManifest = build_manifest(
        project=build_test_project(models=(model,)),
        project_name=_PROJECT,
        adapter_type=_ADAPTER,
        upstream_deps={},
        downstream_deps={},
    )

    with pytest.raises(TypeError, match=re.escape(test_case.expected_message)):
        manifest.contents()
