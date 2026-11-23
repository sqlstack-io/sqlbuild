from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompileInputError,
    CompilePipelineResult,
    DiscoveredProjectInputs,
    build_compiled_project,
    discover_project_inputs,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    GroupedDeclarationCompileTestCase,
    MacroDeclarationContextErrorTestCase,
    MacroDeclarationRenderingTestCase,
    MacroDeclarationResourceTestCase,
    MacroTestDiscoveryIntegrationTestCase,
    SqlTestProductionScopeIntegrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    run_compile_pipeline_for_project,
)


@pytest.mark.parametrize(
    "test_case",
    (
        MacroDeclarationRenderingTestCase(
            description="DuckDB array rendering",
            adapter_name="duckdb",
            adapter_factory=DuckDbAdapter,
            expected_sql="SELECT ['north', 'south'] AS regions",
        ),
        MacroDeclarationRenderingTestCase(
            description="Snowflake array rendering",
            adapter_name="snowflake",
            adapter_factory=SnowflakeAdapter,
            expected_sql="SELECT ARRAY_CONSTRUCT('north', 'south') AS regions",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_active_adapter_when_macro_renders_constant_then_uses_adapter_sql(
    test_case: MacroDeclarationRenderingTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": (
                f'name = "demo"\nadapter = "{test_case.adapter_name}"\n\n'
                "[settings]\nsql_analysis = false\n"
            ),
            "sql/models/_constants/policy.sql": (
                'CONSTANT (name regions, value ["north", "south"], render_as array);\n'
            ),
            "sql/models/_macros/python/policy.py": (
                "def region_array(ctx) -> str:\n    return ctx.render_constant('regions')\n"
            ),
            "sql/models/summary.sql": (
                "MODEL (description 'Test model summary.', materialized view, database warehouse, schema analytics);\n\n"
                "SELECT @py.region_array() AS regions"
            ),
        },
    )
    adapter: BaseAdapter = test_case.adapter_factory()

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered_inputs,
        adapter=adapter,
    )

    assert project.models[0].query_sql == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    (
        GroupedDeclarationCompileTestCase(
            description="grouped private macro reads grouped private constant and enum",
            project_files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                "sql/models/orders/_sqlbuild/_constants/policy.sql": (
                    "CONSTANT (name minimum_quantity, value 2);\n"
                ),
                "sql/models/orders/_sqlbuild/_enums/status.sql": (
                    "ENUM (name order_status, members [ACTIVE]);\n"
                ),
                "sql/models/orders/_sqlbuild/_macros/python/policy.py": (
                    "def order_policy(ctx) -> str:\n"
                    "    quantity = ctx.render_constant('minimum_quantity')\n"
                    "    status = ctx.render_enum_member(\n"
                    "        enum_name='order_status', member_name='ACTIVE'\n"
                    "    )\n"
                    '    return f"{quantity} AS minimum_quantity, {status} AS status"\n'
                ),
                "sql/models/orders/summary.sql": (
                    'MODEL (description "Order summary.");\nSELECT @py.order_policy()'
                ),
            },
            expected_sql="SELECT 2 AS minimum_quantity, 'ACTIVE' AS status",
        ),
        GroupedDeclarationCompileTestCase(
            description="grouped public macro is visible to descendant model",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n[scopes]\nenforce_placement = false\n'
                ),
                "sql/models/orders/_sqlbuild/macros/python/policy.py": (
                    "def minimum_quantity() -> str:\n    return '2'\n"
                ),
                "sql/models/orders/history/summary.sql": (
                    'MODEL (description "Historical order summary.");\n'
                    "SELECT @py.minimum_quantity() AS minimum_quantity"
                ),
            },
            expected_sql="SELECT 2 AS minimum_quantity",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_grouped_declarations_when_compiling_then_scope_resolves_from_owner(
    test_case: GroupedDeclarationCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path, adapter=DuckDbAdapter()
    )

    assert result.project.models[0].query_sql == test_case.expected_sql


