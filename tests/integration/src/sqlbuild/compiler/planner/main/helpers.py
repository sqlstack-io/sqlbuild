"""Test helpers for planner orchestration integration tests."""

from __future__ import annotations

import re
from collections import Counter
from collections.abc import Callable
from dataclasses import fields, replace
from datetime import UTC, datetime
from itertools import chain
from pathlib import Path
from tempfile import gettempdir
from types import MappingProxyType
from typing import Any, cast

from sqlbuild._native import (
    CompiledFunction,
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSeed,
    CompiledSource,
    CompileModelConfig,
    CompilePipelineOptions,
    CompilePipelineResult,
    CompileSqlReference,
    DeferralInputs,
    Dialect,
    DiscoveredProjectInputs,
    DiscoveredSchemaFile,
    DiscoveredSeedFile,
    DiscoveredSourceFile,
    Fingerprint,
    FunctionArgument,
    PlannerOverrides,
    PlannerPolicies,
    PlannerSelection,
    PlanOutput,
    SchemaSeedEntry,
    SettingsConfig,
    SourceEntry,
    SqlReferenceKind,
    build_compiled_function_fingerprint_sql,
    build_execution_plan,
    compiled_model_record,
    compute_query_hash,
    discover_project_inputs,
    live_reads,
    project_with,
    run_compile_pipeline,
    write_fingerprint,
)
from sqlbuild.adapter.contract.models import RelationInfo, RelationReadProbe
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import (
    CompiledFunctionView,
    CompiledModelView,
    CompiledProjectView,
    CompiledSeedView,
    CompiledSourceView,
    FunctionLanguage,
)
from sqlbuild.runtime.contracts.models import ConnectionHooks
from tests.integration.src.sqlbuild.compiler.planner.main._test_types import (
    BuildExecutionPlanTestCase,
    FormatPlanIntegrationTestCase,
    SourceCursorInputPlanErrorTestCase,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeRelation,
    OfflineSnowflakeAdapter,
    RecordedQuery,
    RecordingSnowflakeWarehouse,
    SyntheticSnowflakeProject,
)


def build_project_from_test_case(
    test_case: BuildExecutionPlanTestCase,
) -> CompiledProjectView:
    """Build a CompiledProject from an integration test case."""

    models: list[CompiledModelView] = []
    model_name: str
    target_schema: str
    for model_name, target_schema in test_case.model_locations.items():
        config_values: dict[str, object] = test_case.model_configs.get(model_name, {})
        query_sql: str = test_case.model_queries.get(model_name, f"SELECT * FROM {model_name}")
        dep_names: tuple[str, ...] = test_case.model_deps.get(model_name, ())
        model_deps: tuple[CompiledObjectKey, ...] = tuple(
            CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=d) for d in dep_names
        )
        function_deps: tuple[CompiledObjectKey, ...] = tuple(
            CompiledObjectKey(resource_type=CompiledResourceType.UDF, name=d)
            for d in test_case.function_deps.get(model_name, ())
        )
        models.append(
            CompiledModel(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.MODEL,
                    name=model_name,
                ),
                deps=(*model_deps, *function_deps),
                name=model_name,
                relative_path=Path(f"models/{model_name}.sql"),
                query_sql=query_sql,
                config=CompileModelConfig(values=config_values),
                destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=model_name,
                    qualified_name=f"{target_schema}.{model_name}",
                ),
            )
        )

    seeds: list[CompiledSeedView] = []
    seed_name: str
    for seed_name, target_schema in test_case.seed_locations.items():
        seed_file_path: Path = _ensure_test_seed_file(seed_name)
        seeds.append(
            CompiledSeed(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.SEED,
                    name=seed_name,
                ),
                deps=(),
                name=seed_name,
                seed_file=DiscoveredSeedFile(
                    file_path=seed_file_path,
                    relative_path=Path(f"seeds/{seed_name}.csv"),
                ),
                schema_entry=SchemaSeedEntry(name=seed_name, columns=()),
                schema_file=DiscoveredSchemaFile(
                    file_path=Path("seeds/schema.yml"),
                    relative_path=Path("seeds/schema.yml"),
                    contents="",
                    model_entries=(),
                    seed_entries=(),
                ),
                destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=seed_name,
                    qualified_name=f"{target_schema}.{seed_name}",
                ),
            )
        )

    functions: list[CompiledFunctionView] = []
    function_name: str
    for function_name, target_schema in test_case.function_locations.items():
        body_sql: str = test_case.function_bodies.get(function_name, "value = 1")
        language: FunctionLanguage = test_case.function_languages.get(
            function_name, FunctionLanguage.SQL
        )
        functions.append(
            CompiledFunction(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.UDF,
                    name=function_name,
                ),
                deps=(),
                name=function_name,
                relative_path=Path(f"functions/{language.value}/{function_name}.sql"),
                arguments=(FunctionArgument(name="value", type="INTEGER"),),
                returns="INTEGER",
                body_sql=body_sql,
                destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=function_name,
                    qualified_name=f"{target_schema}.{function_name}",
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=function_name,
                    qualified_name=f"{target_schema}.{function_name}",
                ),
                language=language,
                entry_point=(None, "main")[language == FunctionLanguage.PYTHON],
            )
        )

    return CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection=test_case.effective_connection,
        effective_vars={},
        settings=SettingsConfig(
            sql_analysis=_settings_bool(
                test_case.effective_connection, "sql_analysis", default=False
            )
        ),
        models=tuple(models),
        seeds=tuple(seeds),
        functions=tuple(functions),
    )


