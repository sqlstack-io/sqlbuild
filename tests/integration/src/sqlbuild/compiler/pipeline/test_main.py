from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import cast
from unittest.mock import Mock

import pytest

from sqlbuild._native import (
    CompilePipelineResult,
    DiscoveredProjectInputs,
    InferredColumn,
    InferredNullability,
    ModelPlanEntry,
    RulesError,
    build_compiled_project,
    build_project_graph,
    discover_project_inputs,
    write_compile_target,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    AppendCursorPipelineIntegrationTestCase,
    CompileProgressIntegrationTestCase,
    CteTypePropagationIntegrationTestCase,
    DeferToIntegrationTestCase,
    ExpectedModelEntry,
    RulesPipelineIntegrationTestCase,
    RunCompilePipelineIntegrationTestCase,
    SnowflakeTargetValidationIntegrationTestCase,
    SqlAnalysisChainCompileTargetIntegrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    apply_warehouse_setup_sql,
    build_manifest_for_pipeline_result,
    run_compile_pipeline_for_project,
    validate_manifest_against_dbt_schema,
)

_PROJECT_TOML: str = 'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
_FILE_PROJECT_TOML: str = (
    'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        CteTypePropagationIntegrationTestCase(
            description="typed cte passthrough satisfies the model contract",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "sources/raw.yml": (
                    "sources:\n"
                    "  - name: raw_orders\n    description: Test source raw_orders.\n"
                    "    expression: SELECT '12' AS amount\n"
                    "    columns:\n"
                    "      - name: amount\n"
                    "        type: VARCHAR\n"
                    "        nullable: false\n"
                ),
                "schemas/orders.sql": (
                    "SCHEMA (\n"
                    "  name order_amount,\n"
                    "  columns (amount (type INTEGER, nullable false)),\n"
                    ");\n"
                ),
                "models/orders.sql": (
                    "MODEL (\n"
                    '  description "Test model orders.",\n'
                    "  contract enforced,\n"
                    "  model_schema order_amount,\n"
                    ");\n\n"
                    "WITH transformed AS (\n"
                    "  SELECT CAST(amount AS INTEGER) AS amount\n"
                    '  FROM __source("raw_orders")\n'
                    "),\n"
                    "final AS (\n"
                    "  SELECT amount\n"
                    "  FROM transformed\n"
                    ")\n"
                    "SELECT amount\n"
                    "FROM final\n"
                ),
            },
            expected_column_name="amount",
            expected_column_type="INTEGER",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_cte_chain_when_compiling_contract_then_type_remains_proven(
    test_case: CteTypePropagationIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered_inputs,
        adapter=DuckDbAdapter(),
    )

    assert project.diagnostics == ()
    assert project.models[0].inferred_columns == (
        InferredColumn(
            name=test_case.expected_column_name,
            type=test_case.expected_column_type,
            nullability=InferredNullability.NON_NULL,
        ),
    )


