from __future__ import annotations

import itertools
import json
from collections.abc import Callable
from pathlib import Path
from types import MappingProxyType
from typing import TYPE_CHECKING, cast

from sqlbuild._native import (
    CollectionRendering,
    ColumnLineageConfidence,
    ColumnTransformKind,
    CompileAdapterContext,
    CompiledDirectLogicSqlTestPayload,
    CompiledLineageColumnFact,
    CompiledLineageSourceFact,
    CompiledModelSqlTestPayload,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSqlTest,
    CompilerDiagnostic,
    CompileSqlReference,
    DeclarationExpansionContext,
    DeclarationIdentity,
    DeclarationKind,
    DeclarationRecord,
    DeclarationResolutionContext,
    DeclarationRuntimeProjection,
    DeclarationScopeResolver,
    DiscoveredMacroFile,
    DiscoveredProjectInputs,
    InferredNullability,
    LoadedMacro,
    MacroMetadata,
    NativeColumnQuery,
    NativeQueryColumns,
    OwnershipRoot,
    ResourceIdentity,
    ResourceKind,
    ResourceRecord,
    ScopeIndex,
    ScopeKind,
    ScopeLookup,
    assemble_project,
    build_compile_inputs,
    build_execution_upstream_deps,
    build_manifest,
    build_native_declaration_contexts,
    build_scope_lookup,
    collect_compile_diagnostics,
    discover_project_inputs,
    expand_sql_macros,
    infer_native_query_columns,
    project_lineage_views,
    resolve_ref_references,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from sqlbuild.compiler.compile._helpers.render.user_macros import load_project_macros
from sqlbuild.compiler.compile.models import MacroContext
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView, ModelInput

DUCKDB_COMPILE_ADAPTER_CONTEXT: CompileAdapterContext = CompileAdapterContext(
    value_renderer=DuckDbAdapter(),
    collection_rendering=CollectionRendering.VALUE_LIST,
    python_functions_inherit_default_namespace=True,
    sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
    sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name,
)
DUCKDB_ARRAY_COMPILE_ADAPTER_CONTEXT: CompileAdapterContext = CompileAdapterContext(
    value_renderer=DUCKDB_COMPILE_ADAPTER_CONTEXT.value_renderer,
    collection_rendering=CollectionRendering.ARRAY,
    python_functions_inherit_default_namespace=True,
    sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
    sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name,
)
SQLSERVER_COMPILE_ADAPTER_CONTEXT: CompileAdapterContext = CompileAdapterContext(
    value_renderer=SqlServerAdapter(),
    collection_rendering=CollectionRendering.VALUE_LIST,
    python_functions_inherit_default_namespace=True,
    sql_lexical_syntax=SqlServerAdapter.sql_lexical_syntax,
    sql_analysis_dialect=SqlServerAdapter.sql_analysis_dialect_name,
)
DUCKDB_DECLARATION_EXPANSION_CONTEXT: DeclarationExpansionContext = DeclarationExpansionContext(
    declarations=DeclarationResolutionContext(),
    value_renderer=DUCKDB_COMPILE_ADAPTER_CONTEXT.value_renderer,
    collection_rendering=DUCKDB_COMPILE_ADAPTER_CONTEXT.collection_rendering,
)


def direct_orders_lineage(*column_names: str) -> tuple[CompiledLineageColumnFact, ...]:
    return tuple(
        CompiledLineageColumnFact(
            output_column=column_name,
            upstream_columns=(
                CompiledLineageSourceFact(
                    resource_type=CompiledResourceType.MODEL,
                    resource_name="orders",
                    column_name=column_name,
                ),
            ),
            transform_kind=ColumnTransformKind.DIRECT,
            confidence=ColumnLineageConfidence.HIGH,
        )
        for column_name in column_names
    )


def build_scoped_macro_resolver(
    *, tmp_path: Path, definitions: dict[str, tuple[str, ScopeKind, str | None, str]]
) -> tuple[dict[str, LoadedMacro], DeclarationScopeResolver]:
    """Build loaded macros and canonical scope projection for expansion tests."""

    loaded: dict[str, LoadedMacro] = {}
    records: list[DeclarationRecord] = []
    projection: dict[DeclarationIdentity, LoadedMacro] = {}
    for name, (output, scope, owner, authored_path) in definitions.items():
        relative_path: Path = Path(authored_path)

        def macro(*, _output: str = output) -> str:
            return _output

        item: LoadedMacro = LoadedMacro(name, tmp_path / relative_path, relative_path, "", macro)
        identity: DeclarationIdentity = DeclarationIdentity(DeclarationKind.MACRO, name)
        loaded[name] = item
        projection[identity] = item
        records.append(
            DeclarationRecord(
                identity=identity,
                path=relative_path.as_posix(),
                line=1,
                column=1,
                scope=scope,
                ownership_root=OwnershipRoot(relative_path.parts[0]),
                owning_path=owner,
                macro=MacroMetadata(),
            )
        )
    model_identity: ResourceIdentity = ResourceIdentity(ResourceKind.MODEL, "orders")
    index: ScopeIndex = ScopeIndex(
        resources=(
            ResourceRecord(
                model_identity,
                "models/marts/finance/orders.sql",
                OwnershipRoot("models", resource_kind=ResourceKind.MODEL),
            ),
        ),
        declarations=tuple(records),
    )
    lookup: ScopeLookup = build_scope_lookup(index=index)
    return loaded, DeclarationScopeResolver(
        project_dir=tmp_path,
        lookup=lookup,
        projection=DeclarationRuntimeProjection(MappingProxyType(projection)),
        native_contexts=build_native_declaration_contexts(lookup=lookup, declarations=projection),
    )


def build_loaded_macros(tmp_path: Path, macro_file_contents: str) -> dict[str, LoadedMacro]:
    macros_dir: Path = tmp_path / "macros" / "python"
    macros_dir.mkdir(parents=True, exist_ok=True)
    macro_file_path: Path = macros_dir / "common.py"
    macro_file_path.write_text(macro_file_contents, encoding="utf-8")
    return load_project_macros(
        (
            DiscoveredMacroFile(
                file_path=macro_file_path,
                relative_path=Path("macros/python/common.py"),
                contents=macro_file_contents,
            ),
        )
    )


def compile_first_model(*, project_dir: Path) -> CompiledModelView:
    """Compile a fixture project and return its first model."""

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run",
    )
    compiled_project: CompiledProjectView = assemble_project(
        inputs=compile_inputs,
        skip_column_inference=True,
    )
    return compiled_project.models[0]


