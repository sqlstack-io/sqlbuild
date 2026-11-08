"""Test helpers for sql_test_assembly tests."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from types import MappingProxyType

from sqlbuild._native import (
    CompiledFunction,
    CompiledModel,
    CompiledModelSqlTestPayload,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSqlTest,
    CompileModelConfig,
    CompileSqlTestCte,
    DiscoveredSqlTestBlock,
    DiscoveredSqlTestFile,
    LoadedMacro,
    NativeSqlTestPlan,
    PlanWarning,
    SqlLexicalSyntax,
    SqlReferenceKind,
    SqlTestPlanEntry,
    SqlTestPlanResult,
    expand_sql_macros,
    extract_sql_references,
    plan_compiled_sql_test_artifacts,
    plan_sql_tests,
    plan_sql_tests_natively,
    query_fingerprint,
    render_sql_test_comparison_sql,
    sql_test_plan_error_messages,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.compile.models import MacroContext
from sqlbuild.compiler.compile.types import (
    CompiledFunctionView,
    CompiledModelView,
    CompiledProjectView,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly._test_types import (
    PlanTestChainTestCase,
)

ASSERT_TEST_CTE_PREFIX: str = "__assert__"
DBT_REF_TEST_CTE_PREFIX: str = "__dbt_ref__"
EXPECTED_TEST_CTE_PREFIX: str = "__expected__"
REF_TEST_CTE_PREFIX: str = "__ref__"
SEED_TEST_CTE_PREFIX: str = "__seed__"
SOURCE_TEST_CTE_PREFIX: str = "__source__"
TABLE_FN_TEST_CTE_PREFIX: str = "__table_fn__"

_STUB_TEST_FILE: DiscoveredSqlTestFile = DiscoveredSqlTestFile(
    file_path=Path("tests/unit/test_chain.sql"),
    relative_path=Path("tests/unit/test_chain.sql"),
    contents="",
    blocks=(),
)

_STUB_TEST_BLOCK: DiscoveredSqlTestBlock = DiscoveredSqlTestBlock(
    test_index=0,
    header_values={},
    sql_body="",
)

_MACRO_CONTEXT: MacroContext = MacroContext(
    adapter_name="duckdb",
    sql_analysis_enabled=True,
    target_name=None,
    vars={},
)


def plan_single_test(
    *,
    test: CompiledSqlTest,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    sql_analysis_enabled: bool = False,
) -> tuple[SqlTestPlanEntry, tuple[PlanWarning, ...]]:
    """Plan one SQL test through the batch planner and return its entry and warnings."""

    result: SqlTestPlanResult
    (result,) = plan_sql_tests(
        tests=(test,),
        project=project,
        adapter=adapter,
        sql_analysis_enabled=sql_analysis_enabled,
    )
    assert result.entry is not None
    return result.entry, result.warnings


def plan_single_test_allowing_errors(
    *,
    test: CompiledSqlTest,
    project: CompiledProjectView,
    adapter: BaseAdapter,
    sql_analysis_enabled: bool = False,
) -> tuple[SqlTestPlanResult, NativeSqlTestPlan]:
    """Plan one SQL test through the batch planner and return its raw native plan too."""

    result: SqlTestPlanResult
    (result,) = plan_sql_tests(
        tests=(test,),
        project=project,
        adapter=adapter,
        sql_analysis_enabled=sql_analysis_enabled,
    )
    plan: NativeSqlTestPlan
    (plan,) = plan_sql_tests_natively(
        project=project,
        tests=(test,),
        adapter=adapter,
        sql_analysis_enabled=sql_analysis_enabled,
        render_sql=False,
    )
    return result, plan


def assert_native_artifact_matches_runtime_plan(
    *,
    project: CompiledProjectView,
    sql_test: CompiledSqlTest,
    entry: SqlTestPlanEntry,
    warnings: tuple[PlanWarning, ...],
    sql_analysis_enabled: bool,
) -> bool:
    """Assert the compiled artifact matches the runtime comparison SQL for the same plan."""

    adapter: DuckDbAdapter = DuckDbAdapter()
    expected_sql: str = render_sql_test_comparison_sql(
        (entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
    )[0]
    (artifact,) = plan_compiled_sql_test_artifacts(
        project=project,
        tests=(sql_test,),
        adapter=adapter,
        sql_analysis_enabled=sql_analysis_enabled,
    )

    dialect: str | None = adapter.sql_analysis_dialect()
    assert dialect is not None
    assert query_fingerprint(artifact.sql, dialect) == query_fingerprint(expected_sql, dialect)
    assert artifact.model_names == tuple(step.model_name for step in entry.chain)
    assert artifact.error_messages == sql_test_plan_error_messages(warnings=warnings)
    return True


def build_test_and_project(
    test_case: PlanTestChainTestCase,
) -> tuple[CompiledSqlTest, CompiledProjectView]:
    """Build a CompiledSqlTest and CompiledProject from a test case."""

    authored_ctes: list[CompileSqlTestCte] = []
    name: str
    body: str
    for name, body in test_case.mock_ref_ctes.items():
        authored_ctes.append(
            CompileSqlTestCte(
                name=f"{REF_TEST_CTE_PREFIX}{name}",
                sql_body=body,
            )
        )
    for name, body in test_case.mock_source_ctes.items():
        authored_ctes.append(
            CompileSqlTestCte(
                name=f"{SOURCE_TEST_CTE_PREFIX}{name}",
                sql_body=body,
            )
        )
    for name, body in test_case.mock_seed_ctes.items():
        authored_ctes.append(
            CompileSqlTestCte(
                name=f"{SEED_TEST_CTE_PREFIX}{name}",
                sql_body=body,
            )
        )
    for name, body in test_case.mock_dbt_ref_ctes.items():
        authored_ctes.append(
            CompileSqlTestCte(
                name=f"{DBT_REF_TEST_CTE_PREFIX}{name}",
                sql_body=body,
            )
        )
    for name, body in test_case.mock_table_function_ctes.items():
        authored_ctes.append(
            CompileSqlTestCte(
                name=f"{TABLE_FN_TEST_CTE_PREFIX}{name}",
                sql_body=body,
            )
        )
    for name, body in test_case.helper_ctes.items():
        authored_ctes.append(CompileSqlTestCte(name=name, sql_body=body))

    generated_sql_body: str = _build_test_sql_body(
        test_case.mock_ref_ctes,
        test_case.mock_source_ctes,
        test_case.mock_seed_ctes,
        test_case.mock_dbt_ref_ctes,
        test_case.mock_table_function_ctes,
        test_case.helper_ctes,
        test_case.expected_cte_bodies,
    )
    sql_body: str = (generated_sql_body, test_case.sql_body)[bool(test_case.sql_body)]

    loaded_macros: dict[str, LoadedMacro] = _build_loaded_macros(test_case.loaded_macro_outputs)
    compiled_test: CompiledSqlTest = CompiledSqlTest(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_TEST,
            name="test_chain",
        ),
        scope_deps=tuple(
            CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=n)
            for n in test_case.expected_model_names
        ),
        name="test_chain",
        test_file=_STUB_TEST_FILE,
        test_block=_STUB_TEST_BLOCK,
        sql_body=sql_body,
        payload=CompiledModelSqlTestPayload(
            authored_ctes=tuple(authored_ctes),
            macro_mocks=test_case.macro_mocks,
            model_query_overrides=_build_model_query_overrides(
                test_case=test_case,
                loaded_macros=loaded_macros,
            ),
            mock_model_names=tuple(test_case.mock_ref_ctes.keys()),
            mock_source_names=tuple(test_case.mock_source_ctes.keys()),
            mock_seed_names=tuple(test_case.mock_seed_ctes.keys()),
            mock_dbt_ref_names=tuple(test_case.mock_dbt_ref_ctes.keys()),
            mock_table_function_names=tuple(test_case.mock_table_function_ctes.keys()),
            expected_model_names=test_case.expected_model_names,
            expected_ctes=tuple(
                CompileSqlTestCte(name=f"{EXPECTED_TEST_CTE_PREFIX}{name}", sql_body=body)
                for name, body in test_case.expected_cte_bodies.items()
            ),
            assertion_ctes=tuple(
                CompileSqlTestCte(
                    name=f"{ASSERT_TEST_CTE_PREFIX}{name}",
                    sql_body=body,
                )
                for name, body in test_case.assertion_ctes.items()
            ),
            assertion_names=tuple(test_case.assertion_ctes),
        ),
    )

    models: list[CompiledModelView] = []
    model_name: str
    query_sql: str
    for model_name, query_sql in test_case.model_queries.items():
        models.append(
            CompiledModel(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.MODEL,
                    name=model_name,
                ),
                deps=tuple(
                    (
                        *(
                            CompiledObjectKey(
                                resource_type=CompiledResourceType.MODEL,
                                name=dependency_name,
                            )
                            for dependency_name in _model_dependencies(query_sql=query_sql)
                        ),
                        *(
                            CompiledObjectKey(
                                resource_type=CompiledResourceType.TABLE_FN,
                                name=dependency_name,
                            )
                            for dependency_name in _table_function_dependencies(query_sql=query_sql)
                        ),
                    )
                ),
                name=model_name,
                relative_path=Path(f"models/{model_name}.sql"),
                query_sql=query_sql,
                config=CompileModelConfig(),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="staging",
                    name=model_name,
                    qualified_name=f"staging.{model_name}",
                ),
            )
        )

    functions: list[CompiledFunctionView] = []
    function_name: str
    qualified_name: str
    for function_name, qualified_name in test_case.function_locations.items():
        functions.append(
            CompiledFunction(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.UDF,
                    name=function_name,
                ),
                deps=(),
                name=function_name,
                relative_path=Path(f"functions/sql/{function_name}.sql"),
                arguments=(),
                returns="BOOLEAN",
                body_sql="SELECT TRUE",
                destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name=function_name,
                    qualified_name=qualified_name,
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name=function_name,
                    qualified_name=qualified_name,
                ),
            )
        )
    for function_name, qualified_name in test_case.table_function_locations.items():
        functions.append(
            CompiledFunction(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.TABLE_FN,
                    name=function_name,
                ),
                deps=(),
                name=function_name,
                relative_path=Path(f"functions/sql/{function_name}.sql"),
                arguments=(),
                returns="TABLE",
                body_sql="SELECT 1 AS order_id",
                destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name=function_name,
                    qualified_name=qualified_name,
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name=function_name,
                    qualified_name=qualified_name,
                ),
            )
        )

    project: CompiledProjectView = CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=tuple(models),
        functions=tuple(functions),
    )

    return compiled_test, project


def _model_dependencies(*, query_sql: str) -> tuple[str, ...]:
    return _reference_names(query_sql=query_sql, kind=SqlReferenceKind.REF)


def _table_function_dependencies(*, query_sql: str) -> tuple[str, ...]:
    return _reference_names(query_sql=query_sql, kind=SqlReferenceKind.TABLE_FUNCTION)


def _reference_names(*, query_sql: str, kind: SqlReferenceKind) -> tuple[str, ...]:
    return reference_names_by_kind(query_sql=query_sql)[kind]


def reference_names_by_kind(*, query_sql: str) -> dict[SqlReferenceKind, tuple[str, ...]]:
    """Return distinct logical reference names in authored order, grouped by reference kind."""

    names: dict[SqlReferenceKind, dict[str, None]] = {kind: {} for kind in SqlReferenceKind}
    for reference in extract_sql_references(sql=query_sql, syntax=SqlLexicalSyntax()):
        names[SqlReferenceKind(reference.ref_kind)][reference.ref_name] = None
    return {kind: tuple(kind_names) for kind, kind_names in names.items()}


def _build_model_query_overrides(
    *,
    test_case: PlanTestChainTestCase,
    loaded_macros: dict[str, LoadedMacro],
) -> dict[str, str]:
    """Build model query overrides for planner tests with macro mocks."""

    overrides: dict[str, str] = dict(test_case.model_query_overrides)
    _MACRO_OVERRIDE_BUILDERS[bool(test_case.macro_mocks)](overrides, test_case, loaded_macros)
    return overrides


def _expand_model_query_overrides(
    overrides: dict[str, str],
    test_case: PlanTestChainTestCase,
    loaded_macros: dict[str, LoadedMacro],
) -> None:
    model_name: str
    query_sql: str
    for model_name, query_sql in test_case.model_macro_source_queries.items():
        overrides[model_name] = expand_sql_macros(
            sql=query_sql,
            file_path=Path(f"models/{model_name}.sql"),
            loaded_macros=loaded_macros,
            macro_overrides=test_case.macro_mocks,
            macro_context=_MACRO_CONTEXT,
        ).sql


def _keep_model_query_overrides(
    overrides: dict[str, str],
    test_case: PlanTestChainTestCase,
    loaded_macros: dict[str, LoadedMacro],
) -> None:
    del overrides, test_case, loaded_macros


_MACRO_OVERRIDE_BUILDERS: MappingProxyType[
    bool,
    Callable[[dict[str, str], PlanTestChainTestCase, dict[str, LoadedMacro]], None],
] = MappingProxyType({False: _keep_model_query_overrides, True: _expand_model_query_overrides})


def _build_loaded_macros(macro_outputs: dict[str, str]) -> dict[str, LoadedMacro]:
    """Build loaded macro stubs for planner tests."""

    loaded_macros: dict[str, LoadedMacro] = {}
    name: str
    output: str
    for name, output in macro_outputs.items():
        loaded_macros[name] = LoadedMacro(
            name=name,
            file_path=Path("macros/python/test_macros.py"),
            relative_path=Path("macros/python/test_macros.py"),
            raw_source="",
            function=_macro_function(output),
        )
    return loaded_macros


def _macro_function(output: str) -> Callable[..., object]:
    """Return a callable macro stub."""

    def _inner(*_args: object, **_kwargs: object) -> str:
        return output

    return _inner


def _build_test_sql_body(
    mock_refs: dict[str, str],
    mock_sources: dict[str, str],
    mock_seeds: dict[str, str],
    mock_dbt_refs: dict[str, str],
    mock_table_functions: dict[str, str],
    helpers: dict[str, str],
    expected_bodies: dict[str, str],
) -> str:
    """Build a synthetic test sql_body containing expected CTEs."""

    parts: list[str] = []
    name: str
    body: str
    for name, body in mock_refs.items():
        parts.append(f"{REF_TEST_CTE_PREFIX}{name} AS ({body})")
    for name, body in mock_sources.items():
        parts.append(f"{SOURCE_TEST_CTE_PREFIX}{name} AS ({body})")
    for name, body in mock_seeds.items():
        parts.append(f"{SEED_TEST_CTE_PREFIX}{name} AS ({body})")
    for name, body in mock_dbt_refs.items():
        parts.append(f"{DBT_REF_TEST_CTE_PREFIX}{name} AS ({body})")
    for name, body in mock_table_functions.items():
        parts.append(f"{TABLE_FN_TEST_CTE_PREFIX}{name} AS ({body})")
    for name, body in helpers.items():
        parts.append(f"{name} AS ({body})")
    for name, body in expected_bodies.items():
        parts.append(f"{EXPECTED_TEST_CTE_PREFIX}{name} AS ({body})")
    return ("WITH " + ", ".join(parts) + " SELECT 1", "SELECT 1")[not parts]


CURSOR_WINDOW_MODEL_SQL: str = (
    "SELECT order_date FROM orders "
    "WHERE order_date >= __cursor_start() AND order_date < __cursor_end()"
)
CURSOR_WINDOW_ADAPTERS: dict[str, BaseAdapter] = {
    "duckdb": DuckDbAdapter(),
    "snowflake": SnowflakeAdapter(),
}


def build_cursor_model(*, cursor_type: str, cursor_grain: str | None) -> CompiledModelView:
    """Build one incremental model whose query uses both cursor intrinsics."""

    values: dict[str, object] = {
        "materialized": "incremental",
        "cursor": "order_date",
        "cursor_type": cursor_type,
        "cursor_grain": cursor_grain,
    }
    return CompiledModel(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="daily_orders"),
        deps=(),
        name="daily_orders",
        relative_path=Path("models/daily_orders.sql"),
        query_sql=CURSOR_WINDOW_MODEL_SQL,
        config=CompileModelConfig(values=values),
        destination=CompiledRelationLocation(
            database=None,
            schema="main",
            name="daily_orders",
            qualified_name="main.daily_orders",
        ),
    )


def build_windowed_sql_test(*, cursor_start: str | None, cursor_end: str | None) -> CompiledSqlTest:
    """Build one model SQL test whose header declares the given cursor window."""

    return CompiledSqlTest(
        key=CompiledObjectKey(resource_type=CompiledResourceType.SQL_TEST, name="orders_case"),
        scope_deps=(
            CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="daily_orders"),
        ),
        name="orders_case",
        test_file=DiscoveredSqlTestFile(
            file_path=Path("tests/unit/orders_case.sql"),
            relative_path=Path("tests/unit/orders_case.sql"),
            contents="",
            blocks=(),
        ),
        test_block=DiscoveredSqlTestBlock(
            test_index=1,
            header_values={},
            sql_body="",
            cursor_start=cursor_start,
            cursor_end=cursor_end,
        ),
        sql_body=(
            f"WITH {EXPECTED_TEST_CTE_PREFIX}daily_orders AS "
            "(SELECT TIMESTAMP '2026-02-01' AS order_date) SELECT 1"
        ),
        payload=CompiledModelSqlTestPayload(
            expected_model_names=("daily_orders",),
            expected_ctes=(
                CompileSqlTestCte(
                    name=f"{EXPECTED_TEST_CTE_PREFIX}daily_orders",
                    sql_body="SELECT TIMESTAMP '2026-02-01' AS order_date",
                ),
            ),
        ),
    )


def plan_cursor_window_model_sql(
    *, model: CompiledModelView, test: CompiledSqlTest, adapter: BaseAdapter
) -> str:
    """Plan one windowed SQL test and return the rendered SQL of its cursor model step."""

    project: CompiledProjectView = CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=(model,),
    )
    entry, _ = plan_single_test(test=test, project=project, adapter=adapter)
    (step,) = entry.chain
    return step.resolved_sql