def _settings_bool(settings: dict[str, object], key: str, *, default: bool) -> bool:
    """Read a boolean setting from legacy test-case settings dictionaries."""

    raw_value: object | None = settings.get(key)
    return cast(bool, (default, raw_value)[isinstance(raw_value, bool)])


def build_project_from_source_cursor_input_test_case(
    test_case: SourceCursorInputPlanErrorTestCase,
) -> CompiledProjectView:
    """Build a project with one incremental model reading one source."""

    source_entry: SourceEntry = SourceEntry(
        name=test_case.source_name,
        schema=test_case.source_schema,
        table=test_case.source_table,
    )
    source: CompiledSourceView = CompiledSource(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SOURCE,
            name=test_case.source_name,
        ),
        deps=(),
        name=test_case.source_name,
        source_entry=source_entry,
        source_file=DiscoveredSourceFile(
            file_path=Path("sources/raw.yml"),
            relative_path=Path("sources/raw.yml"),
            contents="",
            source_entries=(source_entry,),
        ),
    )
    model: CompiledModelView = CompiledModel(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name=test_case.model_name,
        ),
        deps=(source.key,),
        name=test_case.model_name,
        relative_path=Path(f"models/{test_case.model_name}.sql"),
        query_sql=f'SELECT * FROM __source("{test_case.source_name}")',
        references=(
            CompileSqlReference(
                ref_kind=SqlReferenceKind.SOURCE,
                ref_name=test_case.source_name,
            ),
        ),
        config=CompileModelConfig(
            values={
                "materialized": "incremental",
                "cursor": test_case.cursor_column,
                "cursor_inputs": {test_case.source_name: test_case.cursor_input_column},
            }
        ),
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name=test_case.model_name,
            qualified_name=f"staging.{test_case.model_name}",
        ),
    )
    return CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        settings=SettingsConfig(),
        models=(model,),
        sources=(source,),
    )


def build_future_cursor_project() -> CompiledProjectView:
    """Build one timestamp incremental source project for future safety tests."""

    case: SourceCursorInputPlanErrorTestCase = SourceCursorInputPlanErrorTestCase(
        description="future cursor",
        setup_sql=(),
        model_name="events_incremental",
        source_name="raw_events",
        source_schema="raw",
        source_table="events",
        cursor_column="occurred_at",
        cursor_input_column="occurred_at",
        expected_error_fragment="",
    )
    project: CompiledProjectView = build_project_from_source_cursor_input_test_case(case)
    model: CompiledModelView = project.models[0]
    return project_with(
        project=project,
        models=(
            replace(
                compiled_model_record(model),
                config=replace(
                    model.config,
                    values={
                        **model.config.values,
                        "cursor_type": "timestamp",
                        "cursor_grain": "day",
                        "incremental_strategy": "delete_insert",
                    },
                ),
            ),
        ),
    )


