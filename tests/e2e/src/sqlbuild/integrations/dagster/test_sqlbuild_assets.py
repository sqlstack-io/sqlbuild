"""E2E tests for executing SQLBuild through Dagster assets."""

import json
import os
import runpy
import subprocess
import threading
import time
from collections.abc import Iterator, Mapping
from pathlib import Path
from typing import Any, cast

import dagster as dg
import pytest
from dagster import (
    AssetCheckExecutionContext,
    AssetCheckSeverity,
    AssetExecutionContext,
    AssetKey,
    ExecuteInProcessResult,
    materialize,
)

from sqlbuild._native import PlaygroundCommandRequest
from sqlbuild._native import run_playground_command as run_playground
from sqlbuild.integrations.dagster import (
    SqlBuildCliResource,
    SqlBuildDagsterTranslator,
    SqlBuildProject,
    build_sqlbuild_asset_selection,
    sqlbuild_assets,
    sqlbuild_scenario_checks,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    build_empty_cursor_project,
    execute_empty_cursor_sql,
    load_empty_cursor_payments,
    prepare_empty_cursor_inputs_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    prepare_inline_project,
    prepare_source_loader_strategies,
    prepare_waffle_shop,
    run_sqb,
    table_exists,
)
from tests.e2e.src.sqlbuild.integrations.dagster._test_types import (
    DagsterColumnAuditChecksE2ETestCase,
    DagsterEmptyCursorInputsCheckE2ETestCase,
    DagsterPlaygroundE2ETestCase,
    DagsterPythonNodesArtifactE2ETestCase,
    DagsterSqlBuildE2ETestCase,
    DagsterSqlBuildFailedCheckE2ETestCase,
    DagsterSqlBuildLoaderE2ETestCase,
    DagsterSqlBuildScenarioE2ETestCase,
    DagsterSqlBuildSelectionE2ETestCase,
    DagsterSqlBuildStreamingE2ETestCase,
    DagsterTranslatorE2ETestCase,
)
from tests.e2e.src.sqlbuild.integrations.dagster.helpers import (
    add_failing_daily_revenue_audits,
    check_names_for_asset,
    check_severity_for_asset,
    failed_check_severities_for_asset,
    materialization_metadata_keys,
    prepare_python_nodes_integration_project,
    wait_for_captured_stdout_fragment,
    write_sqb_capture_command,
    write_sqb_streaming_command,
)


class _NamespacedDagsterTranslator(SqlBuildDagsterTranslator):
    def get_asset_key(self, node: Mapping[str, Any]) -> AssetKey:
        return AssetKey(["translated", str(node["name"])])