@pytest.mark.parametrize(
    "test_case",
    [
        RulesPipelineIntegrationTestCase(
            description="configured Rules block planning before a warehouse connection",
            expected_error_pattern="SQBRSQL004",
            expected_connection_calls=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_rule_finding_when_running_shared_compile_pipeline_then_planning_is_blocked(
    test_case: RulesPipelineIntegrationTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'{_PROJECT_TOML}\n[rules]\nselect = ["SQBRSQL004"]\n', encoding="utf-8"
    )
    model: Path = tmp_path / "models" / "orders.sql"
    model.parent.mkdir()
    model.write_text(
        'MODEL (description "Orders");\nSELECT order_id FROM orders LIMIT 1\n',
        encoding="utf-8",
    )
    adapter: DuckDbAdapter = DuckDbAdapter()
    connect: Mock = Mock()
    monkeypatch.setattr(adapter, "connect", connect)

    with pytest.raises(RulesError, match=test_case.expected_error_pattern):
        _ = run_compile_pipeline_for_project(project_dir=tmp_path, adapter=adapter)

    assert connect.call_count == test_case.expected_connection_calls


@pytest.mark.parametrize(
    "test_case",
    [
        RunCompilePipelineIntegrationTestCase(
            description="single table model with no schema defaults to adapter schema",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id"
                ),
            },
            expected_models={
                "orders": ExpectedModelEntry(
                    description="table model with adapter default schema",
                    expected_resolved_sql_fragment="SELECT 1 AS order_id",
                    expected_logical_ddl_fragment="CREATE OR REPLACE TABLE",
                    expected_manifest_compiled_code_fragment="SELECT 1 AS order_id",
                ),
            },
            expected_model_count=1,
            expected_seed_count=0,
            expected_manifest_node_count=1,
        ),
        RunCompilePipelineIntegrationTestCase(
            description="view materialization produces create or replace view DDL",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/active_orders.sql": (
                    "MODEL (description 'Test model active_orders.', materialized view);\n\nSELECT order_id FROM orders WHERE status = 'active'"
                ),
            },
            expected_models={
                "active_orders": ExpectedModelEntry(
                    description="view model with correct DDL",
                    expected_resolved_sql_fragment="SELECT order_id FROM orders",
                    expected_logical_ddl_fragment="CREATE OR REPLACE VIEW",
                    expected_manifest_compiled_code_fragment="SELECT order_id FROM orders",
                ),
            },
            expected_model_count=1,
            expected_seed_count=0,
            expected_manifest_node_count=1,
        ),
        RunCompilePipelineIntegrationTestCase(
            description="model with explicit schema and database populates manifest target",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.',\n  materialized table\n  schema analytics\n"
                    "  database warehouse\n);\n\n"
                    "SELECT 1 AS order_id"
                ),
            },
            expected_models={
                "orders": ExpectedModelEntry(
                    description="table model with explicit schema and database",
                    expected_resolved_sql_fragment="SELECT 1 AS order_id",
                    expected_logical_ddl_fragment="CREATE OR REPLACE TABLE",
                    expected_manifest_compiled_code_fragment="SELECT 1 AS order_id",
                ),
            },
            expected_model_count=1,
            expected_seed_count=0,
            expected_manifest_node_count=1,
        ),
        RunCompilePipelineIntegrationTestCase(
            description="two models with ref dependency resolves ref to qualified name",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 1 AS order_id"
                ),
                "models/fact_orders.sql": (
                    'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT order_id FROM __ref("stg_orders")'
                ),
            },
            expected_models={
                "stg_orders": ExpectedModelEntry(
                    description="upstream staging model",
                    expected_resolved_sql_fragment="SELECT 1 AS order_id",
                    expected_logical_ddl_fragment="CREATE OR REPLACE TABLE",
                    expected_manifest_compiled_code_fragment="SELECT 1 AS order_id",
                ),
                "fact_orders": ExpectedModelEntry(
                    description="downstream model with resolved ref",
                    expected_resolved_sql_fragment="main.stg_orders",
                    expected_logical_ddl_fragment="CREATE OR REPLACE TABLE",
                    expected_manifest_compiled_code_fragment="main.stg_orders",
                ),
            },
            expected_model_count=2,
            expected_seed_count=0,
            expected_manifest_node_count=2,
        ),
        RunCompilePipelineIntegrationTestCase(
            description="model with source reference resolves source to qualified name",
            warehouse_setup_sql=("CREATE TABLE main.payments AS SELECT 1 AS payment_id",),
            project_files={
                "sqlbuild_project.toml": _FILE_PROJECT_TOML,
                "sources/raw.yml": (
                    "sources:\n  - name: raw_payments\n    description: Test source raw_payments.\n    schema: main\n    table: payments\n"
                ),
                "models/stg_payments.sql": (
                    'MODEL (description "Test model stg_payments.", materialized table);\n\nSELECT payment_id FROM __source("raw_payments")'
                ),
            },
            expected_models={
                "stg_payments": ExpectedModelEntry(
                    description="model with resolved source reference",
                    expected_resolved_sql_fragment="main.payments",
                    expected_logical_ddl_fragment="CREATE OR REPLACE TABLE",
                    expected_manifest_compiled_code_fragment="main.payments",
                ),
            },
            expected_model_count=1,
            expected_seed_count=0,
            expected_manifest_node_count=1,
        ),
        RunCompilePipelineIntegrationTestCase(
            description="multiple models in subdirectories preserves relative paths",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/staging/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized view);\n\nSELECT 1 AS order_id"
                ),
                "models/marts/fact_orders.sql": (
                    'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT order_id FROM __ref("stg_orders")'
                ),
            },
            expected_models={
                "stg_orders": ExpectedModelEntry(
                    description="staging view in subdirectory",
                    expected_resolved_sql_fragment="SELECT 1 AS order_id",
                    expected_logical_ddl_fragment="CREATE OR REPLACE VIEW",
                    expected_manifest_compiled_code_fragment="SELECT 1 AS order_id",
                ),
                "fact_orders": ExpectedModelEntry(
                    description="mart table with ref to staging view",
                    expected_resolved_sql_fragment="main.stg_orders",
                    expected_logical_ddl_fragment="CREATE OR REPLACE TABLE",
                    expected_manifest_compiled_code_fragment="main.stg_orders",
                ),
            },
            expected_model_count=2,
            expected_seed_count=0,
            expected_manifest_node_count=2,
        ),
        RunCompilePipelineIntegrationTestCase(
            description="Python macro receives constants and enums visible to its caller",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/_constants/policy.sql": (
                    "CONSTANT (name minimum_quantity, value 2);\n"
                    'CONSTANT (name regions, value ["north", "south"]);\n'
                    "CONSTANT (name unique_ids, value {2, 1});\n"
                    'CONSTANT (name labels, value (north "North", south "South"));\n'
                    'CONSTANT (name adjustment, type decimal, value "2.50");\n'
                    'CONSTANT (name display_name, value "O\'Reilly");\n'
                ),
                "models/_enums/status.sql": (
                    'ENUM (name order_status, members (ACTIVE "active", PAUSED "paused"));\n'
                ),
                "models/_macros/python/policy.py": (
                    "from sqlbuild.compiler.compile.models import MacroContext\n\n\n"
                    "def policy_columns(ctx: MacroContext) -> str:\n"
                    "    minimum = ctx.constants['minimum_quantity']\n"
                    "    region_count = len(ctx.constants['regions'])\n"
                    "    unique_count = len(ctx.constants['unique_ids'])\n"
                    "    label = ctx.constants['labels']['north']\n"
                    "    adjustment_sql = ctx.render_constant('adjustment')\n"
                    "    display_name_sql = ctx.render_constant('display_name')\n"
                    "    regions_sql = ctx.render_constant('regions')\n"
                    "    active_sql = ctx.render_enum_member(\n"
                    "        enum_name='order_status', member_name='ACTIVE'\n"
                    "    )\n"
                    "    optional = ctx.constants.get('maximum_quantity', 7)\n"
                    "    has_closed = 'CLOSED' in ctx.enums['order_status']\n"
                    "    return (\n"
                    '        f"{minimum} AS minimum_quantity, {region_count} AS region_count, "\n'
                    "        f\"{unique_count} AS unique_count, '{label}' AS label, \"\n"
                    '        f"{adjustment_sql} AS adjustment, {display_name_sql} AS display_name, "\n'
                    "        f\"{active_sql} AS active_status, 'north' IN {regions_sql} \"\n"
                    '        f"AS region_supported, "\n'
                    '        f"{optional} AS optional_quantity, {has_closed} AS has_closed"\n'
                    "    )\n"
                ),
                "models/policy_summary.sql": (
                    "MODEL (description 'Test model policy_summary.', materialized view);\n\nSELECT @py.policy_columns()"
                ),
            },
            expected_models={
                "policy_summary": ExpectedModelEntry(
                    description="model expanded with caller-visible declarations",
                    expected_resolved_sql_fragment=(
                        "SELECT 2 AS minimum_quantity, 2 AS region_count, 2 AS unique_count, "
                        "'North' AS label, 2.50 AS adjustment, 'O''Reilly' AS display_name, "
                        "'active' AS active_status, 'north' IN ('north', 'south') "
                        "AS region_supported, "
                        "7 AS optional_quantity, False AS has_closed"
                    ),
                    expected_logical_ddl_fragment="CREATE OR REPLACE VIEW",
                    expected_manifest_compiled_code_fragment=(
                        "SELECT 2 AS minimum_quantity, 2 AS region_count, 2 AS unique_count, "
                        "'North' AS label, 2.50 AS adjustment, 'O''Reilly' AS display_name, "
                        "'active' AS active_status, 'north' IN ('north', 'south') "
                        "AS region_supported, "
                        "7 AS optional_quantity, False AS has_closed"
                    ),
                ),
            },
            expected_model_count=1,
            expected_seed_count=0,
            expected_manifest_node_count=1,
            expected_declaration_usages=(
                "constant:adjustment",
                "constant:display_name",
                "constant:labels",
                "constant:minimum_quantity",
                "constant:regions",
                "constant:unique_ids",
                "enum:order_status",
                "macro:policy_columns",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_files_when_running_compile_pipeline_then_produces_valid_output(
    test_case: RunCompilePipelineIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    monkeypatch.chdir(tmp_path)
    apply_warehouse_setup_sql(project_dir=tmp_path, statements=test_case.warehouse_setup_sql)

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
    )

    assert len(result.plan_output.model_entries) == test_case.expected_model_count
    assert len(result.plan_output.seed_entries) == test_case.expected_seed_count
    actual_declaration_usages: tuple[str, ...] = tuple(
        sorted(
            f"{usage.declaration.kind.value}:{usage.declaration.name}"
            for usage in result.project.scope_index.usages
        )
    )
    assert actual_declaration_usages == test_case.expected_declaration_usages
    manifest: dict[str, object] = build_manifest_for_pipeline_result(
        result=result,
        project_name="demo",
        adapter_type="duckdb",
    )
    manifest_nodes: dict[str, object] = cast(dict[str, object], manifest["nodes"])
    assert len(manifest_nodes) == test_case.expected_manifest_node_count

    entry_map: dict[str, ModelPlanEntry] = {e.name: e for e in result.plan_output.model_entries}
    model_name: str
    expected: ExpectedModelEntry
    for model_name, expected in test_case.expected_models.items():
        entry: ModelPlanEntry = entry_map[model_name]
        assert expected.expected_resolved_sql_fragment in entry.resolved_sql
        assert expected.expected_logical_ddl_fragment in entry.logical_ddl
        node_id: str = f"model.demo.{model_name}"
        model_node: dict[str, object] = cast(dict[str, object], manifest_nodes[node_id])
        compiled_code: str = cast(str, model_node["compiled_code"])
        assert expected.expected_manifest_compiled_code_fragment in compiled_code

    validate_manifest_against_dbt_schema(manifest)


@pytest.mark.parametrize(
    "test_case",
    [
        RunCompilePipelineIntegrationTestCase(
            description="run selector resolves Python task without SQL-only planning",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id",
                "python/tasks/orders.py": (
                    "from sqlbuild.tasks import task\n\n"
                    "@task\n"
                    "def prepare_orders(ctx):\n"
                    "    '''Test task prepare_orders.'''\n    return ctx.result(payload={'ok': True})\n"
                ),
            },
            expected_models={},
            expected_model_count=0,
            expected_seed_count=0,
            expected_manifest_node_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_run_selector_when_running_compile_pipeline_then_tracks_python_nodes(
    test_case: RunCompilePipelineIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
        select=("task:prepare_orders",),
        resolve_python_run_selectors=True,
    )

    assert len(result.plan_output.model_entries) == test_case.expected_model_count
    assert len(result.plan_output.seed_entries) == test_case.expected_seed_count
    assert result.python_node_names == frozenset({"prepare_orders"})


@pytest.mark.parametrize(
    "test_case",
    [
        CompileProgressIntegrationTestCase(
            description="reports compile progress from compile pipeline",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id",
            },
            expected_progress_prefixes=("Compiling project...", "Compiled project."),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_progress_callback_when_running_compile_pipeline_then_reports_compile_progress(
    test_case: CompileProgressIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    progress_messages: list[str] = []
    write_repo_files(tmp_path, test_case.project_files)

    run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
        on_progress=progress_messages.append,
    )

    expected_prefix: str
    for expected_prefix in test_case.expected_progress_prefixes:
        assert any(message.startswith(expected_prefix) for message in progress_messages)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileProgressIntegrationTestCase(
            description="reports compile progress from project graph build",
            project_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id",
            },
            expected_progress_prefixes=("Compiling project...", "Compiled project."),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_progress_callback_when_building_project_graph_then_reports_compile_progress(
    test_case: CompileProgressIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    progress_messages: list[str] = []
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    build_project_graph(
        discovered_inputs=discovered_inputs,
        adapter=DuckDbAdapter(),
        no_sql_validation=True,
        on_progress=progress_messages.append,
    )

    expected_prefix: str
    for expected_prefix in test_case.expected_progress_prefixes:
        assert any(message.startswith(expected_prefix) for message in progress_messages)


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeTargetValidationIntegrationTestCase(
            description="snowflake compile requires explicit target database and schema",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
                ),
                "sqlbuild_local.toml": (
                    'adapter = "snowflake"\n\n'
                    "[connection]\n"
                    'account = "${ENV:TEST_ACCOUNT}"\n'
                    'warehouse = "TEST_WH"\n'
                    'database = "TEST_DB"\n'
                    'schema = "TEST_SCHEMA"\n'
                ),
                "models/stg_orders.sql": "MODEL (description 'Test model stg_orders.', materialized view);\n\nSELECT 1 AS order_id\n",
            },
            expected_error_fragment="snowflake execution requires explicit target database",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_local_override_without_target_namespace_when_compiling_then_it_fails_early(
    test_case: SnowflakeTargetValidationIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("TEST_ACCOUNT", "test-account")
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        build_compiled_project(
            discovered_inputs=discovered_inputs,
            adapter=SnowflakeAdapter(),
            no_sql_validation=True,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeTargetValidationIntegrationTestCase(
            description="snowflake local target namespace resolves model target",
            project_files={
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sqlbuild_local.toml": (
                    'adapter = "snowflake"\n'
                    'target = "dev"\n\n'
                    "[connection]\n"
                    'database = "${ENV:TEST_DB}"\n'
                    'schema = "${ENV:TEST_SCHEMA}"\n\n'
                    "[targets.dev]\n"
                    'database = "${ENV:TEST_DB}"\n'
                    'schema = "${ENV:TEST_SCHEMA}"\n'
                ),
                "models/stg_orders.sql": "MODEL (description 'Test model stg_orders.', materialized view);\n\nSELECT 1 AS order_id\n",
            },
            expected_database="LOCAL_DB",
            expected_schema="LOCAL_SCHEMA",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_local_target_namespace_when_compiling_then_targets_resolve(
    test_case: SnowflakeTargetValidationIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("TEST_DB", "LOCAL_DB")
    monkeypatch.setenv("TEST_SCHEMA", "LOCAL_SCHEMA")
    write_repo_files(tmp_path, test_case.project_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered_inputs,
        adapter=SnowflakeAdapter(),
        no_sql_validation=True,
    )

    assert project.models[0].destination.database == test_case.expected_database
    assert project.models[0].destination.schema == test_case.expected_schema
    assert project.effective_target_database == test_case.expected_database
    assert project.effective_target_schema == test_case.expected_schema


@pytest.mark.parametrize(
    "test_case",
    [
        DeferToIntegrationTestCase(
            description="defer-to resolves unselected ref to deferred target schema",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\n'
                    'adapter = "duckdb"\n'
                    'default_target = "dev"\n\n'
                    "[connection]\n"
                    'database = ":memory:"\n\n'
                    "[targets.dev]\n"
                    'schema = "dev_schema"\n\n'
                    "[targets.prod]\n"
                    'schema = "prod_schema"\n'
                ),
                "models/stg_orders.sql": (
                    "MODEL (description 'Test model stg_orders.', materialized table);\n\nSELECT 1 AS order_id"
                ),
                "models/fact_orders.sql": (
                    'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT order_id FROM __ref("stg_orders")'
                ),
            },
            defer_to="prod",
            select=("fact_orders",),
            expected_model_count=1,
            expected_resolved_sql_fragments={
                "fact_orders": "prod_schema.stg_orders",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_with_defer_to_when_compiling_then_resolves_refs_to_deferred_target(
    test_case: DeferToIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    db_path: str = str(tmp_path / "test.duckdb")
    project_files: dict[str, str] = {
        k: v.replace(":memory:", db_path) for k, v in test_case.project_files.items()
    }
    write_repo_files(tmp_path, project_files)

    import duckdb

    conn: duckdb.DuckDBPyConnection = duckdb.connect(db_path)
    conn.execute("CREATE SCHEMA IF NOT EXISTS prod_schema")
    conn.execute("CREATE TABLE prod_schema.stg_orders (order_id INTEGER)")
    conn.close()

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
        defer_to=test_case.defer_to,
        select=test_case.select,
    )

    assert len(result.plan_output.model_entries) == test_case.expected_model_count
    entry_map: dict[str, ModelPlanEntry] = {e.name: e for e in result.plan_output.model_entries}
    model_name: str
    expected_fragment: str
    for model_name, expected_fragment in test_case.expected_resolved_sql_fragments.items():
        assert expected_fragment in entry_map[model_name].resolved_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlAnalysisChainCompileTargetIntegrationTestCase(
            description="chain test compile target defines each model once as a generated cte",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\n'
                    'adapter = "duckdb"\n\n'
                    "[connection]\n"
                    'database = "warehouse.duckdb"\n\n'
                    "[settings]\n"
                    "sql_analysis = true\n"
                ),
                "models/stg_orders.sql": (
                    'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT id, amount FROM __source("raw")'
                ),
                "models/fact_orders.sql": (
                    "MODEL (description 'Test model fact_orders.', materialized table);\n\n"
                    "WITH local_helper AS (SELECT 1 AS one) "
                    'SELECT id, amount + one AS adjusted FROM __ref("stg_orders") '
                    "CROSS JOIN local_helper"
                ),
                "sources/raw.yml": "sources:\n  - name: raw\n    description: Test source raw.\n    schema: main\n    table: raw\n",
                "tests/unit/test_chain.sql": (
                    "TEST();\n\n"
                    "WITH\n"
                    "__source__raw AS (SELECT 1 AS id, 100 AS amount),\n"
                    "__expected__stg_orders AS (SELECT 1 AS id, 100 AS amount),\n"
                    "__expected__fact_orders AS (SELECT 1 AS id, 101 AS adjusted)\n"
                    "SELECT 1\n"
                ),
            },
            warehouse_setup_sql=("CREATE TABLE main.raw AS SELECT 1 AS id, 100 AS amount",),
            compiled_test_path=(
                "target/compiled/tests/_chain_/fact_orders__stg_orders/test_chain.sql"
            ),
            expected_fragments=(
                "WITH __source__raw AS (",
                "__ref__stg_orders AS (",
                "FROM __source__raw",
                "__ref__fact_orders AS (WITH local_helper AS (SELECT 1 AS one) "
                "SELECT id, amount + one AS adjusted FROM __ref__stg_orders "
                "CROSS JOIN local_helper),\n",
                "__actual__stg_orders AS (SELECT * FROM __ref__stg_orders)",
                "__actual__fact_orders AS (SELECT * FROM __ref__fact_orders)",
                "FROM __ref__stg_orders",
                "'stg_orders' AS model_name",
            ),
            unexpected_fragments=(
                "__ref__stg_orders AS (WITH",
                "\nlocal_helper AS (",
                "__REF(",
                "__SOURCE(",
                "\n\nWITH",
                "__actual_0 AS (WITH",
                "__actual_0 AS (\n  WITH",
                "__actual_1 AS (\n  WITH",
                "__actual_0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_analysis_enabled_chain_test_when_writing_compile_target_then_uses_ctes(
    test_case: SqlAnalysisChainCompileTargetIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    monkeypatch.chdir(tmp_path)
    apply_warehouse_setup_sql(project_dir=tmp_path, statements=test_case.warehouse_setup_sql)

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
    )
    write_compile_target(
        target_dir=tmp_path / "target",
        adapter=DuckDbAdapter(),
        plan_output=result.plan_output,
    )

    compiled_test_sql: str = (tmp_path / test_case.compiled_test_path).read_text(encoding="utf-8")

    expected_fragment: str
    for expected_fragment in test_case.expected_fragments:
        assert expected_fragment in compiled_test_sql
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_fragments:
        assert unexpected_fragment not in compiled_test_sql


@pytest.mark.parametrize(
    "test_case",
    [
        AppendCursorPipelineIntegrationTestCase(
            description="append cursor defaults to inclusive lower bound in resolved sql",
            model_header_cursor_config="",
            expected_resolved_sql_fragment="WHERE ordered_at >= TIMESTAMP '2026-01-01 00:00:00'",
        ),
        AppendCursorPipelineIntegrationTestCase(
            description="append cursor can use exclusive lower bound in resolved sql",
            model_header_cursor_config="  append_cursor_inclusive false,\n",
            expected_resolved_sql_fragment="WHERE ordered_at > TIMESTAMP '2026-01-01 00:00:00'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_append_cursor_model_when_compiling_then_sql_uses_expected_lower_bound(
    test_case: AppendCursorPipelineIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    db_path: Path = tmp_path / "append_cursor.duckdb"
    model_header: str = (
        "MODEL (\n"
        '  description "Test model orders.",\n'
        "  materialized incremental,\n"
        "  incremental_strategy append,\n"
        "  cursor ordered_at,\n"
        "  cursor_type timestamp,\n"
        "  cursor_grain second,\n"
        + test_case.model_header_cursor_config
        + ");\n\n"
        + 'SELECT id, ordered_at FROM __source("raw_orders")'
    )
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": (
                f'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "{db_path}"\n'
            ),
            "sources/raw.yml": (
                "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    schema: main\n    table: raw_orders\n"
            ),
            "models/orders.sql": model_header,
        },
    )

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    connection.execute("CREATE TABLE main.raw_orders (id INTEGER, ordered_at TIMESTAMP)")
    connection.execute(
        "INSERT INTO main.raw_orders VALUES (1, '2026-01-01 00:00:00'), (2, '2026-01-01 01:00:00')"
    )
    connection.execute("CREATE TABLE main.orders (id INTEGER, ordered_at TIMESTAMP)")
    connection.execute("INSERT INTO main.orders VALUES (1, '2026-01-01 00:00:00')")
    connection.close()

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path,
        adapter=DuckDbAdapter(),
    )

    entry_map: dict[str, ModelPlanEntry] = {e.name: e for e in result.plan_output.model_entries}
    assert test_case.expected_resolved_sql_fragment in entry_map["orders"].resolved_sql
