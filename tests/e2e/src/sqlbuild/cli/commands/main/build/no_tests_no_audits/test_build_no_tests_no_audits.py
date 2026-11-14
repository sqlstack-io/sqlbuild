"""E2E tests for sqb build --no-tests --no-audits command."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build.no_tests_no_audits._test_types import (
    RunE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
    query_duckdb,
    row_count,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes discovered Python lifecycle hooks",
            expected_exit_code=0,
            expected_table_names=("orders", "hook_log"),
            expected_view_names=(),
            expected_output_fragments=(
                "Execution  sqb build",
                "table     orders",
                "pre_hook  python  log_hook",
                "post_hook python  log_hook",
            ),
            expected_query_results=(
                ("SELECT order_id FROM main.orders", ((1,),)),
                (
                    "SELECT model_name, phase FROM main.hook_log ORDER BY phase",
                    (("orders", "post_hooks"), ("orders", "pre_hooks")),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_with_python_hooks_when_running_run_then_hooks_execute(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_hooks_run_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "python_hooks_run_project"
                adapter = "duckdb"

                [connection]
                database = "python_hooks_run_project.duckdb"
                """
            ).strip()
            + "\n",
            "hooks/python/lifecycle.py": dedent(
                """
                from sqlbuild.hooks import HookContext, hook


                @hook
                def log_hook(ctx: HookContext):
                    '''Test hook log_hook.'''
                    ctx.execute_sql(
                        f"CREATE TABLE IF NOT EXISTS {ctx.destination.schema}.hook_log "
                        "(model_name VARCHAR, phase VARCHAR)"
                    )
                    ctx.execute_sql(
                        f"INSERT INTO {ctx.destination.schema}.hook_log VALUES "
                        f"('{ctx.model_name}', '{ctx.phase}')"
                    )
                """
            ).strip()
            + "\n",
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("log_hook")],
                  post_hooks [python("log_hook")]
                );

                SELECT 1 AS order_id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits"),
        project_dir=project_dir,
    )
    db_path: Path = project_dir / "python_hooks_run_project.duckdb"

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)
    for query, expected_rows in test_case.expected_query_results:
        assert query_duckdb(db_path=db_path, sql=query) == list(expected_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run materializes tables and views with correct data",
            expected_exit_code=0,
            expected_table_names=("daily_revenue", "dim_customers", "fact_orders"),
            expected_view_names=("stg_customers", "stg_orders", "stg_payments"),
            expected_fact_orders_data=(
                (1, 1, "Classic Belgian", "completed"),
                (2, 1, "Cheddar Herb", "completed"),
                (3, 2, "Chicken and Waffle", "completed"),
                (4, 3, "Liege", "completed"),
                (5, 4, "Classic Belgian", "completed"),
                (6, 4, "Brussels", "completed"),
                (7, 5, "Everything Bagel", "cancelled"),
                (8, 1, "Liege", "completed"),
                (9, 2, "Chicken and Waffle", "preparing"),
                (10, 3, "Classic Belgian", "placed"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_project_when_running_run_then_warehouse_state_matches_expected(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    db_path: Path = project_dir / "waffle_shop.duckdb"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    run_sql_path: Path = project_dir / "target" / "run" / "models" / "marts" / "fact_orders.sql"
    assert run_sql_path.exists()
    run_sql: str = run_sql_path.read_text(encoding="utf-8")
    assert "CREATE OR REPLACE TABLE main.fact_orders__staging AS" in run_sql
    assert "ALTER TABLE main.fact_orders__staging RENAME TO fact_orders;" in run_sql

    table_name: str
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name), (
            f"table {table_name} should exist"
        )

    view_name: str
    for view_name in test_case.expected_view_names:
        assert table_exists(db_path=db_path, table_name=view_name), f"view {view_name} should exist"

    fact_sql: str = (
        "SELECT order_id, customer_id, waffle_name, order_status "
        "FROM main.fact_orders ORDER BY order_id"
    )
    fact_rows: list[tuple[Any, ...]] = query_duckdb(db_path=db_path, sql=fact_sql)
    assert tuple(tuple(r) for r in fact_rows) == test_case.expected_fact_orders_data


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="generated factory nodes participate in run build and check lifecycle",
            expected_exit_code=0,
            expected_table_names=("fact_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_factory_generated_nodes_when_running_commands_then_lifecycle_succeeds(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="factory_nodes_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "factory_nodes_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "factory_nodes_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "python/factories/generated.py": """
from pathlib import Path
from sqlbuild.assets import asset
from sqlbuild.checks import check
from sqlbuild.factories import factory
from sqlbuild.loaders import loader
from sqlbuild.tasks import task


PROJECT_DIR = Path(__file__).parents[2]


@factory
def generated_pipeline():
    @task(name="prepare_orders", tags=("factory", "runtime"))
    def prepare(ctx):
        '''Test task prepare.'''
        PROJECT_DIR.joinpath("prepare_marker.txt").write_text("ran", encoding="utf-8")
        return ctx.result(payload={"rows": 1})

    @loader(name="raw_orders", depends_on=[prepare])
    def load(ctx):
        '''Test loader load.'''
        PROJECT_DIR.joinpath("loader_marker.txt").write_text("ran", encoding="utf-8")
        return [{"order_id": 1}]

    @asset(name="orders_export", depends_on=prepare, tags=("factory", "runtime"))
    def export(ctx):
        '''Test asset export.'''
        PROJECT_DIR.joinpath("asset_marker.txt").write_text("ran", encoding="utf-8")
        return ctx.result(materialized=True)

    @check(name="orders_export_check", depends_on=export, tags=("factory", "quality"))
    def export_check(ctx):
        '''Test check export_check.'''
        return ctx.pass_(message="generated export exists")

    return [prepare, load, export, export_check]
""",
        },
    )
    db_path: Path = project_dir / "factory_nodes_project.duckdb"

    run_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "+asset:orders_export",
        ),
        project_dir=project_dir,
    )
    bare_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "prepare_orders"),
        project_dir=project_dir,
    )
    tag_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "tag:runtime"),
        project_dir=project_dir,
    )
    dag_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "dag", "--json"),
        project_dir=project_dir,
    )
    compile_dag_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--dag", "target/factory_dag.json"),
        project_dir=project_dir,
    )
    check_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "check", "--select", "check:orders_export_check"),
        project_dir=project_dir,
    )
    no_python_build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-python", "--select", "+fact_orders"),
        project_dir=project_dir,
    )

    assert run_result.returncode == test_case.expected_exit_code, (
        run_result.stdout + run_result.stderr
    )
    assert "prepare_orders" in run_result.stdout
    assert "orders_export" in run_result.stdout
    assert "Python checks" in run_result.stdout
    assert bare_result.returncode == test_case.expected_exit_code, (
        bare_result.stdout + bare_result.stderr
    )
    assert "prepare_orders" in bare_result.stdout
    assert tag_result.returncode == test_case.expected_exit_code, (
        tag_result.stdout + tag_result.stderr
    )
    assert "orders_export" in tag_result.stdout
    assert dag_result.returncode == test_case.expected_exit_code, (
        dag_result.stdout + dag_result.stderr
    )
    dag_payload: dict[str, object] = json.loads(dag_result.stdout)
    dag_node_ids: set[str] = {str(node["id"]) for node in dag_payload["nodes"]}
    assert {"task:prepare_orders", "asset:orders_export", "source:raw_orders"} <= dag_node_ids
    assert "factory:generated_pipeline" not in dag_node_ids
    assert compile_dag_result.returncode == test_case.expected_exit_code, (
        compile_dag_result.stdout + compile_dag_result.stderr
    )
    compiled_dag_payload: dict[str, object] = json.loads(
        project_dir.joinpath("target/factory_dag.json").read_text(encoding="utf-8")
    )
    compiled_node_ids: set[str] = {str(node["id"]) for node in compiled_dag_payload["nodes"]}
    assert "asset:orders_export" in compiled_node_ids
    assert check_result.returncode == test_case.expected_exit_code, (
        check_result.stdout + check_result.stderr
    )
    assert "orders_export_check" in check_result.stdout
    assert no_python_build_result.returncode == test_case.expected_exit_code, (
        no_python_build_result.stdout + no_python_build_result.stderr
    )
    assert "source    raw_orders" in no_python_build_result.stdout
    assert "asset    orders_export" not in no_python_build_result.stdout
    assert project_dir.joinpath("prepare_marker.txt").exists()
    assert project_dir.joinpath("loader_marker.txt").exists()
    assert project_dir.joinpath("asset_marker.txt").exists()
    table_name: str
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run reuses existing intermediate target for source-only selection",
            expected_exit_code=0,
            expected_table_names=("raw_events",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_intermediate_target_when_running_source_only_then_reuses_target(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="run_existing_intermediate_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "run_existing_intermediate_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "run_existing_intermediate_project.duckdb"\n'
            ),
            "python/loaders/raw.py": (
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
            "sources/raw.yml": "sources:\n  - name: raw_events\n    description: Test source raw_events.\n    managed: true\n",
        },
    )
    setup_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "load", "--select", "fetch_events"),
        project_dir=project_dir,
    )
    assert setup_result.returncode == 0, setup_result.stdout + setup_result.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "raw_events"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "loader    fetch_events" not in result.stdout
    assert "source    raw_events" in result.stdout
    db_path: Path = project_dir / "run_existing_intermediate_project.duckdb"
    table_name: str
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes selected task node",
            expected_exit_code=0,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_selector_when_running_run_then_task_executes(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_run_project.duckdb"\n'
            ),
            "python/tasks/orders.py": (
                "from pathlib import Path\n"
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def prepare_orders(ctx):\n"
                "    '''Test task prepare_orders.'''\n    Path(__file__).parents[2].joinpath('task_marker.txt').write_text('ran')\n"
                "    return ctx.result(payload={'status': 'ok'})\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "task:prepare_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "python    task      prepare_orders" in result.stdout
    assert "OK" in result.stdout
    assert (project_dir / "task_marker.txt").read_text(encoding="utf-8") == "ran"


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes selected asset node",
            expected_exit_code=0,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_asset_selector_when_running_run_then_asset_executes(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_asset_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_asset_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_asset_run_project.duckdb"\n'
            ),
            "python/assets/orders.py": (
                "from pathlib import Path\n"
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def prepared_orders(ctx):\n"
                "    '''Test asset prepared_orders.'''\n    Path(__file__).parents[2].joinpath('asset_marker.txt').write_text('ran')\n"
                "    return ctx.result(payload={'status': 'ok'}, materialized=True)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "asset:prepared_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "python    asset     prepared_orders" in result.stdout
    assert "OK" in result.stdout
    assert (project_dir / "asset_marker.txt").read_text(encoding="utf-8") == "ran"


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run json output includes selected task node",
            expected_exit_code=0,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_selector_with_json_output_when_running_run_then_json_includes_task(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    json_output_path: Path = tmp_path / "target" / "run.json"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_run_json_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_run_json_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_run_json_project.duckdb"\n'
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def prepare_orders(ctx):\n"
                "    '''Test task prepare_orders.'''\n    return ctx.result(metadata={'rows': 3})\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--json-output",
            str(json_output_path),
            "--select",
            "task:prepare_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(json_output_path.read_text(encoding="utf-8"))
    assets: list[dict[str, object]] = list(payload["assets"])  # type: ignore[arg-type]
    assert {
        "kind": "task",
        "name": "prepare_orders",
        "status": "success",
        "metadata": {"rows": 3},
    } in assets
    assert payload["summary"] == {
        "success_count": 1,
        "failure_count": 0,
        "timeout_count": 0,
        "skipped_count": 0,
        "warning_count": 0,
        "python_check_pass_count": 0,
        "python_check_warn_count": 0,
        "python_check_fail_count": 0,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run fails when selected task fails",
            expected_exit_code=1,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failing_task_selector_when_running_run_then_command_fails(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    json_output_path: Path = tmp_path / "target" / "run.json"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_failed_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_failed_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_failed_run_project.duckdb"\n'
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def fail_orders(ctx):\n"
                "    '''Test task fail_orders.'''\n    raise RuntimeError('API unavailable')\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--json-output",
            str(json_output_path),
            "--select",
            "task:fail_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    combined_output: str = result.stdout + result.stderr
    assert "python    task      fail_orders" in combined_output
    assert "FAIL" in combined_output
    assert "\u2717 Completed with errors" in combined_output
    assert "Python node failures:" in combined_output
    payload: dict[str, object] = json.loads(json_output_path.read_text(encoding="utf-8"))
    assert payload["status"] == "failed"
    assert payload["summary"] == {
        "success_count": 0,
        "failure_count": 1,
        "timeout_count": 0,
        "skipped_count": 0,
        "warning_count": 0,
        "python_check_pass_count": 0,
        "python_check_warn_count": 0,
        "python_check_fail_count": 0,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes upstream task before source loader and model",
            expected_exit_code=0,
            expected_table_names=("fact_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_loader_source_model_chain_when_running_model_then_task_runs_before_loader(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_loader_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_loader_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_loader_run_project.duckdb"\n'
            ),
            "python/tasks/orders.py": (
                "from pathlib import Path\n"
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def prepare_orders(ctx):\n"
                "    '''Test task prepare_orders.'''\n    Path(__file__).parents[2].joinpath('orders_ready.txt').write_text('ready')\n"
                "    return ctx.result(metadata={'prepared': True})\n"
            ),
            "python/loaders/orders.py": (
                "from pathlib import Path\n"
                "from sqlbuild.loaders import loader\n"
                "from python.tasks.orders import prepare_orders\n\n"
                "@loader(depends_on=(prepare_orders,))\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    marker = Path(__file__).parents[2].joinpath('orders_ready.txt')\n"
                "    if not marker.exists():\n"
                "        raise RuntimeError('orders were not prepared')\n"
                "    return [{'order_id': 1, 'amount_cents': 100}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: amount_cents\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                "MODEL (description 'Test model fact_orders.', materialized table);\n\n"
                'SELECT order_id, amount_cents FROM __source("raw_orders")\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "+fact_orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "python    task      prepare_orders" in result.stdout
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("prepare_orders") < execution_output.index("fact_orders")
    db_path: Path = project_dir / "python_loader_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)
    assert row_count(db_path=db_path, table_name="fact_orders") == 1


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes selected task after selected model for read-only SQL access",
            expected_exit_code=0,
            expected_table_names=("fact_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_and_task_selector_when_running_run_then_task_can_read_built_model(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_model_task_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_model_task_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_model_task_run_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id, 100 AS amount_cents\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: amount_cents\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                "MODEL (description 'Test model fact_orders.', materialized table);\n\n"
                'SELECT order_id, amount_cents FROM __source("raw_orders")\n'
            ),
            "python/tasks/orders.py": (
                "from pathlib import Path\n"
                "from sqlbuild.refs import model\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=model('fact_orders'))\n"
                "def summarize_orders(ctx):\n"
                "    '''Test task summarize_orders.'''\n    orders = ctx.relation(model('fact_orders'))\n"
                "    rows = ctx.query(f'SELECT COUNT(*) FROM {orders}').fetchall()[0][0]\n"
                "    Path(__file__).parents[2].joinpath('summary.txt').write_text(str(rows))\n"
                "    return ctx.result(metadata={'rows': rows})\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "fact_orders summarize_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("fact_orders") < execution_output.index("summarize_orders")
    assert (project_dir / "summary.txt").read_text(encoding="utf-8") == "1"
    db_path: Path = project_dir / "python_model_task_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes selected asset after selected terminal model",
            expected_exit_code=0,
            expected_table_names=("fact_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_asset_depends_on_terminal_model_when_running_run_then_asset_reads_model(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_model_asset_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_model_asset_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_model_asset_run_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "python/assets/orders.py": (
                "from pathlib import Path\n"
                "from sqlbuild.assets import asset\n"
                "from sqlbuild.refs import model\n\n"
                "@asset(depends_on=model('fact_orders'))\n"
                "def export_fact_orders(ctx):\n"
                "    '''Test asset export_fact_orders.'''\n    orders = ctx.relation(model('fact_orders'))\n"
                "    rows = ctx.query(f'SELECT COUNT(*) FROM {orders}').fetchall()[0][0]\n"
                "    Path(__file__).parents[2].joinpath('export.txt').write_text(str(rows))\n"
                "    return ctx.result(metadata={'rows': rows}, materialized=True)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "fact_orders export_fact_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("fact_orders") < execution_output.index("export_fact_orders")
    assert (project_dir / "export.txt").read_text(encoding="utf-8") == "1"
    db_path: Path = project_dir / "python_model_asset_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run expands and executes task asset task chain",
            expected_exit_code=0,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_asset_task_chain_when_running_final_task_then_chain_executes_in_order(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_task_asset_task_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_task_asset_task_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_task_asset_task_run_project.duckdb"\n'
            ),
            "python/tasks/fetch_orders.py": (
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def fetch_orders(ctx):\n"
                "    '''Test task fetch_orders.'''\n    return ctx.result(payload={'rows': 1})\n"
            ),
            "python/tasks/notify_orders.py": (
                "from pathlib import Path\n"
                "from python.assets.orders import publish_orders\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=publish_orders)\n"
                "def notify_orders(ctx):\n"
                "    '''Test task notify_orders.'''\n    metadata = ctx.result_of(node_function=publish_orders).metadata\n"
                "    output = Path(__file__).parents[2].joinpath('notify.txt')\n"
                "    output.write_text(str(metadata['published']))\n"
                "    return ctx.result()\n"
            ),
            "python/assets/orders.py": (
                "from sqlbuild.assets import asset\n"
                "from python.tasks.fetch_orders import fetch_orders\n\n"
                "@asset(depends_on=fetch_orders)\n"
                "def publish_orders(ctx):\n"
                "    '''Test asset publish_orders.'''\n    payload = ctx.result_of(node_function=fetch_orders).payload\n"
                "    return ctx.result(payload=payload, metadata={'published': True})\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "+notify_orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert result.stdout.index("fetch_orders") < result.stdout.index("publish_orders")
    assert result.stdout.index("publish_orders") < result.stdout.index("notify_orders")
    assert (project_dir / "notify.txt").read_text(encoding="utf-8") == "True"


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes source task asset chain after source load",
            expected_exit_code=0,
            expected_table_names=("raw_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_task_asset_selection_when_running_run_then_task_reads_loaded_source(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_source_task_asset_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_source_task_asset_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_source_task_asset_run_project.duckdb"\n'
            ),
            "python/loaders/orders.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 1, 'amount_cents': 100}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: amount_cents\n"
                "        type: INTEGER\n"
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.refs import source\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=source('raw_orders'))\n"
                "def summarize_loaded_orders(ctx):\n"
                "    '''Test task summarize_loaded_orders.'''\n    orders = ctx.relation(source('raw_orders'))\n"
                "    rows = ctx.query(f'SELECT COUNT(*) FROM {orders}').fetchall()[0][0]\n"
                "    return ctx.result(payload={'rows': rows}, metadata={'rows': rows})\n"
            ),
            "python/assets/orders.py": (
                "from sqlbuild.assets import asset\n"
                "from python.tasks.orders import summarize_loaded_orders\n\n"
                "@asset(depends_on=summarize_loaded_orders)\n"
                "def publish_loaded_orders(ctx):\n"
                "    '''Test asset publish_loaded_orders.'''\n    payload = ctx.result_of(node_function=summarize_loaded_orders).payload\n"
                "    return ctx.result(payload=payload, materialized=False)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "+raw_orders summarize_loaded_orders publish_loaded_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("raw_orders") < execution_output.index("summarize_loaded_orders")
    assert execution_output.index("summarize_loaded_orders") < execution_output.index(
        "publish_loaded_orders"
    )
    db_path: Path = project_dir / "python_source_task_asset_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run json includes skipped task and asset materialization fields",
            expected_exit_code=0,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_skip_and_asset_selection_with_json_when_running_run_then_json_records_fields(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    json_output_path: Path = tmp_path / "target" / "run.json"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_json_fields_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_json_fields_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_json_fields_run_project.duckdb"\n'
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def optional_orders(ctx):\n"
                "    '''Test task optional_orders.'''\n    return ctx.skip(reason='no files')\n\n"
            ),
            "python/assets/orders.py": (
                "from sqlbuild.assets import asset\n\n"
                "@asset\n"
                "def observed_orders(ctx):\n"
                "    '''Test asset observed_orders.'''\n    return ctx.result(metadata={'uri': 's3://orders'}, materialized=False)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--json-output",
            str(json_output_path),
            "--select",
            "optional_orders observed_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(json_output_path.read_text(encoding="utf-8"))
    assets: dict[str, dict[str, object]] = {
        str(asset["name"]): asset
        for asset in payload["assets"]  # type: ignore[index]
    }
    assert assets["optional_orders"]["status"] == "skipped"
    assert assets["optional_orders"]["skip_reason"] == "no files"
    assert assets["observed_orders"]["kind"] == "asset"
    assert assets["observed_orders"]["materialized"] is False
    assert assets["observed_orders"]["metadata"] == {"uri": "s3://orders"}
    assert payload["summary"] == {
        "success_count": 1,
        "failure_count": 0,
        "timeout_count": 0,
        "skipped_count": 1,
        "warning_count": 0,
        "python_check_pass_count": 0,
        "python_check_warn_count": 0,
        "python_check_fail_count": 0,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes independent Python branches and SQL branch",
            expected_exit_code=0,
            expected_table_names=("fact_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_independent_python_and_sql_selectors_when_running_run_then_all_branches_run(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_independent_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_independent_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_independent_run_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT order_id FROM __source("raw_orders")\n'
            ),
            "python/tasks/branches.py": (
                "from pathlib import Path\n"
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def branch_a(ctx):\n"
                "    '''Test task branch_a.'''\n    Path(__file__).parents[2].joinpath('branch_a.txt').write_text('a')\n"
                "    return ctx.result()\n\n"
                "@task\n"
                "def branch_b(ctx):\n"
                "    '''Test task branch_b.'''\n    Path(__file__).parents[2].joinpath('branch_b.txt').write_text('b')\n"
                "    return ctx.result()\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "fact_orders branch_a branch_b",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert (project_dir / "branch_a.txt").read_text(encoding="utf-8") == "a"
    assert (project_dir / "branch_b.txt").read_text(encoding="utf-8") == "b"
    db_path: Path = project_dir / "python_independent_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes SQL-ready Python before downstream SQL",
            expected_exit_code=0,
            expected_table_names=("stg_orders", "fact_orders"),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_depends_on_model_when_running_run_then_task_runs_before_downstream_sql(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_read_side_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_read_side_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_read_side_run_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id, 100 AS amount_cents\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: amount_cents\n"
                "        type: INTEGER\n"
            ),
            "models/stg_orders.sql": (
                'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __ref("stg_orders")\n'
            ),
            "python/tasks/profile.py": (
                "from sqlbuild.refs import model\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=model('stg_orders'))\n"
                "def profile_stg_orders(ctx):\n"
                "    '''Test task profile_stg_orders.'''\n    orders = ctx.relation(model('stg_orders'))\n"
                "    rows = ctx.query(f'SELECT COUNT(*) FROM {orders}').fetchall()[0][0]\n"
                "    return ctx.result(payload={'rows': rows})\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "stg_orders fact_orders profile_stg_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("stg_orders") < execution_output.index("profile_stg_orders")
    assert execution_output.index("profile_stg_orders") < execution_output.index("fact_orders")
    db_path: Path = project_dir / "python_read_side_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes task after typed source dependency",
            expected_exit_code=0,
            expected_table_names=("raw_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_depends_on_source_when_running_run_then_task_runs_after_source_load(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_read_side_source_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_read_side_source_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_read_side_source_run_project.duckdb"\n'
            ),
            "python/loaders/orders.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 1, 'amount_cents': 100}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
                "      - name: amount_cents\n"
                "        type: INTEGER\n"
            ),
            "python/tasks/profile.py": (
                "from sqlbuild.refs import source\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=source('raw_orders'))\n"
                "def profile_raw_orders(ctx):\n"
                "    '''Test task profile_raw_orders.'''\n    orders = ctx.relation(source('raw_orders'))\n"
                "    rows = ctx.query(f'SELECT COUNT(*) FROM {orders}').fetchall()[0][0]\n"
                "    return ctx.result(payload={'rows': rows})\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "+raw_orders profile_raw_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("raw_orders") < execution_output.index("profile_raw_orders")
    db_path: Path = project_dir / "python_read_side_source_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run fails when SQL-ready Python task fails",
            expected_exit_code=1,
            expected_table_names=("stg_orders", "fact_orders"),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_ready_task_fails_when_running_run_then_footer_json_and_exit_fail(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    json_output_path: Path = tmp_path / "target" / "run.json"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_read_side_failure_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_read_side_failure_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_read_side_failure_run_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/stg_orders.sql": (
                'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __ref("stg_orders")\n'
            ),
            "python/tasks/profile.py": (
                "from sqlbuild.refs import model\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=model('stg_orders'))\n"
                "def fail_after_stg_orders(ctx):\n"
                "    '''Test task fail_after_stg_orders.'''\n    raise RuntimeError('profile failed')\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--json-output",
            str(json_output_path),
            "--select",
            "stg_orders fact_orders fail_after_stg_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    combined_output: str = result.stdout + result.stderr
    assert "python    task      fail_after_stg_orders" in combined_output
    assert "FAIL" in combined_output
    assert "Python node failures:" in combined_output
    payload: dict[str, object] = json.loads(json_output_path.read_text(encoding="utf-8"))
    assert payload["status"] == "failed"
    assert payload["summary"]["failure_count"] == 1  # type: ignore[index]
    assets: dict[str, dict[str, object]] = {
        str(asset["name"]): asset
        for asset in payload["assets"]  # type: ignore[index]
    }
    assert assets["fail_after_stg_orders"]["status"] == "failed"
    assert "profile failed" in str(assets["fail_after_stg_orders"]["error_message"])


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run rejects Python dependency on terminal loader",
            expected_exit_code=1,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_depends_on_terminal_loader_when_running_run_then_command_rejects_boundary(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_boundary_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_boundary_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_boundary_run_project.duckdb"\n'
            ),
            "python/loaders/orders.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    return [{'order_id': 1}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "python/tasks/orders.py": (
                "from python.loaders.orders import raw_orders\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=raw_orders)\n"
                "def summarize_orders(ctx):\n"
                "    '''Test task summarize_orders.'''\n    return ctx.result()\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "summarize_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    combined_output: str = result.stdout + result.stderr
    assert "depends on terminal loader 'raw_orders'" in combined_output
    assert "depend on source 'raw_orders' instead" in combined_output


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes intermediate loader before dependent task and asset",
            expected_exit_code=0,
            expected_table_names=("stage_orders",),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_asset_depend_on_intermediate_loader_when_running_run_then_loader_runs_first(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_intermediate_loader_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_intermediate_loader_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_intermediate_loader_run_project.duckdb"\n'
            ),
            "python/loaders/orders.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    destination='stage_orders',\n"
                "    write_strategy='table',\n"
                "    columns=[\n"
                "        {'name': 'order_id', 'type': 'INTEGER'},\n"
                "        {'name': 'amount_cents', 'type': 'INTEGER'},\n"
                "    ],\n"
                ")\n"
                "def stage_orders(ctx):\n"
                "    '''Test loader stage_orders.'''\n    return [{'order_id': 1, 'amount_cents': 100}]\n"
            ),
            "python/tasks/orders.py": (
                "from python.loaders.orders import stage_orders\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=stage_orders)\n"
                "def summarize_stage_orders(ctx):\n"
                "    '''Test task summarize_stage_orders.'''\n    rows = ctx.query('SELECT COUNT(*) FROM stage_orders').fetchall()[0][0]\n"
                "    return ctx.result(payload={'rows': rows}, metadata={'rows': rows})\n"
            ),
            "python/assets/orders.py": (
                "from sqlbuild.assets import asset\n"
                "from python.tasks.orders import summarize_stage_orders\n\n"
                "@asset(depends_on=summarize_stage_orders)\n"
                "def publish_stage_orders(ctx):\n"
                "    '''Test asset publish_stage_orders.'''\n    payload = ctx.result_of(node_function=summarize_stage_orders).payload\n"
                "    return ctx.result(payload=payload, materialized=True)\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "+publish_stage_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("stage_orders") < execution_output.index("summarize_stage_orders")
    assert execution_output.index("summarize_stage_orders") < execution_output.index(
        "publish_stage_orders"
    )
    db_path: Path = project_dir / "python_intermediate_loader_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes loader task loader chain before model",
            expected_exit_code=0,
            expected_table_names=("window_orders", "raw_orders", "fact_orders"),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_task_loader_chain_when_running_model_then_ingress_orders_chain(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_loader_task_loader_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_loader_task_loader_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_loader_task_loader_run_project.duckdb"\n'
            ),
            "python/loaders/window.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    destination='window_orders',\n"
                "    write_strategy='table',\n"
                "    columns=[{'name': 'order_id', 'type': 'INTEGER'}],\n"
                ")\n"
                "def load_window_orders(ctx):\n"
                "    '''Test loader load_window_orders.'''\n    return [{'order_id': 1}]\n"
            ),
            "python/tasks/orders.py": (
                "from pathlib import Path\n"
                "from python.loaders.window import load_window_orders\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=load_window_orders)\n"
                "def prepare_raw_orders(ctx):\n"
                "    '''Test task prepare_raw_orders.'''\n    rows = ctx.query('SELECT COUNT(*) FROM window_orders').fetchall()[0][0]\n"
                "    Path(__file__).parents[2].joinpath('prepared.txt').write_text(str(rows))\n"
                "    return ctx.result(metadata={'rows': rows})\n"
            ),
            "python/loaders/raw.py": (
                "from pathlib import Path\n"
                "from python.tasks.orders import prepare_raw_orders\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(depends_on=(prepare_raw_orders,))\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    marker = Path(__file__).parents[2].joinpath('prepared.txt')\n"
                "    if marker.read_text() != '1':\n"
                "        raise RuntimeError('window orders were not prepared')\n"
                "    return [{'order_id': 1}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "+fact_orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("window_orders") < execution_output.index("prepare_raw_orders")
    assert execution_output.index("raw_orders") < execution_output.index("fact_orders")
    db_path: Path = project_dir / "python_loader_task_loader_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes task asset loader chain before model",
            expected_exit_code=0,
            expected_table_names=("raw_orders", "fact_orders"),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_asset_loader_chain_when_running_model_then_ingress_orders_chain(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_task_asset_loader_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_task_asset_loader_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_task_asset_loader_run_project.duckdb"\n'
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def fetch_orders(ctx):\n"
                "    '''Test task fetch_orders.'''\n    return ctx.result(payload={'order_id': 1})\n"
            ),
            "python/assets/orders.py": (
                "from pathlib import Path\n"
                "from python.tasks.orders import fetch_orders\n"
                "from sqlbuild.assets import asset\n\n"
                "@asset(depends_on=fetch_orders)\n"
                "def publish_orders(ctx):\n"
                "    '''Test asset publish_orders.'''\n    payload = ctx.result_of(node_function=fetch_orders).payload\n"
                "    marker = Path(__file__).parents[2].joinpath('asset_ready.txt')\n"
                "    marker.write_text(str(payload['order_id']))\n"
                "    return ctx.result(payload=payload, materialized=True)\n"
            ),
            "python/loaders/orders.py": (
                "from pathlib import Path\n"
                "from python.assets.orders import publish_orders\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(depends_on=(publish_orders,))\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    marker = Path(__file__).parents[2].joinpath('asset_ready.txt')\n"
                "    if marker.read_text() != '1':\n"
                "        raise RuntimeError('asset was not ready')\n"
                "    return [{'order_id': 1}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "+fact_orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("fetch_orders") < execution_output.index("publish_orders")
    assert execution_output.index("raw_orders") < execution_output.index("fact_orders")
    db_path: Path = project_dir / "python_task_asset_loader_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes loader asset loader chain before model",
            expected_exit_code=0,
            expected_table_names=("window_orders", "raw_orders", "fact_orders"),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_asset_loader_chain_when_running_model_then_ingress_orders_chain(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_loader_asset_loader_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_loader_asset_loader_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_loader_asset_loader_run_project.duckdb"\n'
            ),
            "python/loaders/window.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    destination='window_orders',\n"
                "    write_strategy='table',\n"
                "    columns=[{'name': 'order_id', 'type': 'INTEGER'}],\n"
                ")\n"
                "def load_window_orders(ctx):\n"
                "    '''Test loader load_window_orders.'''\n    return [{'order_id': 1}]\n"
            ),
            "python/assets/orders.py": (
                "from pathlib import Path\n"
                "from python.loaders.window import load_window_orders\n"
                "from sqlbuild.assets import asset\n\n"
                "@asset(depends_on=load_window_orders)\n"
                "def prepare_asset_orders(ctx):\n"
                "    '''Test asset prepare_asset_orders.'''\n    rows = ctx.query('SELECT COUNT(*) FROM window_orders').fetchall()[0][0]\n"
                "    Path(__file__).parents[2].joinpath('asset_ready.txt').write_text(str(rows))\n"
                "    return ctx.result(metadata={'rows': rows}, materialized=True)\n"
            ),
            "python/loaders/raw.py": (
                "from pathlib import Path\n"
                "from python.assets.orders import prepare_asset_orders\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(depends_on=(prepare_asset_orders,))\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    marker = Path(__file__).parents[2].joinpath('asset_ready.txt')\n"
                "    if marker.read_text() != '1':\n"
                "        raise RuntimeError('asset orders were not prepared')\n"
                "    return [{'order_id': 1}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "+fact_orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("window_orders") < execution_output.index("prepare_asset_orders")
    assert execution_output.index("raw_orders") < execution_output.index("fact_orders")
    db_path: Path = project_dir / "python_loader_asset_loader_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes loader loader chain before model",
            expected_exit_code=0,
            expected_table_names=("window_orders", "raw_orders", "fact_orders"),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_loader_chain_when_running_model_then_ingress_orders_chain(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_loader_loader_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_loader_loader_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_loader_loader_run_project.duckdb"\n'
            ),
            "python/loaders/window.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    destination='window_orders',\n"
                "    write_strategy='table',\n"
                "    columns=[{'name': 'order_id', 'type': 'INTEGER'}],\n"
                ")\n"
                "def load_window_orders(ctx):\n"
                "    '''Test loader load_window_orders.'''\n    return [{'order_id': 1}]\n"
            ),
            "python/loaders/raw.py": (
                "from python.loaders.window import load_window_orders\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(depends_on=(load_window_orders,))\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    rows = ctx.query('SELECT COUNT(*) FROM window_orders').fetchall()[0][0]\n"
                "    return [{'order_id': rows}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "+fact_orders"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("window_orders") < execution_output.index("raw_orders")
    assert execution_output.index("raw_orders") < execution_output.index("fact_orders")
    db_path: Path = project_dir / "python_loader_loader_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run executes full Python SQL Python spine in lifecycle order",
            expected_exit_code=0,
            expected_table_names=("window_orders", "raw_orders", "fact_orders"),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_task_asset_loader_model_task_asset_task_spine_when_running_run_then_orders(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_sql_python_spine_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_sql_python_spine_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_sql_python_spine_run_project.duckdb"\n'
            ),
            "python/loaders/window.py": (
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    destination='window_orders',\n"
                "    write_strategy='table',\n"
                "    columns=[{'name': 'order_id', 'type': 'INTEGER'}],\n"
                ")\n"
                "def load_window_orders(ctx):\n"
                "    '''Test loader load_window_orders.'''\n    return [{'order_id': 7}]\n"
            ),
            "python/tasks/prepare.py": (
                "from python.loaders.window import load_window_orders\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=load_window_orders)\n"
                "def prepare_orders(ctx):\n"
                "    '''Test task prepare_orders.'''\n    rows = ctx.query('SELECT order_id FROM window_orders').fetchall()\n"
                "    return ctx.result(payload={'order_id': rows[0][0]})\n"
            ),
            "python/assets/prepare.py": (
                "from pathlib import Path\n"
                "from python.tasks.prepare import prepare_orders\n"
                "from sqlbuild.assets import asset\n\n"
                "@asset(depends_on=prepare_orders)\n"
                "def publish_prepared_orders(ctx):\n"
                "    '''Test asset publish_prepared_orders.'''\n    payload = ctx.result_of(node_function=prepare_orders).payload\n"
                "    marker = Path(__file__).parents[2].joinpath('prepared_order_id.txt')\n"
                "    marker.write_text(str(payload['order_id']))\n"
                "    return ctx.result(payload=payload, materialized=True)\n"
            ),
            "python/loaders/raw.py": (
                "from pathlib import Path\n"
                "from python.assets.prepare import publish_prepared_orders\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(depends_on=(publish_prepared_orders,))\n"
                "def raw_orders(ctx):\n"
                "    '''Test loader raw_orders.'''\n    marker = Path(__file__).parents[2].joinpath('prepared_order_id.txt')\n"
                "    return [{'order_id': int(marker.read_text())}]\n"
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    managed: true\n"
                "    write_strategy: table\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/fact_orders.sql": (
                'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "python/tasks/profile.py": (
                "from sqlbuild.refs import model\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=model('fact_orders'))\n"
                "def profile_fact_orders(ctx):\n"
                "    '''Test task profile_fact_orders.'''\n    orders = ctx.relation(model('fact_orders'))\n"
                "    order_id = ctx.query(f'SELECT order_id FROM {orders}').fetchall()[0][0]\n"
                "    return ctx.result(payload={'order_id': order_id}, metadata={'rows': 1})\n"
            ),
            "python/assets/export.py": (
                "from python.tasks.profile import profile_fact_orders\n"
                "from sqlbuild.assets import asset\n\n"
                "@asset(depends_on=profile_fact_orders)\n"
                "def export_fact_orders(ctx):\n"
                "    '''Test asset export_fact_orders.'''\n    payload = ctx.result_of(node_function=profile_fact_orders).payload\n"
                "    return ctx.result(payload=payload, metadata={'exported': True})\n"
            ),
            "python/tasks/notify.py": (
                "from pathlib import Path\n"
                "from python.assets.export import export_fact_orders\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=export_fact_orders)\n"
                "def notify_fact_orders(ctx):\n"
                "    '''Test task notify_fact_orders.'''\n    payload = ctx.result_of(node_function=export_fact_orders).payload\n"
                "    output = Path(__file__).parents[2].joinpath('notify.txt')\n"
                "    output.write_text(str(payload['order_id']))\n"
                "    return ctx.result(metadata={'notified': True})\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "+fact_orders +notify_fact_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    execution_output: str = result.stdout[result.stdout.index("Execution  sqb build") :]
    assert execution_output.index("window_orders") < execution_output.index("prepare_orders")
    assert execution_output.index("prepare_orders") < execution_output.index(
        "publish_prepared_orders"
    )
    assert execution_output.index("raw_orders") < execution_output.index("fact_orders")
    assert execution_output.index("fact_orders") < execution_output.index("profile_fact_orders")
    assert execution_output.index("profile_fact_orders") < execution_output.index(
        "export_fact_orders"
    )
    assert execution_output.index("export_fact_orders") < execution_output.index(
        "notify_fact_orders"
    )
    assert (project_dir / "notify.txt").read_text(encoding="utf-8") == "7"
    db_path: Path = project_dir / "python_sql_python_spine_run_project.duckdb"
    for table_name in test_case.expected_table_names:
        assert table_exists(db_path=db_path, table_name=table_name)
    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT order_id FROM fact_orders",
    )
    assert rows == [(7,)]


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run rejects source downstream task feeding loader",
            expected_exit_code=1,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_task_loader_chain_when_running_run_then_command_rejects_boundary(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_source_task_loader_boundary_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_source_task_loader_boundary_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_source_task_loader_boundary_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.refs import source\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=source('raw_orders'))\n"
                "def prepare_orders(ctx):\n"
                "    '''Test task prepare_orders.'''\n    return ctx.result()\n"
            ),
            "python/loaders/orders.py": (
                "from python.tasks.orders import prepare_orders\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    destination='stage_orders',\n"
                "    write_strategy='table',\n"
                "    columns=[{'name': 'order_id', 'type': 'INTEGER'}],\n"
                "    depends_on=(prepare_orders,),\n"
                ")\n"
                "def stage_orders(ctx):\n"
                "    '''Test loader stage_orders.'''\n    return [{'order_id': 1}]\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "loader:stage_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    combined_output: str = result.stdout + result.stderr
    assert "Loader 'stage_orders' depends on Python node 'prepare_orders'" in combined_output
    assert "depends on SQL" in combined_output


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="run rejects model downstream task feeding loader",
            expected_exit_code=1,
            expected_table_names=(),
            expected_view_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_task_loader_chain_when_running_run_then_command_rejects_boundary(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_model_task_loader_boundary_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_model_task_loader_boundary_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_model_task_loader_boundary_project.duckdb"\n'
            ),
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_orders\n    description: Test source raw_orders.\n"
                "    expression: SELECT 1 AS order_id\n"
                "    columns:\n"
                "      - name: order_id\n"
                "        type: INTEGER\n"
            ),
            "models/stg_orders.sql": (
                'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT * FROM __source("raw_orders")\n'
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.refs import model\n"
                "from sqlbuild.tasks import task\n\n"
                "@task(depends_on=model('stg_orders'))\n"
                "def prepare_orders(ctx):\n"
                "    '''Test task prepare_orders.'''\n    return ctx.result()\n"
            ),
            "python/loaders/orders.py": (
                "from python.tasks.orders import prepare_orders\n"
                "from sqlbuild.loaders import loader\n\n"
                "@loader(\n"
                "    destination='stage_orders',\n"
                "    write_strategy='table',\n"
                "    columns=[{'name': 'order_id', 'type': 'INTEGER'}],\n"
                "    depends_on=(prepare_orders,),\n"
                ")\n"
                "def stage_orders(ctx):\n"
                "    '''Test loader stage_orders.'''\n    return [{'order_id': 1}]\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            "loader:stage_orders",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    combined_output: str = result.stdout + result.stderr
    assert "Loader 'stage_orders' depends on Python node 'prepare_orders'" in combined_output
    assert "depends on SQL" in combined_output


@pytest.mark.parametrize(
    "test_case",
    [
        RunE2ETestCase(
            description="build rejects a selected check whose task is not selected",
            expected_exit_code=1,
            expected_table_names=(),
            expected_view_names=(),
            expected_output_fragments=(
                "Python check 'check_orders_export' checks task:prepare_orders, which the "
                "selection does not include; select it too, or narrow the selector so it no "
                "longer matches the check.",
            ),
            select=("check:check_orders_export",),
        ),
        RunE2ETestCase(
            description="build runs a selected check with its selected task",
            expected_exit_code=0,
            expected_table_names=(),
            expected_view_names=(),
            expected_output_fragments=("check     check_orders_export START", "PASS=2"),
            select=("check:check_orders_export", "task:prepare_orders"),
        ),
        RunE2ETestCase(
            description="build rejects a selected check on a source no loader writes",
            expected_exit_code=1,
            expected_table_names=(),
            expected_view_names=(),
            expected_output_fragments=(
                "Python check 'check_raw_events' checks source:raw_events, which a build does "
                "not load, so the check would not run; run it with sqb check, or narrow the "
                "selector so it no longer matches the check.",
            ),
            select=("check:check_raw_events", "source:raw_events"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_check_selector_when_running_build_then_check_needs_its_checked_node(
    test_case: RunE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_check_run_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "python_check_run_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "python_check_run_project.duckdb"\n'
            ),
            "python/checks/orders.py": (
                "from python.tasks.orders import prepare_orders\n"
                "from sqlbuild.checks import check\n\n"
                "@check(depends_on=prepare_orders)\n"
                "def check_orders_export(ctx):\n"
                "    '''Test check check_orders_export.'''\n    return ctx.pass_()\n"
            ),
            "python/tasks/orders.py": (
                "from sqlbuild.tasks import task\n\n"
                "@task\n"
                "def prepare_orders(ctx):\n"
                "    '''Test task prepare_orders.'''\n    return ctx.result()\n"
            ),
            "python/checks/events.py": (
                "from sqlbuild.checks import check\n"
                "from sqlbuild.refs import source\n\n"
                "@check(depends_on=source('raw_events'))\n"
                "def check_raw_events(ctx):\n"
                "    '''Test check check_raw_events.'''\n    return ctx.pass_()\n"
            ),
            "sources/raw.yml": (
                "sources:\n  - name: raw_events\n    description: Test source raw_events.\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--no-tests",
            "--no-audits",
            "--select",
            *test_case.select,
        ),
        project_dir=project_dir,
    )

    combined_output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, combined_output
    for fragment in test_case.expected_output_fragments:
        assert fragment in combined_output