def write_previous_function_fingerprints(
    *,
    test_case: BuildExecutionPlanTestCase,
    adapter: DuckDbAdapter,
    connection: Any,
) -> None:
    """Write previous function definition fingerprints for planner tests."""

    previous_case: BuildExecutionPlanTestCase = replace(
        test_case,
        function_bodies={**test_case.function_bodies, **test_case.previous_function_bodies},
    )
    previous_project: CompiledProjectView = build_project_from_test_case(previous_case)
    function_map: dict[str, CompiledFunctionView] = {
        function.name: function for function in previous_project.functions
    }
    function_name: str
    for function_name in test_case.previous_function_bodies:
        function: CompiledFunctionView = function_map[function_name]
        fingerprint_sql: str = build_compiled_function_fingerprint_sql(function)
        _ = write_fingerprint(
            connection=connection,
            execute=adapter.execute,
            database=function.destination.database,
            schema=function.destination.schema or "",
            fingerprint=Fingerprint(
                node_type=("udf", "table_fn")[bool(function.return_columns)],
                node_name=function.name,
                target_database=function.destination.database,
                target_schema=function.destination.schema,
                target_name=function.destination.name,
                run_id="previous_run",
                definition_hash=compute_query_hash(query_sql=fingerprint_sql, dialect=None),
                schema_fingerprint=compute_query_hash(query_sql="", dialect=None),
                definition=fingerprint_sql,
                ts=datetime.now(tz=UTC),
            ),
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        )


def build_project_from_format_test_case(
    test_case: FormatPlanIntegrationTestCase,
) -> CompiledProjectView:
    """Build a CompiledProject from a format integration test case."""

    models: list[CompiledModelView] = []
    model_name: str
    target_schema: str
    for model_name, target_schema in test_case.model_locations.items():
        config_values: dict[str, object] = test_case.model_configs.get(model_name, {})
        query_sql: str = test_case.model_queries.get(model_name, f"SELECT * FROM {model_name}")
        dep_names: tuple[str, ...] = test_case.model_deps.get(model_name, ())
        deps: tuple[CompiledObjectKey, ...] = tuple(
            CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=d) for d in dep_names
        )
        models.append(
            CompiledModel(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.MODEL,
                    name=model_name,
                ),
                deps=deps,
                name=model_name,
                relative_path=Path(f"models/{model_name}.sql"),
                query_sql=query_sql,
                config=CompileModelConfig(values=config_values),
                destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=model_name,
                    qualified_name=f"{target_schema}.{model_name}",
                ),
            )
        )

    seeds: list[CompiledSeedView] = []
    seed_name: str
    for seed_name, target_schema in test_case.seed_locations.items():
        seed_file_path: Path = _ensure_test_seed_file(seed_name)
        seeds.append(
            CompiledSeed(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.SEED,
                    name=seed_name,
                ),
                deps=(),
                name=seed_name,
                seed_file=DiscoveredSeedFile(
                    file_path=seed_file_path,
                    relative_path=Path(f"seeds/{seed_name}.csv"),
                ),
                schema_entry=SchemaSeedEntry(name=seed_name, columns=()),
                schema_file=DiscoveredSchemaFile(
                    file_path=Path("seeds/schema.yml"),
                    relative_path=Path("seeds/schema.yml"),
                    contents="",
                    model_entries=(),
                    seed_entries=(),
                ),
                destination=CompiledRelationLocation(
                    database=None,
                    schema=target_schema,
                    name=seed_name,
                    qualified_name=f"{target_schema}.{seed_name}",
                ),
            )
        )

    return CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection=test_case.effective_connection,
        effective_vars={},
        models=tuple(models),
        seeds=tuple(seeds),
    )