def compile_project_inputs(*, project_dir: Path) -> CompileInputsView:
    """Compile a discovered DuckDB fixture through the input attachment boundary."""

    return build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run",
    )


def compiled_sql_test_expected_model_names(test: CompiledSqlTest) -> tuple[str, ...]:
    getters: MappingProxyType[type[object], Callable[[object], tuple[str, ...]]] = MappingProxyType(
        {
            CompiledModelSqlTestPayload: _compiled_model_expected_names,
            CompiledDirectLogicSqlTestPayload: _compiled_direct_empty_names,
        }
    )
    return getters[type(test.payload)](test.payload)


def compiled_sql_test_tested_resource_names(test: CompiledSqlTest) -> tuple[str, ...]:
    getters: MappingProxyType[type[object], Callable[[object], tuple[str, ...]]] = MappingProxyType(
        {
            CompiledModelSqlTestPayload: _compiled_model_empty_names,
            CompiledDirectLogicSqlTestPayload: _compiled_direct_tested_names,
        }
    )
    return getters[type(test.payload)](test.payload)


def _compiled_model_expected_names(payload: object) -> tuple[str, ...]:
    return cast(CompiledModelSqlTestPayload, payload).expected_model_names


def _compiled_direct_empty_names(payload: object) -> tuple[str, ...]:
    del payload
    return ()


def _compiled_model_empty_names(payload: object) -> tuple[str, ...]:
    del payload
    return ()


def _compiled_direct_tested_names(payload: object) -> tuple[str, ...]:
    return cast(CompiledDirectLogicSqlTestPayload, payload).tested_resource_names


def singular_audit_files(*, base: dict[str, str], sql: str) -> dict[str, str]:
    """Return project files with one top-level singular audit over the given SQL."""

    return base | {"audits/singular/check.sql": f"AUDIT ();\n{sql}"}


def model_header(*, key: str, value: str) -> str:
    """Return a one-column model whose header sets one key."""

    return f"MODEL (description 'Test model check.', {key} {value});\nSELECT 1 AS order_id"


def compile_and_assemble(*, project_dir: Path) -> CompiledProjectView:
    """Attach and assemble a DuckDB fixture so scope placement is validated."""

    return assemble_project(
        inputs=compile_project_inputs(project_dir=project_dir), skip_column_inference=True
    )