@pytest.mark.parametrize(
    "test_case",
    (
        DagsterColumnAuditChecksE2ETestCase(
            description="column arguments produce unique checks while non-column IDs stay stable",
            expected_check_names=(
                "audit__model_is_empty__orders",
                "audit__not_null__customer_id",
                "audit__not_null__order_id",
            ),
            expected_check_ids=(
                "audit:model_is_empty:model:orders",
                "audit:not_null:model:orders:customer_id",
                "audit:not_null:model:orders:order_id",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_model_level_column_audits_when_loading_dagster_assets_then_checks_are_unique(
    test_case: DagsterColumnAuditChecksE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="column_audit_checks",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "column_audit_checks"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "column_audit_checks.duckdb"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Test model orders.',\n"
                "  columns (\n"
                "    order_id (),\n"
                "    customer_id (),\n"
                "  ),\n"
                "  audits [\n"
                "    not_null (column order_id, severity error),\n"
                "    not_null (column customer_id, severity error),\n"
                "    model_is_empty (severity error),\n"
                "  ],\n"
                ");\n\n"
                "SELECT 1 AS order_id, 2 AS customer_id\n"
            ),
            "sql/audits/generic/model_is_empty.sql": (
                'AUDIT ();\n\nSELECT * FROM __ref("@model")\n'
            ),
        },
    )
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(REPO_ROOT / ".venv" / "bin" / "sqb"),),
    )

    sqlbuild_project.prepare()
    dag_payload: dict[str, object] = json.loads(
        sqlbuild_project.dag_path.read_text(encoding="utf-8")
    )
    dag_checks: list[dict[str, object]] = cast(list[dict[str, object]], dag_payload["checks"])

    @sqlbuild_assets(project=sqlbuild_project)
    def sqlbuild_orders(context: AssetExecutionContext) -> Iterator[object]:
        del context
        return
        yield

    assert {spec.name for spec in sqlbuild_orders.check_specs} == set(
        test_case.expected_check_names
    )
    assert {check["id"] for check in dag_checks} == set(test_case.expected_check_ids)


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterPythonNodesArtifactE2ETestCase(
            description="dagster consumes real Python-node dag artifact",
            expected_asset_keys=(
                ("main", "orders"),
                ("asset", "prepare_orders"),
                ("warehouse_export",),
                ("asset", "orders_export"),
            ),
            expected_check_names=("python_check__check_orders_export",),
            expected_task_group="python",
            expected_asset_group="exports",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_nodes_project_when_loading_dagster_assets_then_maps_real_artifact(
    test_case: DagsterPythonNodesArtifactE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_python_nodes_integration_project(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )

    sqlbuild_project.prepare()

    @sqlbuild_assets(project=sqlbuild_project)
    def sqlbuild_python_nodes(context: AssetExecutionContext) -> Iterator[object]:
        del context
        return
        yield

    asset_keys: set[tuple[str, ...]] = {tuple(key.path) for key in sqlbuild_python_nodes.keys}
    specs_by_key: dict[tuple[str, ...], tuple[Any, ...]] = {
        tuple(spec.key.path): (spec,) for spec in sqlbuild_python_nodes.specs
    }
    assert len(specs_by_key) == len(sqlbuild_python_nodes.specs)
    task_spec: object = next(iter(specs_by_key.get(("asset", "prepare_orders"), ())))
    asset_spec: object = next(iter(specs_by_key.get(("asset", "orders_export"), ())))

    assert sqlbuild_project.dag_path.exists()
    assert set(test_case.expected_asset_keys) <= asset_keys
    assert set(test_case.expected_check_names) <= {
        spec.name for spec in sqlbuild_python_nodes.check_specs
    }
    assert task_spec.group_name == test_case.expected_task_group
    assert asset_spec.group_name == test_case.expected_asset_group


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildE2ETestCase(
            description="dagster loads generated dag artifact and executes sqlbuild build",
            expected_success=True,
            expected_dag_artifact="target/sqlbuild_dag.json",
            expected_table_names=(
                "raw_customers",
                "raw_orders",
                "fact_orders",
                "dim_customers",
                "waffle_types",
            ),
            expected_asset_keys=(
                ("raw_customers",),
                ("raw_orders",),
                ("main", "waffle_types"),
                ("main", "is_completed_order"),
                ("main", "fact_orders"),
            ),
            expected_json_metadata_asset_key=("main", "fact_orders"),
            expected_json_metadata_keys=("duration_ms", "kind", "name", "status", "target"),
            expected_check_asset_key=("main", "fact_orders"),
            expected_check_names=("audit__not_null__order_id", "sql_test__test_fact_orders"),
            expected_warn_check_name="audit__not_null__order_id",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_executing_sqlbuild_assets_then_dagster_run_succeeds(
    test_case: DagsterSqlBuildE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")

    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_waffle_shop(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["build"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_waffle_shop],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
    )

    assert result.success is test_case.expected_success
    assert sqlbuild_project.dag_path == project_dir / test_case.expected_dag_artifact
    assert sqlbuild_project.dag_path.exists()
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=project_dir / "waffle_shop.duckdb", table_name=table_name)
    assert set(test_case.expected_asset_keys) <= {
        tuple(asset_key.path) for asset_key in sqlbuild_waffle_shop.keys
    }
    assert set(test_case.expected_json_metadata_keys) <= materialization_metadata_keys(
        result=result,
        asset_key=test_case.expected_json_metadata_asset_key,
    )
    assert set(test_case.expected_check_names) <= check_names_for_asset(
        result=result,
        asset_key=test_case.expected_check_asset_key,
    )
    assert (
        check_severity_for_asset(
            result=result,
            asset_key=test_case.expected_check_asset_key,
            check_name=test_case.expected_warn_check_name,
        )
        == AssetCheckSeverity.WARN
    )
    rendered_logs: str = capsys.readouterr().err
    assert '"version": 1' not in rendered_logs
    assert '"assets": [' not in rendered_logs
    assert '"checks": [' not in rendered_logs


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildSelectionE2ETestCase(
            description="dagster subset selection runs sqlbuild with select file",
            selected_asset_keys=(("main", "waffle_types"),),
            expected_success=True,
            expected_selector_file_contents="waffle_types\n",
            expected_selector_log_line="waffle_types",
            expected_table_names=("waffle_types",),
        ),
        DagsterSqlBuildSelectionE2ETestCase(
            description="dagster multi-asset selection writes all sqlbuild selectors",
            selected_asset_keys=(
                ("raw_customers",),
                ("raw_orders",),
                ("raw_payments",),
                ("main", "waffle_types"),
                ("main", "stg_customers"),
                ("main", "stg_orders"),
                ("main", "stg_payments"),
            ),
            expected_success=True,
            expected_selector_file_contents=(
                "raw_customers\nraw_orders\nraw_payments\nwaffle_types\n"
                "stg_customers\nstg_orders\nstg_payments\n"
            ),
            expected_selector_log_line=(
                "raw_customers raw_orders raw_payments waffle_types "
                "stg_customers stg_orders stg_payments"
            ),
            expected_table_names=(
                "raw_customers",
                "raw_orders",
                "waffle_types",
                "stg_customers",
                "stg_orders",
                "stg_payments",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dagster_asset_selection_when_executing_sqlbuild_then_uses_select_file(
    test_case: DagsterSqlBuildSelectionE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    command_log_path: Path = tmp_path / "sqb_command_log.txt"
    selector_log_path: Path = tmp_path / "sqb_selector_log.txt"
    sqb_command: tuple[str, ...] = write_sqb_capture_command(
        root=tmp_path,
        command_log_path=command_log_path,
        selector_log_path=selector_log_path,
    )
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=sqb_command,
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_waffle_shop(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["build"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_waffle_shop],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=[AssetKey(list(asset_key)) for asset_key in test_case.selected_asset_keys],
    )

    assert result.success is test_case.expected_success
    assert "build --select-file" in command_log_path.read_text(encoding="utf-8")
    assert selector_log_path.read_text(encoding="utf-8") == (
        test_case.expected_selector_file_contents
    )
    rendered_logs: str = capsys.readouterr().err
    assert "SQLBuild command:" in rendered_logs
    assert "build --select-file" in rendered_logs
    assert (
        f"SQLBuild selected assets from Dagster: {len(test_case.selected_asset_keys)} "
        "(selector file: "
    ) in rendered_logs
    assert test_case.expected_selector_log_line in rendered_logs
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=project_dir / "waffle_shop.duckdb", table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    (
        DagsterTranslatorE2ETestCase(
            description="custom translator controls selection and materialization identity",
            selected_asset_key=("translated", "waffle_types"),
            expected_selector_file_contents="waffle_types\n",
            expected_table_name="waffle_types",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_custom_translator_when_materializing_real_project_then_selection_and_events_share_keys(
    test_case: DagsterTranslatorE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    command_log_path: Path = tmp_path / "sqb_command_log.txt"
    selector_log_path: Path = tmp_path / "sqb_selector_log.txt"
    translator: SqlBuildDagsterTranslator = _NamespacedDagsterTranslator()
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=write_sqb_capture_command(
            root=tmp_path,
            command_log_path=command_log_path,
            selector_log_path=selector_log_path,
        ),
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(
        project=sqlbuild_project,
        translator=translator,
        required_resource_keys={"sqb"},
    )
    def sqlbuild_waffle_shop(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(
            ["build"], context=context, translator=translator
        ).stream()

    selection: Any = build_sqlbuild_asset_selection(
        sqlbuild_assets=[sqlbuild_waffle_shop],
        dag=sqlbuild_project.dag_path,
        sqlbuild_select="waffle_types",
        translator=translator,
    )
    result: ExecuteInProcessResult = materialize(
        [sqlbuild_waffle_shop],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=selection,
    )

    assert result.success
    assert (
        selector_log_path.read_text(encoding="utf-8") == test_case.expected_selector_file_contents
    )
    assert {event.asset_key for event in result.get_asset_materialization_events()} == {
        AssetKey(test_case.selected_asset_key)
    }
    assert table_exists(
        db_path=project_dir / "waffle_shop.duckdb",
        table_name=test_case.expected_table_name,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildLoaderE2ETestCase(
            description="dagster selected loader asset runs sqlbuild load",
            selected_asset_key=("raw_countries",),
            expected_success=True,
            expected_selector_file_contents="raw_countries\n",
            expected_table_names=("raw_countries",),
            expected_metadata_asset_key=("raw_countries",),
            expected_metadata_keys=(
                "duration_ms",
                "kind",
                "loader",
                "name",
                "rows_loaded",
                "status",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_project_when_executing_sqlbuild_load_asset_then_loader_runs(
    test_case: DagsterSqlBuildLoaderE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_source_loader_strategies(tmp_path=tmp_path)
    command_log_path: Path = tmp_path / "sqb_command_log.txt"
    selector_log_path: Path = tmp_path / "sqb_selector_log.txt"
    sqb_command: tuple[str, ...] = write_sqb_capture_command(
        root=tmp_path,
        command_log_path=command_log_path,
        selector_log_path=selector_log_path,
    )
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=sqb_command,
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_loaders(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["load"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_loaders],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=[AssetKey(list(test_case.selected_asset_key))],
    )

    assert result.success is test_case.expected_success
    assert "load --select-file" in command_log_path.read_text(encoding="utf-8")
    assert selector_log_path.read_text(encoding="utf-8") == (
        test_case.expected_selector_file_contents
    )
    for table_name in test_case.expected_table_names:
        assert table_exists(
            db_path=project_dir / "source_loader_strategies.duckdb",
            table_name=table_name,
        )
    assert set(test_case.expected_metadata_keys) <= materialization_metadata_keys(
        result=result,
        asset_key=test_case.expected_metadata_asset_key,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildLoaderE2ETestCase(
            description="dagster selected chained source reuses existing intermediate target",
            selected_asset_key=("raw_events",),
            expected_success=True,
            expected_selector_file_contents="raw_events\n",
            expected_table_names=("raw_events",),
            expected_metadata_asset_key=("raw_events",),
            expected_metadata_keys=("kind", "loader", "name", "status"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_source_loader_when_dagster_selects_source_then_reuses_intermediate_target(
    test_case: DagsterSqlBuildLoaderE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="dagster_chained_source_loader",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "dagster_chained_source_loader"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "dagster_chained_source_loader.duckdb"\n'
            ),
            "python/nodes/loaders/raw.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    ctx.execute_sql(f'CREATE OR REPLACE TABLE {ctx.destination} AS "
                "SELECT event_id FROM {events.destination}')\n"
            ),
            "sql/sources/raw.yml": "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n",
        },
    )
    setup_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "load", "--select", "fetch_events"),
        project_dir=project_dir,
    )
    assert setup_result.returncode == 0, setup_result.stdout + setup_result.stderr
    command_log_path: Path = tmp_path / "sqb_command_log.txt"
    selector_log_path: Path = tmp_path / "sqb_selector_log.txt"
    sqb_command: tuple[str, ...] = write_sqb_capture_command(
        root=tmp_path,
        command_log_path=command_log_path,
        selector_log_path=selector_log_path,
    )
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=sqb_command,
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_loaders(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["load"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_loaders],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=[AssetKey(list(test_case.selected_asset_key))],
    )

    assert result.success is test_case.expected_success
    assert selector_log_path.read_text(encoding="utf-8") == (
        test_case.expected_selector_file_contents
    )
    assert "load --select-file" in command_log_path.read_text(encoding="utf-8")
    for table_name in test_case.expected_table_names:
        assert table_exists(
            db_path=project_dir / "dagster_chained_source_loader.duckdb",
            table_name=table_name,
        )
    assert set(test_case.expected_metadata_keys) <= materialization_metadata_keys(
        result=result,
        asset_key=test_case.expected_metadata_asset_key,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildLoaderE2ETestCase(
            description="dagster selected chained source fails when intermediate target is missing",
            selected_asset_key=("raw_events",),
            expected_success=False,
            expected_selector_file_contents="raw_events\n",
            expected_table_names=(),
            expected_metadata_asset_key=("raw_events",),
            expected_metadata_keys=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_chained_source_loader_when_dagster_selects_source_without_intermediate_then_fails(
    test_case: DagsterSqlBuildLoaderE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="dagster_missing_intermediate_source_loader",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "dagster_missing_intermediate_source_loader"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "dagster_missing_intermediate_source_loader.duckdb"\n'
            ),
            "python/nodes/loaders/raw.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(write_strategy='table', columns=[\n"
                "    {'name': 'event_id', 'type': 'INTEGER'},\n"
                "])\n"
                "def fetch_events(ctx):\n"
                "    '''Test loader fetch_events.'''\n    return [{'event_id': 1}]\n\n"
                "@loader(depends_on=[fetch_events])\n"
                "def raw_events(ctx):\n"
                "    '''Test loader raw_events.'''\n    events = ctx.loader(fetch_events)\n"
                "    ctx.execute_sql(f'CREATE OR REPLACE TABLE {ctx.destination} AS "
                "SELECT event_id FROM {events.destination}')\n"
            ),
            "sql/sources/raw.yml": "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n",
        },
    )
    command_log_path: Path = tmp_path / "sqb_command_log.txt"
    selector_log_path: Path = tmp_path / "sqb_selector_log.txt"
    sqb_command: tuple[str, ...] = write_sqb_capture_command(
        root=tmp_path,
        command_log_path=command_log_path,
        selector_log_path=selector_log_path,
    )
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=sqb_command,
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_loaders(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["load"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_loaders],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=[AssetKey(list(test_case.selected_asset_key))],
        raise_on_error=False,
    )

    assert result.success is test_case.expected_success
    assert selector_log_path.read_text(encoding="utf-8") == (
        test_case.expected_selector_file_contents
    )
    assert "load --select-file" in command_log_path.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterPlaygroundE2ETestCase(
            description="generated dagster playground materializes loader-backed waffle shop",
            expected_success=True,
            expected_table_names=("raw__customers", "raw__orders", "fact_orders"),
            expected_schema="dev",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_dagster_playground_when_materializing_assets_then_build_succeeds(
    test_case: DagsterPlaygroundE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    playground_name: str = "dagster_waffle_shop"
    assert (
        run_playground(
            PlaygroundCommandRequest(
                project_dir=tmp_path, target_path=playground_name, template="dagster"
            )
        )
        == 0
    )
    orchestrator_dir: Path = tmp_path / playground_name
    project_dir: Path = orchestrator_dir / "waffle_shop"
    sqb_bin_dir: Path = REPO_ROOT / ".venv" / "bin"
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    monkeypatch.setenv("PATH", f"{sqb_bin_dir}{os.pathsep}{os.environ.get('PATH', '')}")

    generated_defs: dict[str, object] = runpy.run_path(str(orchestrator_dir / "definitions.py"))
    waffle_shop_assets: object = generated_defs["waffle_shop_assets"]
    sqlbuild_project: SqlBuildProject = generated_defs["SQLBUILD_PROJECT"]  # type: ignore[assignment]

    result: ExecuteInProcessResult = materialize(
        [waffle_shop_assets],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
    )

    assert result.success is test_case.expected_success
    for table_name in test_case.expected_table_names:
        assert table_exists(
            db_path=project_dir / "waffle_shop_control.duckdb",
            table_name=table_name,
            schema=test_case.expected_schema,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildStreamingE2ETestCase(
            description="sqlbuild stdout is visible before process exits",
            selected_asset_key=("main", "waffle_types"),
            expected_stdout_fragment="streamed before exit\n",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_process_is_still_running_when_emitting_stdout_then_dagster_streams_output(
    test_case: DagsterSqlBuildStreamingE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()
    started_path: Path = tmp_path / "stream-started.txt"
    release_path: Path = tmp_path / "stream-release.txt"
    json_payload: str = (
        '{"version": 1, "command": "build", "status": "success", '
        '"summary": {}, "assets": [{"kind": "seed", "name": "waffle_types", '
        '"status": "success", "duration_ms": 1}], "checks": []}'
    )
    streaming_command: tuple[str, ...] = write_sqb_streaming_command(
        root=tmp_path,
        started_path=started_path,
        release_path=release_path,
        stdout_text=test_case.expected_stdout_fragment,
        json_payload=json_payload,
    )

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_waffle_shop(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["build"], context=context).stream()

    result_out: list[ExecuteInProcessResult] = []
    error_out: list[BaseException] = []

    def run_materialize() -> None:
        try:
            result_out.append(
                materialize(
                    [sqlbuild_waffle_shop],
                    resources={
                        "sqb": SqlBuildCliResource(
                            project_dir=sqlbuild_project,
                            sqb_command=list(streaming_command),
                        )
                    },
                    selection=[AssetKey(list(test_case.selected_asset_key))],
                )
            )
        except BaseException as error:
            error_out.append(error)

    thread: threading.Thread = threading.Thread(target=run_materialize)
    thread.start()
    try:
        deadline: float = time.monotonic() + 10.0
        while not started_path.exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert started_path.exists()
        rendered_stdout: str = wait_for_captured_stdout_fragment(
            capsys=capsys,
            expected_fragment=test_case.expected_stdout_fragment,
            deadline=deadline,
        )
        assert test_case.expected_stdout_fragment in rendered_stdout
    finally:
        release_path.write_text("release", encoding="utf-8")
        thread.join(timeout=10.0)

    assert not thread.is_alive()
    assert error_out == []
    assert result_out[0].success


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildFailedCheckE2ETestCase(
            description="failed warn and error audits stay linked to selected asset",
            selected_asset_key=("main", "daily_revenue"),
            expected_check_names=(
                "audit__forced_warning_failure__daily_revenue",
                "audit__forced_error_failure__daily_revenue",
                "audit__forced_measurement_warning__daily_revenue",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failing_sqlbuild_audits_when_executing_dagster_then_links_checks_with_severity(
    test_case: DagsterSqlBuildFailedCheckE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    build_resource: SqlBuildCliResource = SqlBuildCliResource(
        project_dir=project_dir,
        sqb_command=[str(sqb_executable)],
    )
    build_resource.cli(args=["build"]).wait()
    add_failing_daily_revenue_audits(project_dir=project_dir)
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_waffle_shop(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(
            ["audit", "--select", "daily_revenue"],
            context=context,
            raise_on_error=False,
        ).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_waffle_shop],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=[AssetKey(list(test_case.selected_asset_key))],
    )

    assert result.success
    severities: dict[str, AssetCheckSeverity] = failed_check_severities_for_asset(
        result=result,
        asset_key=test_case.selected_asset_key,
    )
    assert set(test_case.expected_check_names) <= set(severities)
    assert severities["audit__forced_warning_failure__daily_revenue"] == AssetCheckSeverity.WARN
    assert severities["audit__forced_error_failure__daily_revenue"] == AssetCheckSeverity.ERROR
    assert severities["audit__forced_measurement_warning__daily_revenue"] == AssetCheckSeverity.WARN


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildScenarioE2ETestCase(
            description="scenario test only runs scenarios attached to selected assets",
            selected_asset_keys=(("main", "daily_revenue"),),
            expected_command_fragment=(
                "scenario test daily_revenue_minimal daily_revenue_multi_order --json"
            ),
            unexpected_command_fragment="fact_order_retained_artifacts",
            daily_revenue_asset_key=("main", "daily_revenue"),
            expected_daily_revenue_check_names=(
                "scenario__daily_revenue_minimal",
                "scenario__daily_revenue_multi_order",
            ),
            scenario_order_prices_asset_key=("main", "scenario_order_prices"),
            unexpected_scenario_order_prices_check_names=(
                "scenario__fact_order_retained_artifacts",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_scenarios_when_executing_dagster_then_emits_scenario_checks(
    test_case: DagsterSqlBuildScenarioE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_waffle_shop(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["scenario", "test"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_waffle_shop],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=[AssetKey(list(asset_key)) for asset_key in test_case.selected_asset_keys],
    )

    assert result.success
    rendered_logs: str = capsys.readouterr().err
    assert test_case.expected_command_fragment in rendered_logs
    assert test_case.unexpected_command_fragment not in rendered_logs
    assert set(test_case.expected_daily_revenue_check_names) <= check_names_for_asset(
        result=result,
        asset_key=test_case.daily_revenue_asset_key,
    )
    assert not set(test_case.unexpected_scenario_order_prices_check_names).intersection(
        check_names_for_asset(
            result=result,
            asset_key=test_case.scenario_order_prices_asset_key,
        )
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterSqlBuildScenarioE2ETestCase(
            description="scenario helper only runs selected scenario checks",
            selected_asset_keys=(("main", "daily_revenue"),),
            expected_command_fragment="scenario test daily_revenue_minimal --json",
            unexpected_command_fragment="daily_revenue_multi_order",
            daily_revenue_asset_key=("main", "daily_revenue"),
            expected_daily_revenue_check_names=("scenario__daily_revenue_minimal",),
            scenario_order_prices_asset_key=("main", "scenario_order_prices"),
            unexpected_scenario_order_prices_check_names=(
                "scenario__fact_order_retained_artifacts",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_check_selection_when_executing_dagster_then_runs_only_selected_scenarios(
    test_case: DagsterSqlBuildScenarioE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_scenario_checks(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_waffle_shop_scenarios(context: AssetCheckExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["scenario", "test"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_waffle_shop_scenarios],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
        selection=dg.AssetSelection.checks(
            dg.AssetCheckKey(
                asset_key=AssetKey(list(test_case.daily_revenue_asset_key)),
                name="scenario__daily_revenue_minimal",
            )
        ),
    )

    assert result.success
    rendered_logs: str = capsys.readouterr().err
    assert test_case.expected_command_fragment in rendered_logs
    assert test_case.unexpected_command_fragment not in rendered_logs
    assert set(test_case.expected_daily_revenue_check_names) <= check_names_for_asset(
        result=result,
        asset_key=test_case.daily_revenue_asset_key,
    )
    assert not set(test_case.unexpected_scenario_order_prices_check_names).intersection(
        check_names_for_asset(
            result=result,
            asset_key=test_case.scenario_order_prices_asset_key,
        )
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterEmptyCursorInputsCheckE2ETestCase(
            description="incremental run with no input rows still reports declared audits",
            model_sql=(
                "MODEL (\n"
                '  description "Daily revenue from payments",\n'
                "  materialized incremental,\n"
                "  incremental_strategy delete_insert,\n"
                "  cursor revenue_date,\n"
                "  cursor_type timestamp,\n"
                "  cursor_grain day,\n"
                "  cursor_inputs (raw_payments paid_at),\n"
                "  columns (total_revenue_cents (audits [not_null])),\n"
                ");\n\n"
                "SELECT CAST(paid_at AS DATE) AS revenue_date, "
                "SUM(amount_cents) AS total_revenue_cents\n"
                'FROM __source("raw_payments")\n'
                "GROUP BY 1\n"
            ),
            asset_key=("main", "daily_revenue"),
            expected_check_names=("audit__not_null__total_revenue_cents",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_emptied_cursor_inputs_when_executing_dagster_then_emits_declared_checks(
    test_case: DagsterEmptyCursorInputsCheckE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models={"sql/models/daily_revenue.sql": test_case.model_sql}
    )
    load_empty_cursor_payments(project_dir=project_dir)
    build_empty_cursor_project(project_dir=project_dir)
    execute_empty_cursor_sql(project_dir=project_dir, sql="DELETE FROM main.raw_payments")
    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    sqlbuild_project: SqlBuildProject = SqlBuildProject(
        project_dir=project_dir,
        sqb_command=(str(sqb_executable),),
    )
    monkeypatch.setenv("DAGSTER_IS_DEV_CLI", "1")
    sqlbuild_project.prepare_if_dev()

    @sqlbuild_assets(project=sqlbuild_project, required_resource_keys={"sqb"})
    def sqlbuild_empty_inputs(context: AssetExecutionContext) -> Iterator[object]:
        yield from context.resources.sqb.cli(["build"], context=context).stream()

    result: ExecuteInProcessResult = materialize(
        [sqlbuild_empty_inputs],
        resources={"sqb": SqlBuildCliResource(project_dir=sqlbuild_project)},
    )

    assert result.success
    assert AssetKey(list(test_case.asset_key)) in {
        event.asset_key for event in result.get_asset_materialization_events()
    }
    assert check_names_for_asset(result=result, asset_key=test_case.asset_key) == frozenset(
        test_case.expected_check_names
    )
    assert all(evaluation.passed for evaluation in result.get_asset_check_evaluations())
