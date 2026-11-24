"""Test helpers for sql_test_assembly integration tests."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import (
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
    SqlReferenceKind,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)
from tests.integration.src.sqlbuild.compiler.planner._helpers.sql_test_assembly._test_types import (
    ExecuteChainTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly.helpers import (
    reference_names_by_kind,
)

EXPECTED_TEST_CTE_PREFIX: str = "__expected__"
REF_TEST_CTE_PREFIX: str = "__ref__"
SOURCE_TEST_CTE_PREFIX: str = "__source__"

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


def build_test_and_project(
    test_case: ExecuteChainTestCase,
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
    for name, body in test_case.helper_ctes.items():
        authored_ctes.append(CompileSqlTestCte(name=name, sql_body=body))

    sql_body: str = _build_test_sql_body(
        test_case.mock_ref_ctes,
        test_case.mock_source_ctes,
        test_case.helper_ctes,
        test_case.expected_cte_bodies,
    )

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
            mock_model_names=tuple(test_case.mock_ref_ctes.keys()),
            mock_source_names=tuple(test_case.mock_source_ctes.keys()),
            expected_model_names=test_case.expected_model_names,
            expected_ctes=tuple(
                CompileSqlTestCte(name=f"__expected__{name}", sql_body=body)
                for name, body in test_case.expected_cte_bodies.items()
            ),
        ),
    )

    models: list[CompiledModelView] = []
    model_name: str
    query_sql: str
    for model_name, query_sql in test_case.model_queries.items():
        dependency_names: tuple[str, ...] = reference_names_by_kind(query_sql=query_sql)[
            SqlReferenceKind.REF
        ]
        dependencies: tuple[CompiledObjectKey, ...] = tuple(
            CompiledObjectKey(
                resource_type=CompiledResourceType.MODEL,
                name=dependency_name,
            )
            for dependency_name in dict.fromkeys(dependency_names)
        )
        models.append(
            CompiledModel(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.MODEL,
                    name=model_name,
                ),
                deps=dependencies,
                name=model_name,
                relative_path=Path(f"sql/models/{model_name}.sql"),
                query_sql=query_sql,
                config=CompileModelConfig(),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name=model_name,
                    qualified_name=f"main.{model_name}",
                ),
            )
        )

    project: CompiledProjectView = CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=tuple(models),
    )

    return compiled_test, project


def _build_test_sql_body(
    mock_refs: dict[str, str],
    mock_sources: dict[str, str],
    helpers: dict[str, str],
    expected_bodies: dict[str, str],
) -> str:
    """Build a synthetic test sql_body with expected CTEs."""

    parts: list[str] = []
    name: str
    body: str
    for name, body in mock_refs.items():
        parts.append(f"{REF_TEST_CTE_PREFIX}{name} AS ({body})")
    for name, body in mock_sources.items():
        parts.append(f"{SOURCE_TEST_CTE_PREFIX}{name} AS ({body})")
    for name, body in helpers.items():
        parts.append(f"{name} AS ({body})")
    for name, body in expected_bodies.items():
        parts.append(f"{EXPECTED_TEST_CTE_PREFIX}{name} AS ({body})")
    return "WITH " + ", ".join(parts) + " SELECT 1"