def _ensure_test_seed_file(seed_name: str) -> Path:
    seed_dir: Path = Path(gettempdir()) / "sqlbuild_planner_seed_fixtures"
    seed_dir.mkdir(parents=True, exist_ok=True)
    seed_path: Path = seed_dir / f"{seed_name}.csv"
    _SEED_FILE_WRITERS[seed_path.exists()](seed_path)
    return seed_path


def _write_seed_file(seed_path: Path) -> None:
    seed_path.write_text("id,code\n1,US\n", encoding="utf-8")


def _keep_seed_file(seed_path: Path) -> None:
    del seed_path


_SEED_FILE_WRITERS: MappingProxyType[bool, Callable[[Path], None]] = MappingProxyType(
    {False: _write_seed_file, True: _keep_seed_file}
)


def build_execution_plan_from_kwargs(**kwargs: Any) -> PlanOutput:
    """Adapt flat planner kwargs to the grouped build_execution_plan inputs."""

    def grouped(model: type) -> dict[str, Any]:
        names: frozenset[str] = frozenset(field.name for field in fields(model))
        return {name: kwargs.pop(name) for name in names & kwargs.keys()}

    selection: PlannerSelection = PlannerSelection(**grouped(PlannerSelection))
    overrides: PlannerOverrides = PlannerOverrides(**grouped(PlannerOverrides))
    deferral: DeferralInputs = DeferralInputs(**grouped(DeferralInputs))
    policies: PlannerPolicies = PlannerPolicies(**grouped(PlannerPolicies))
    adapter: Any = kwargs.pop("adapter")
    connection: Any = kwargs.pop("connection")
    return build_execution_plan(
        dialect=Dialect(adapter),
        reads=live_reads(adapter=adapter, connection=connection),
        selection=selection,
        overrides=overrides,
        deferral=deferral,
        policies=policies,
        **kwargs,
    )


_OFFLINE_METADATA_KINDS: frozenset[str] = frozenset(
    {
        "tables",
        "columns",
        "show_columns",
        "show_tables",
        "show_views",
        "show_schema_columns",
        "other_metadata",
    }
)
_OFFLINE_INFORMATION_SCHEMA_KINDS: frozenset[str] = frozenset({"tables", "columns"})
_SHOW_SCHEMA_SCOPE: re.Pattern[str] = re.compile(r' IN SCHEMA (?:"[^"]*"\.)?"([^"]*)"')
_CURSOR_BOUND_RELATION: re.Pattern[str] = re.compile(r"\sFROM\s+(\S+)\s*$", re.IGNORECASE | re.M)


def plan_offline_snowflake_project(
    *,
    project: SyntheticSnowflakeProject,
    warehouse: RecordingSnowflakeWarehouse,
    no_cache: bool = True,
) -> CompilePipelineResult:
    """Compile and plan a synthetic Snowflake project against the recording warehouse."""

    inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=project.project_dir)
    return run_compile_pipeline(
        discovered_inputs=inputs,
        adapter=OfflineSnowflakeAdapter(warehouse=warehouse),
        options=CompilePipelineOptions(
            no_sql_validation=True,
            no_cache=no_cache,
            connection_config=dict(inputs.project_config.connection),
        ),
        hooks=ConnectionHooks(),
    )


def offline_metadata_queries(warehouse: RecordingSnowflakeWarehouse) -> tuple[RecordedQuery, ...]:
    """Return every catalog metadata statement the warehouse received."""

    return tuple(filter(lambda query: query.kind in _OFFLINE_METADATA_KINDS, warehouse.queries))


def offline_metadata_reads_by_schema(
    *, warehouse: RecordingSnowflakeWarehouse, kind: str
) -> dict[str, int]:
    """Count schema-scoped SHOW reads of one kind by the schema they list."""

    return dict(
        Counter(
            _SHOW_SCHEMA_SCOPE.findall(query.sql)[0] for query in warehouse.queries_of_kind(kind)
        )
    )


def offline_information_schema_queries(
    warehouse: RecordingSnowflakeWarehouse,
) -> tuple[RecordedQuery, ...]:
    """Return metadata statements that need a running warehouse."""

    return tuple(
        filter(lambda query: query.kind in _OFFLINE_INFORMATION_SCHEMA_KINDS, warehouse.queries)
    )