def gate_audit(*, read: str) -> str:
    """Return a generic audit that joins its target to another resource."""

    return f"AUDIT ();\nSELECT s.* FROM @relation s LEFT JOIN {read} a USING (code)"


def gate_model(*, sql: str, header: str = "MODEL (description 'Test model.');") -> str:
    """Return a model with the given header."""

    return f"{header}\n{sql}"


def execution_edge_names(*, project: CompiledProjectView) -> frozenset[tuple[str, str]]:
    """Return (node, upstream) name pairs of the project's execution graph."""

    edges: set[tuple[str, str]] = set()
    key: CompiledObjectKey
    deps: tuple[CompiledObjectKey, ...]
    for key, deps in build_execution_upstream_deps(project).items():
        edges.update((key.name, dep.name) for dep in deps)
    return frozenset(edges)


def inline_sql_hook_header(sql: str) -> str:
    """Return a MODEL header running one inline SQL pre-hook."""

    return f"MODEL (description 'Test model.', pre_hooks [inline_sql('{sql}')]);"


def lineage_edge_names(*, project: CompiledProjectView) -> frozenset[tuple[str, str]]:
    """Return (node, upstream) name pairs of the project's lineage graph used for selection."""

    edges: set[tuple[str, str]] = set()
    key: CompiledObjectKey
    deps: tuple[CompiledObjectKey, ...]
    for key, deps in project_lineage_views(project).upstream_deps.items():
        edges.update((key.name, dep.name) for dep in deps)
    return frozenset(edges)


def expand_typed_macro_sql(
    *, tmp_path: Path, macro_file_contents: str, sql: str, enforce_explicit: bool = True
) -> str:
    """Expand model SQL for the ``order_summary`` model against one macro module."""

    loaded_macros: dict[str, LoadedMacro] = build_loaded_macros(tmp_path, macro_file_contents)
    return expand_sql_macros(
        sql=sql,
        file_path=tmp_path / "models" / "order_summary.sql",
        loaded_macros=loaded_macros,
        macro_context=MacroContext(
            adapter_name="duckdb",
            sql_analysis_enabled=True,
            target_name="dev",
            _enforce_explicit_references=enforce_explicit,
        ),
        consumer=ResourceIdentity(kind=ResourceKind.MODEL, name="order_summary"),
    ).sql


def resolve_model_references_for_adapter(*, sql: str, adapter_name: str) -> str:
    """Resolve ``__ref("orders")`` to ``analytics.sales.orders`` with one adapter's quoting."""

    adapters: dict[str, Callable[[], BaseAdapter]] = {
        "bigquery": BigQueryAdapter,
        "duckdb": DuckDbAdapter,
        "postgres": PostgresAdapter,
        "snowflake": SnowflakeAdapter,
        "sqlserver": SqlServerAdapter,
    }
    return resolve_ref_references(
        query_sql=sql,
        model_locations={
            "orders": CompiledRelationLocation(
                database="analytics", schema="sales", name="orders", qualified_name=None
            )
        },
        seed_locations={},
        cursor_bounds=None,
        cursor_filter_inputs={},
        adapter=adapters[adapter_name](),
        cursor_type=None,
        lower_bound_inclusive=True,
    )


def python_hook_source(*, reads: str = "()", body: str = "    return None\n") -> str:
    """Return a Python hook module defining ``refresh_lookup`` with declared reads."""

    return (
        "from sqlbuild.hooks import hook\n"
        "from sqlbuild.refs import model, seed, source\n\n\n"
        f"@hook(reads={reads})\n"
        "def refresh_lookup(ctx):\n"
        f"{body}"
    )


def python_task_source(*, body: str) -> str:
    """Return a Python task module defining ``export_orders`` that depends on ``orders``."""

    return (
        "from sqlbuild.refs import model\n"
        "from sqlbuild.tasks import task\n\n\n"
        '@task(depends_on=model("orders"))\n'
        "def export_orders(ctx):\n"
        f"{body}"
    )


def python_check_source(*, depends_on: str, body: str) -> str:
    """Return a Python check module defining ``orders_present`` with the given dependencies."""

    return (
        "from sqlbuild.checks import check\n"
        "from sqlbuild.refs import model\n\n\n"
        f"@check(depends_on={depends_on})\n"
        "def orders_present(ctx):\n"
        f"{body}"
        "    return ctx.pass_()\n"
    )