@pytest.mark.parametrize(
    "test_case",
    (
        MacroTestDiscoveryIntegrationTestCase(
            description="project macro test is discovered from mirrored macro path",
            project_files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                "sql/macros/python/orders/policy.py": "def order_policy() -> str:\n    return '2'\n",
                "sql/models/orders/summary.sql": (
                    'MODEL (description "Order summary.");\nSELECT @py.order_policy() AS quantity'
                ),
                "tests/unit/macros/orders/test_order_policy__returns_quantity.sql": (
                    'TEST (mode macro, name "order_policy__returns_quantity");\n\n'
                    "WITH\n"
                    "__macro_actual__ AS (SELECT @py.order_policy() AS quantity),\n"
                    "__macro_expected__ AS (SELECT 2 AS quantity)\n"
                    "SELECT 1\n"
                ),
            },
            expected_test_name="order_policy__returns_quantity",
            expected_macro_name="order_policy",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_macro_test_in_mirrored_path_when_compiling_then_test_is_attached(
    test_case: MacroTestDiscoveryIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path, adapter=DuckDbAdapter()
    )

    assert tuple(test.name for test in result.project.sql_tests) == (test_case.expected_test_name,)
    tested_macro_names: list[str] = []
    for test in result.project.sql_tests:
        tested_macro_names.extend(resource.name for resource in test.tested_resources)
    assert tuple(tested_macro_names) == (test_case.expected_macro_name,)


@pytest.mark.parametrize(
    "test_case",
    (
        SqlTestProductionScopeIntegrationTestCase(
            description="macro test inherits tested scoped macro declaration context",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n[scopes]\nenforce_placement = true\n'
                ),
                "sql/models/orders/_sqlbuild/_constants/policy.sql": (
                    "CONSTANT (name minimum_quantity, value 2);\n"
                ),
                "sql/models/orders/_sqlbuild/_macros/python/policy.py": (
                    "def order_policy(ctx) -> str:\n"
                    "    return ctx.render_constant('minimum_quantity')\n"
                ),
                "sql/models/orders/summary.sql": (
                    'MODEL (description "Order summary.");\nSELECT @py.order_policy() AS quantity'
                ),
                "tests/unit/macros/models/orders/test_order_policy__returns_quantity.sql": (
                    'TEST (mode macro, name "order_policy__returns_quantity");\n\n'
                    "WITH\n"
                    "__macro_actual__ AS (SELECT @py.order_policy() AS quantity),\n"
                    "__macro_expected__ AS (SELECT 2 AS quantity)\n"
                    "SELECT 1\n"
                ),
            },
            expected_test_name="order_policy__returns_quantity",
            expected_sql_fragments=("SELECT 2 AS quantity",),
        ),
        SqlTestProductionScopeIntegrationTestCase(
            description="model test combines target production scope and lexical test helpers",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n[scopes]\nenforce_placement = false\n'
                ),
                "sql/models/orders/_sqlbuild/_macros/python/policy.py": (
                    "def order_policy() -> str:\n    return '2'\n"
                ),
                "sql/models/orders/order_summary.sql": (
                    'MODEL (description "Order summary.");\n'
                    'SELECT @py.order_policy() AS quantity FROM __ref("order_input")'
                ),
                "sql/models/order_input.sql": (
                    'MODEL (description "Order input.");\nSELECT 1 AS order_id'
                ),
                "tests/unit/orders/_sqlbuild/macros/python/fixtures.py": (
                    "def fixture_quantity() -> str:\n    return '2'\n"
                ),
                "tests/unit/orders/test_order_summary__uses_policy.sql": (
                    'TEST (name "order_summary__uses_policy");\n\n'
                    "WITH\n"
                    "__ref__order_input AS (SELECT 1 AS order_id),\n"
                    "__expected__order_summary AS (\n"
                    "  SELECT @py.order_policy() + @py.fixture_quantity() AS quantity\n"
                    ")\n"
                    "SELECT 1\n"
                ),
            },
            expected_test_name="order_summary__uses_policy",
            expected_sql_fragments=("SELECT 2 + 2 AS quantity",),
        ),
        SqlTestProductionScopeIntegrationTestCase(
            description="multi-model test receives union of target production scopes",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n[scopes]\nenforce_placement = false\n'
                ),
                "sql/models/orders/_sqlbuild/_macros/python/policy.py": (
                    "def order_policy() -> str:\n    return '2'\n"
                ),
                "sql/models/orders/order_summary.sql": (
                    'MODEL (description "Order summary.");\n'
                    'SELECT @py.order_policy() AS quantity FROM __ref("order_input")'
                ),
                "sql/models/order_input.sql": (
                    'MODEL (description "Order input.");\nSELECT 1 AS order_id'
                ),
                "sql/models/products/_sqlbuild/_macros/python/policy.py": (
                    "def product_policy() -> str:\n    return '3'\n"
                ),
                "sql/models/products/product_summary.sql": (
                    'MODEL (description "Product summary.");\n'
                    'SELECT @py.product_policy() AS quantity FROM __ref("order_input")'
                ),
                "tests/unit/test_summaries__use_policies.sql": (
                    'TEST (name "summaries__use_policies");\n\n'
                    "WITH\n"
                    "__ref__order_input AS (SELECT 1 AS order_id),\n"
                    "__expected__order_summary AS (SELECT @py.order_policy() AS quantity),\n"
                    "__expected__product_summary AS (SELECT @py.product_policy() AS quantity)\n"
                    "SELECT 1\n"
                ),
            },
            expected_test_name="summaries__use_policies",
            expected_sql_fragments=("SELECT 2 AS quantity", "SELECT 3 AS quantity"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_inferred_targets_when_compiling_test_then_production_scopes_are_combined(
    test_case: SqlTestProductionScopeIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path, adapter=DuckDbAdapter()
    )

    assert tuple(test.name for test in result.project.sql_tests) == (test_case.expected_test_name,)
    for expected_fragment in test_case.expected_sql_fragments:
        assert expected_fragment in result.project.sql_tests[0].sql_body


@pytest.mark.parametrize(
    "test_case",
    (
        MacroDeclarationResourceTestCase(
            description="models tests and audits receive caller-visible constants",
            expected_model_sql_fragment="SELECT 2 AS minimum_quantity",
            expected_test_sql_fragment="SELECT 2 AS minimum_quantity",
            expected_audit_sql_fragment="minimum_quantity < 2",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_macro_context_declarations_when_compiling_resources_then_all_expand(
    test_case: MacroDeclarationResourceTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": (
                'name = "demo"\nadapter = "duckdb"\n\n[settings]\nsql_analysis = false\n'
            ),
            "sql/constants/policy.sql": "CONSTANT (name minimum_quantity, value 2);\n",
            "sql/macros/python/policy.py": (
                "def minimum_quantity(ctx) -> str:\n"
                "    return str(ctx.constants['minimum_quantity'])\n"
            ),
            "sql/models/summary.sql": (
                "MODEL (description 'Test model summary.', materialized view, audits [minimum_quantity]);\n\n"
                "SELECT @py.minimum_quantity() AS minimum_quantity"
            ),
            "tests/unit/summary_test.sql": (
                "TEST (mode macro);\n\nWITH\n__macro_actual__ AS (\n"
                "  SELECT @py.minimum_quantity() AS minimum_quantity\n),\n"
                "__macro_expected__ AS (\n  SELECT 2 AS minimum_quantity\n)\nSELECT 1"
            ),
            "sql/audits/generic/minimum_quantity.sql": (
                'AUDIT ();\n\nSELECT * FROM __ref("@model") '
                "WHERE minimum_quantity < @py.minimum_quantity()"
            ),
        },
    )

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path, adapter=DuckDbAdapter()
    )

    assert test_case.expected_model_sql_fragment in result.project.models[0].query_sql
    assert test_case.expected_test_sql_fragment in result.project.sql_tests[0].sql_body
    assert test_case.expected_audit_sql_fragment in result.project.audits[0].sql_body


@pytest.mark.parametrize(
    "test_case",
    (
        MacroDeclarationContextErrorTestCase(
            description="sibling private constant remains inaccessible",
            project_files={
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/models/orders/_constants/policy.sql": (
                    "CONSTANT (name minimum_quantity, value 2);\n"
                ),
                "sql/macros/python/policy.py": (
                    "def minimum_quantity(ctx) -> str:\n"
                    "    return str(ctx.constants['minimum_quantity'])\n"
                ),
                "sql/models/products/summary.sql": (
                    "MODEL (description 'Test model summary.', materialized view);\n\nSELECT @py.minimum_quantity() AS minimum_quantity"
                ),
            },
            expected_error_fragment="Constant 'minimum_quantity'.*is inaccessible",
        ),
        MacroDeclarationContextErrorTestCase(
            description="grouped private macro remains inaccessible to descendant model",
            project_files={
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/models/orders/_sqlbuild/_macros/python/policy.py": (
                    "def minimum_quantity() -> str:\n    return '2'\n"
                ),
                "sql/models/orders/history/summary.sql": (
                    'MODEL (description "Historical order summary.");\n'
                    "SELECT @py.minimum_quantity() AS minimum_quantity"
                ),
            },
            expected_error_fragment="Macro '@py.minimum_quantity'.*is inaccessible",
        ),
        MacroDeclarationContextErrorTestCase(
            description="mocked dependency does not grant its production macro scope",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n[scopes]\nenforce_placement = false\n'
                ),
                "sql/models/products/_sqlbuild/_macros/python/policy.py": (
                    "def product_policy() -> str:\n    return '3'\n"
                ),
                "sql/models/products/product_input.sql": (
                    'MODEL (description "Product input.");\nSELECT 1 AS product_id'
                ),
                "sql/models/orders/order_summary.sql": (
                    'MODEL (description "Order summary.");\n'
                    'SELECT product_id FROM __ref("product_input")'
                ),
                "tests/unit/orders/test_order_summary__uses_input.sql": (
                    'TEST (name "order_summary__uses_input");\n\n'
                    "WITH\n"
                    "__ref__product_input AS (SELECT 1 AS product_id),\n"
                    "__expected__order_summary AS (\n"
                    "  SELECT @py.product_policy() AS product_id\n"
                    ")\n"
                    "SELECT 1\n"
                ),
            },
            expected_error_fragment="Macro '@py.product_policy'.*is inaccessible",
        ),
        MacroDeclarationContextErrorTestCase(
            description="unknown constant reports visible alternatives",
            project_files={
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/models/_constants/policy.sql": (
                    "CONSTANT (name minimum_quantity, value 2);\n"
                ),
                "sql/models/_macros/python/policy.py": (
                    "def missing_quantity(ctx) -> str:\n"
                    "    return ctx.render_constant('maximum_quantity')\n"
                ),
                "sql/models/summary.sql": (
                    "MODEL (description 'Test model summary.', materialized view);\n\nSELECT @py.missing_quantity() AS maximum_quantity"
                ),
            },
            expected_error_fragment=(
                "Unknown constant 'maximum_quantity'.*Visible constants: minimum_quantity"
            ),
        ),
        MacroDeclarationContextErrorTestCase(
            description="unknown enum member reports available alternatives",
            project_files={
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/models/_enums/status.sql": (
                    "ENUM (name order_status, members [ACTIVE, PAUSED]);\n"
                ),
                "sql/models/_macros/python/policy.py": (
                    "def missing_status(ctx) -> str:\n"
                    "    return ctx.render_enum_member(\n"
                    "        enum_name='order_status', member_name='CLOSED'\n"
                    "    )\n"
                ),
                "sql/models/summary.sql": (
                    "MODEL (description 'Test model summary.', materialized view);\n\nSELECT @py.missing_status() AS status"
                ),
            },
            expected_error_fragment=(
                "Unknown member 'CLOSED' for enum 'order_status'.*Available members: ACTIVE, PAUSED"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unavailable_declaration_when_macro_reads_context_then_compile_fails(
    test_case: MacroDeclarationContextErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    with pytest.raises(CompileInputError, match=test_case.expected_error_fragment):
        run_compile_pipeline_for_project(project_dir=tmp_path, adapter=DuckDbAdapter())