def offline_cursor_bound_relations(warehouse: RecordingSnowflakeWarehouse) -> tuple[str, ...]:
    """Return every relation the cursor-bound statements read, in statement order."""

    return tuple(
        chain.from_iterable(
            _CURSOR_BOUND_RELATION.findall(query.sql)
            for query in warehouse.queries_of_kind("cursor_bounds")
        )
    )


def relations_in_schemas(
    *, project: SyntheticSnowflakeProject, schemas: frozenset[str]
) -> tuple[FakeRelation, ...]:
    """Keep only the synthetic relations that already exist in the given schemas."""

    return tuple(filter(lambda relation: relation.schema in schemas, project.relations))


def relations_in_database(
    *, project: SyntheticSnowflakeProject, database: str
) -> tuple[FakeRelation, ...]:
    """Move every synthetic relation into another database."""

    return tuple(replace(relation, database=database) for relation in project.relations)


class CountingSourceDuckDbAdapter(DuckDbAdapter):
    """Plan against one shared DuckDB connection, counting source listings and probes."""

    def __init__(self, *, connection: Any) -> None:
        super().__init__()
        self._shared_connection: Any = connection
        self.listed_schema_sets: list[tuple[str, ...]] = []
        self.probed_relations: list[str] = []

    def connect(self, config: dict[str, Any]) -> Any:
        del config
        return self._shared_connection

    def close(self, connection: Any) -> None:
        del connection

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        self.listed_schema_sets.append(tuple(schema.lower() for schema in schemas or ()))
        return super().list_relations(
            connection=connection, database=database, schemas=schemas, names=names
        )

    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        self.probed_relations.append(relation)
        return super().probe_relation_read(connection=connection, relation=relation)


class UnlistedDatabaseDuckDbAdapter(CountingSourceDuckDbAdapter):
    """Plan against one shared DuckDB connection whose listings of one database fail."""

    def __init__(self, *, connection: Any, unlisted_database: str) -> None:
        super().__init__(connection=connection)
        self._unlisted_database: str = unlisted_database
        self.listed_databases: list[str | None] = []

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        self.listed_databases.append(database)
        _LISTING_GUARDS[database == self._unlisted_database](database)
        return super().list_relations(
            connection=connection, database=database, schemas=schemas, names=names
        )


def _allow_listing(database: str | None) -> None:
    del database


def _refuse_listing(database: str | None) -> None:
    raise RuntimeError(f"cannot list database {database}")


_LISTING_GUARDS: MappingProxyType[bool, Callable[[str | None], None]] = MappingProxyType(
    {False: _allow_listing, True: _refuse_listing}
)


def plan_deferred_project(
    *,
    project_dir: Path,
    adapter: DuckDbAdapter,
    project_files: dict[str, str],
    select: tuple[str, ...],
) -> CompilePipelineResult:
    """Write a project with `dev` and `prod` targets and plan `select` deferring to prod."""

    relative_path: str
    contents: str
    for relative_path, contents in project_files.items():
        (project_dir / relative_path).parent.mkdir(parents=True, exist_ok=True)
        (project_dir / relative_path).write_text(contents, encoding="utf-8")
    return run_compile_pipeline(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=adapter,
        options=CompilePipelineOptions(no_sql_validation=True, defer_to="prod", select=select),
    )


def plan_source_reading_project(
    *, project_dir: Path, adapter: DuckDbAdapter, source_schema: str, source_table: str
) -> CompilePipelineResult:
    """Write and plan one model reading one declared table source."""

    project_files: dict[str, str] = {
        "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            f"    schema: {source_schema}\n"
            f"    table: {source_table}\n"
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.", '
            'materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
        ),
    }
    relative_path: str
    contents: str
    for relative_path, contents in project_files.items():
        (project_dir / relative_path).parent.mkdir(parents=True, exist_ok=True)
        (project_dir / relative_path).write_text(contents, encoding="utf-8")
    return run_compile_pipeline(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=adapter,
        options=CompilePipelineOptions(no_sql_validation=True),
    )