def python_loader_source(*, depends_on: str, body: str) -> str:
    """Return loaders ``raw_regions`` and ``raw_customers``; the latter runs ``body``."""

    return (
        "from sqlbuild.loaders import loader\n\n\n"
        "@loader\n"
        "def raw_regions(ctx):\n"
        "    '''Test loader raw_regions.'''\n    return [{'id': 1}]\n\n\n"
        f"@loader(depends_on=[{depends_on}])\n"
        "def raw_customers(ctx):\n"
        f"{body}"
        "    return [{'id': 1}]\n"
    )


def collect_typed_macro_violations(
    *, tmp_path: Path, macro_file_contents: str, sql: str
) -> tuple[CompilerDiagnostic, ...]:
    """Expand ``order_summary`` SQL and return the explicit-reference violations it reports."""

    with collect_compile_diagnostics() as violations:
        expand_typed_macro_sql(tmp_path=tmp_path, macro_file_contents=macro_file_contents, sql=sql)
    return violations.diagnostics


def render_compile_diagnostics(*, project: CompiledProjectView) -> str:
    """Render every project diagnostic with its code, message, location, and help."""

    return "\n".join(
        f"[{diagnostic.code}] {diagnostic.message} --> {diagnostic.path}:{diagnostic.line} "
        f"= help: {diagnostic.help}"
        for diagnostic in project.diagnostics
    )


def write_scoped_macro_orders_project(project_dir: Path) -> None:
    """Write a project with one global macro and one test-scoped macro."""

    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model: Path = project_dir / "models" / "orders.sql"
    model.parent.mkdir(parents=True)
    model.write_text(
        'MODEL (description "Orders");\nSELECT 1 AS order_id WHERE @py.order_filter("1")\n',
        encoding="utf-8",
    )
    (project_dir / "macros" / "python").mkdir(parents=True)
    (project_dir / "macros" / "python" / "order_filter.py").write_text(
        'def order_filter(column: str) -> str:\n    return f"{column} > 0"\n', encoding="utf-8"
    )
    tests_dir: Path = project_dir / "tests" / "unit"
    (tests_dir / "_macros" / "python").mkdir(parents=True)
    (tests_dir / "_macros" / "python" / "row_limit.py").write_text(
        'def row_limit() -> str:\n    return "LIMIT 1"\n', encoding="utf-8"
    )
    (tests_dir / "test_orders.sql").write_text(
        "TEST ();\nWITH\n__ref__orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT order_id FROM __ref__orders @py.row_limit())\n"
        "SELECT 1\n",
        encoding="utf-8",
    )


def usage_consumers(model_input: ModelInput) -> set[ResourceIdentity | DeclarationIdentity]:
    """The consumers a model input's declaration usages are recorded for."""

    return {usage.consumer for usage in model_input.declaration_usages}


def reanalysed_columns(
    *,
    query_sql: str,
    references: tuple[CompileSqlReference, ...] = (),
    column_nullability_by_table: dict[str, dict[str, InferredNullability]] | None = None,
    column_types_by_table: dict[str, dict[str, str]] | None = None,
    inference_profile: ExpressionInferenceProfile | None = None,
    allow_compact_analysis: bool = True,
    recover_cte_facts: bool = True,
) -> NativeQueryColumns:
    """Native compact re-analysis over known input types, as source bindings run it."""

    assert allow_compact_analysis and recover_cte_facts
    values: list[InferredNullability] = list(
        itertools.chain.from_iterable(
            map(dict.values, (column_nullability_by_table or {}).values())
        )
    )
    assert all(value == InferredNullability.UNKNOWN for value in values)
    return infer_native_query_columns(
        queries=(NativeColumnQuery(sql=query_sql, mode="reanalysis", references=references),),
        profile=inference_profile or ExpressionInferenceProfile(),
        column_types_by_table=column_types_by_table,
    )[0]


def manifest_model_node(*, project: CompiledProjectView, name: str) -> dict[str, object]:
    """Build the project's manifest and return one model node from its published JSON."""

    manifest: dict[str, dict[str, dict[str, object]]] = json.loads(
        build_manifest(
            project=project,
            project_name="demo",
            adapter_type="duckdb",
            upstream_deps={},
            downstream_deps={},
        ).contents()
    )
    return manifest["nodes"][f"model.demo.{name}"]
