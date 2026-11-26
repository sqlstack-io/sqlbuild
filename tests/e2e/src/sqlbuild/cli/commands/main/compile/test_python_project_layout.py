"""E2E coverage for fail-closed Python project layout validation."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FactoryModuleNodePlanTestCase,
    PythonLayoutBuildTestCase,
    PythonProjectLayoutCompileTestCase,
    UnrelatedPythonPackageBuildTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import orders_job_project
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_LAYOUT_CONFIG: str = 'name = "python_layout"\nadapter = "duckdb"\n'
_ASSET_MODULE: str = (
    "from sqlbuild.assets import asset\n\n\n"
    "@asset\n"
    "def orders(ctx):\n"
    "    '''Test task orders.'''\n    return None\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        PythonProjectLayoutCompileTestCase(
            description="Python inside the models folder",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "sql/models/measurements.py": "def build_cases(): return []\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): sql/models/measurements.py",
                "Move pipeline Python nodes and the helper modules they import under python/nodes/",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="Python jobs left in orchestration/ point to the TOML layout",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "orchestration/jobs.py": (
                    "from sqlbuild.orchestration import job\n"
                    "nightly = job('nightly', description='Orders.', select='orders')\n"
                ),
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/jobs.py",
                "files cannot sit directly in orchestration/",
                "jobs, job groups and schedules are TOML files in orchestration/jobs/",
                "https://sqlbuild.com/docs/concepts/orchestration/",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="a sensor importing the removed job API points to the TOML layout",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "orchestration/sensors/orders_sensor.py": (
                    "from sqlbuild.orchestration import job, sensor\n"
                    "@sensor(description='Orders.', when=job('nightly', description='Orders.', select='orders'))\n"
                    "def orders_sensor(ctx):\n    return None\n"
                ),
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/sensors/orders_sensor.py:1:",
                "`job` was removed from sqlbuild.orchestration",
                "https://sqlbuild.com/docs/concepts/orchestration/",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="an invalid cron is reported at its value",
            repo_files=orders_job_project(
                'select = "orders"\nschedule = { cron = "not a cron", timezone = "UTC" }\n'
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/jobs/orders_job.toml:4:21:",
                "schedule cron: invalid cron expression",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="an unknown time zone is reported at its value",
            repo_files=orders_job_project(
                'select = "orders"\nschedule = { cron = "0 2 * * *", timezone = "Mars/Olympus" }\n'
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/jobs/orders_job.toml:4:45:",
                "schedule timezone is not an IANA time zone",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="a starts_at that is not RFC 3339 is reported at its value",
            repo_files=orders_job_project(
                'select = "orders"\nschedule = { cron = "0 2 * * *", timezone = "UTC", starts_at = "yesterday" }\n'
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/jobs/orders_job.toml:4:64:",
                "schedule starts_at must be an RFC 3339 timestamp",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="an invalid build setting is reported at its value",
            repo_files=orders_job_project(
                'select = "orders"\n\n[settings]\nconcurrency = "lots"\n'
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/jobs/orders_job.toml:6:15:",
                "concurrency",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="an unknown selector type is reported at the selector",
            repo_files=orders_job_project('select = "orders tagg:finance"\n'),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/jobs/orders_job.toml:3:10:",
                "unknown selector type 'tagg' in 'tagg:finance'",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="an empty intersection part is reported at the selector",
            repo_files=orders_job_project('select = "tag:finance,"\n'),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D017]",
                "orchestration/jobs/orders_job.toml:3:10:",
                "empty selector",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed tasks Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "tasks/orders.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'tasks/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed assets Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "assets/orders.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'assets/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed checks Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "checks/orders.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'checks/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed loaders Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "loaders/orders.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'loaders/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed factories Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "factories/orders.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'factories/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed libs Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "libs/cleaning.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'libs/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed dagster Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "dagster/definitions.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'dagster/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed rivers_pipeline Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "rivers_pipeline/definitions.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'rivers_pipeline/' is not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed root definitions.py Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "definitions.py": "from sqlbuild.assets import asset\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'definitions.py' is not part of a SQLBuild project",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_under_unsupported_root_when_compiling_then_command_fails_with_path_guidance(
    test_case: PythonProjectLayoutCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_layout",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert all(fragment in result.stderr for fragment in test_case.expected_stderr_fragments)


@pytest.mark.parametrize(
    "test_case",
    (
        PythonProjectLayoutCompileTestCase(
            description="custom Rule harness test path",
            repo_files={
                "sqlbuild_project.toml": 'name = "rule_harness_layout"\nadapter = "duckdb"\n',
                "sql/models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n",
                "tests/rules/test_order_policy.py": "def test_order_policy(): pass\n",
            },
            expected_exit_code=0,
            expected_stderr_fragments=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_rule_harness_test_when_compiling_then_python_test_path_is_accepted(
    test_case: PythonProjectLayoutCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="rule_harness_layout",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert "Project compiled  1 model" in result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        PythonProjectLayoutCompileTestCase(
            description="python root with nested nodes and undecorated helpers",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "python_root_layout"\n'
                    'adapter = "duckdb"\n'
                    'default_target = "dev"\n\n'
                    "[connections.developer]\n"
                    'database = "python_root_layout.duckdb"\n\n'
                    "[targets.dev]\n"
                    'connection = "developer"\n'
                    'schema = "main"\n'
                ),
                "sql/sources/raw.yml": (
                    "sources:\n"
                    "  - name: raw_orders\n    description: Test source raw_orders.\n"
                    "    managed: true\n"
                    "    columns:\n"
                    "      - name: order_id\n"
                    "        type: INTEGER\n"
                    "      - name: status\n"
                    "        type: VARCHAR\n"
                ),
                "sql/models/fact_orders.sql": (
                    "MODEL (description 'Test model fact_orders.', materialized table);\n\n"
                    'SELECT order_id, status FROM __source("raw_orders")\n'
                ),
                "python/nodes/_helpers.py": (
                    "def normalize_status(value):\n    return value.strip().lower()\n"
                ),
                "python/nodes/orders/__init__.py": "",
                "python/nodes/orders/utils.py": (
                    "from python.nodes._helpers import normalize_status\n\n\n"
                    "def order_rows():\n"
                    "    return [\n"
                    "        {'order_id': 1, 'status': normalize_status(' Shipped ')},\n"
                    "        {'order_id': 2, 'status': normalize_status('PENDING')},\n"
                    "    ]\n"
                ),
                "python/nodes/loaders/raw_orders/loader.py": (
                    "from python.nodes.orders.utils import order_rows\n"
                    "from sqlbuild.loaders import loader\n\n\n"
                    "@loader(write_strategy='replace')\n"
                    "def raw_orders(ctx):\n"
                    "    '''Test loader raw_orders.'''\n    return order_rows()\n"
                ),
                "python/nodes/orders/exports/asset.py": (
                    "from sqlbuild.assets import asset\n"
                    "from sqlbuild.refs import model\n\n\n"
                    "@asset(depends_on=model('fact_orders'))\n"
                    "def orders_export(ctx):\n"
                    "    '''Test asset orders_export.'''\n    relation = ctx.relation(model('fact_orders'))\n"
                    "    rows = ctx.query(f'SELECT COUNT(*) FROM {relation}').fetchall()\n"
                    "    return ctx.result(payload={'order_count': int(rows[0][0])})\n"
                ),
                "python/nodes/orders/exports/checks.py": (
                    "from python.nodes.orders.exports.asset import orders_export\n"
                    "from sqlbuild.checks import check\n\n\n"
                    "@check(depends_on=orders_export)\n"
                    "def orders_export_not_empty(ctx):\n"
                    "    '''Test check orders_export_not_empty.'''\n    payload = ctx.result_of(node_function=orders_export).payload\n"
                    "    if payload['order_count'] <= 0:\n"
                    "        return ctx.fail(message='orders export is empty')\n"
                    "    return ctx.pass_(message='orders export is ready')\n"
                ),
            },
            expected_exit_code=0,
            expected_stderr_fragments=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_root_project_when_compiling_and_building_then_nodes_run_with_helpers(
    test_case: PythonProjectLayoutCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_root_layout",
        repo_files=test_case.repo_files,
    )

    compile_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    compile_output: str = compile_result.stdout + compile_result.stderr
    build_output: str = build_result.stdout + build_result.stderr
    assert compile_result.returncode == test_case.expected_exit_code, compile_output
    assert build_result.returncode == test_case.expected_exit_code, build_output
    assert all(
        fragment in build_output
        for fragment in (
            "raw_orders",
            "fact_orders",
            "orders_export",
            "orders_export_not_empty",
            "orders export is ready",
        )
    )
    assert "_helpers" not in build_output
    assert "order_rows" not in build_output


@pytest.mark.parametrize(
    "test_case",
    (
        FactoryModuleNodePlanTestCase(
            description="factory returning its module-level task registers one node",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "factory_layout"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
                ),
                "python/nodes/orders/orders/asset.py": (
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.assets import asset\n\n\n"
                    "@asset\n"
                    "def orders(ctx):\n"
                    "    '''Test task orders.'''\n    return None\n\n\n"
                    "@factory\n"
                    "def order_nodes():\n"
                    "    return orders\n"
                ),
            },
            expected_python_node_names=("orders",),
        ),
        FactoryModuleNodePlanTestCase(
            description="factory binding its generated task to a module global registers it",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "factory_layout"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
                ),
                "python/nodes/orders/orders/assets.py": (
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.assets import asset\n\n\n"
                    "@factory\n"
                    "def order_nodes():\n"
                    "    global orders\n\n"
                    "    @asset\n"
                    "    def orders(ctx):\n"
                    "        '''Test task orders.'''\n        return None\n\n"
                    "    return orders\n"
                ),
            },
            expected_python_node_names=("orders",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_factory_returning_module_node_when_planning_then_node_registers_once(
    test_case: FactoryModuleNodePlanTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="factory_layout",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, list[dict[str, object]]] = json.loads(result.stdout)
    assert (
        tuple(entry["name"] for entry in payload["python_nodes"])
        == test_case.expected_python_node_names
    )


@pytest.mark.parametrize(
    "test_case",
    (
        PythonProjectLayoutCompileTestCase(
            description="factory returning a distinct same-name task still conflicts",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "factory_layout"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
                ),
                "python/nodes/orders/orders/asset.py": (
                    "from sqlbuild.assets import asset\n\n\n"
                    "@asset\n"
                    "def orders(ctx):\n"
                    "    '''Test task orders.'''\n    return None\n"
                ),
                "python/nodes/orders/generated/assets.py": (
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.assets import asset\n\n\n"
                    "@factory\n"
                    "def order_nodes():\n"
                    "    @asset(name='orders')\n"
                    "    def generated_orders(ctx):\n"
                    "        '''Test task generated_orders.'''\n        return None\n"
                    "    return generated_orders\n"
                ),
            },
            expected_exit_code=1,
            expected_stderr_fragments=("Duplicate Python node found for 'orders'",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_factory_returning_distinct_same_name_node_when_planning_then_conflict_is_reported(
    test_case: PythonProjectLayoutCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="factory_layout",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert all(fragment in result.stderr for fragment in test_case.expected_stderr_fragments)


@pytest.mark.parametrize(
    "test_case",
    (
        UnrelatedPythonPackageBuildTestCase(
            description="project python helpers win over an unrelated python package on PYTHONPATH",
            unrelated_files={
                "python/__init__.py": "UNRELATED = True\n",
                "python/nodes/helpers.py": "raise RuntimeError('unrelated python package imported')\n",
            },
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "python_collision"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
                ),
                "python/nodes/helpers/__init__.py": "",
                "python/nodes/helpers/values.py": "STATUS = 'shipped'\n",
                "python/nodes/helpers/clean.py": (
                    "from .values import STATUS\n\n\ndef label():\n    return STATUS\n"
                ),
                "python/nodes/orders/orders/asset.py": (
                    "import python.nodes.helpers.clean\n"
                    "from sqlbuild.assets import asset\n\n\n"
                    "@asset\n"
                    "def orders(ctx):\n"
                    "    '''Test task orders.'''\n    return ctx.result(payload={'status': python.nodes.helpers.clean.label()})\n"
                ),
            },
            expected_build_fragments=("orders", "\u2713 Completed successfully"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unrelated_python_package_on_path_when_building_then_project_python_is_used(
    test_case: UnrelatedPythonPackageBuildTestCase,
    tmp_path: Path,
) -> None:
    site_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="site",
        repo_files=test_case.unrelated_files,
    )
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_collision",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
        env={"PYTHONPATH": str(site_dir)},
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    assert all(fragment in output for fragment in test_case.expected_build_fragments)


@pytest.mark.parametrize(
    "test_case",
    (
        PythonProjectLayoutCompileTestCase(
            description="unknown folder inside sql",
            repo_files={"sqlbuild_project.toml": _LAYOUT_CONFIG, "sql/notes/readme.md": ""},
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'sql/notes/' is not part of a SQLBuild project",
                "move it under one of: sql/models/,",
                "or list it in extra_paths in sqlbuild_project.toml",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="unknown module inside python",
            repo_files={"sqlbuild_project.toml": _LAYOUT_CONFIG, "python/helpers.py": ""},
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]",
                "'python/helpers.py' is not part of a SQLBuild project",
                "move it under one of: python/nodes/,",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="several unknown entries are named together",
            repo_files={
                "sqlbuild_project.toml": _LAYOUT_CONFIG,
                "notes.txt": "",
                "models/orders.sql": "",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D018]: 'models/' and 'notes.txt' are not part of a SQLBuild project",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="extra_paths allows listed entries",
            repo_files={
                "sqlbuild_project.toml": 'extra_paths = ["docs/", "sql/notes"]\n' + _LAYOUT_CONFIG,
                "docs/guide.md": "",
                "sql/notes/readme.md": "",
            },
            expected_exit_code=0,
            expected_stderr_fragments=(),
        ),
        PythonProjectLayoutCompileTestCase(
            description="gitignored, hidden and SQLBuild-written entries are allowed",
            repo_files={
                "sqlbuild_project.toml": _LAYOUT_CONFIG,
                ".gitignore": "scratch/\n*.log\n",
                "scratch/notes.md": "",
                "build.log": "",
                ".env": "",
                "README.md": "",
                "warehouse.duckdb": "",
                "logs/run.log": "",
            },
            expected_exit_code=0,
            expected_stderr_fragments=(),
        ),
        PythonProjectLayoutCompileTestCase(
            description="an asset in node.py",
            repo_files={
                "sqlbuild_project.toml": _LAYOUT_CONFIG,
                "python/nodes/orders/export/node.py": _ASSET_MODULE,
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D011]: @asset 'orders' is declared in python/nodes/orders/export/node.py; "
                "an @asset is declared in asset.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="an asset in loader.py",
            repo_files={
                "sqlbuild_project.toml": _LAYOUT_CONFIG,
                "python/nodes/orders/export/loader.py": _ASSET_MODULE,
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D011]: @asset 'orders' is declared in python/nodes/orders/export/loader.py; "
                "an @asset is declared in asset.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="a loaders.py factory returning an asset",
            repo_files={
                "sqlbuild_project.toml": _LAYOUT_CONFIG,
                "python/nodes/orders/export/loaders.py": (
                    "from sqlbuild.assets import asset\n"
                    "from sqlbuild.factories import factory\n\n\n"
                    "@factory\n"
                    "def order_nodes():\n"
                    "    @asset(name='orders')\n"
                    "    def orders(ctx):\n"
                    "        '''Test task orders.'''\n        return None\n"
                    "    return [orders]\n"
                ),
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D011]: @factory 'order_nodes' in python/nodes/orders/export/loaders.py "
                "makes only assets; a factory that makes only assets is declared in assets.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="a factory.py making one kind",
            repo_files={
                "sqlbuild_project.toml": _LAYOUT_CONFIG,
                "python/nodes/orders/export/factory.py": (
                    "from sqlbuild.assets import asset\n"
                    "from sqlbuild.factories import factory\n\n\n"
                    "@factory\n"
                    "def order_nodes():\n"
                    "    @asset(name='orders')\n"
                    "    def orders(ctx):\n"
                    "        '''Test task orders.'''\n        return None\n"
                    "    return [orders]\n"
                ),
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D011]: @factory 'order_nodes' in python/nodes/orders/export/factory.py "
                "makes only assets; a factory that makes only assets is declared in assets.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="kind-named files and checks beside an asset compile",
            repo_files={
                "sqlbuild_project.toml": _LAYOUT_CONFIG,
                "python/nodes/orders/export/asset.py": _ASSET_MODULE,
                "python/nodes/orders/export/checks.py": (
                    "from python.nodes.orders.export.asset import orders\n"
                    "from sqlbuild.checks import check\n\n\n"
                    "@check(depends_on=orders)\n"
                    "def orders_ok(ctx):\n"
                    "    '''Test check orders_ok.'''\n    return ctx.pass_()\n"
                ),
            },
            expected_exit_code=0,
            expected_stderr_fragments=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_project_layout_when_compiling_then_layout_and_node_file_names_are_checked(
    test_case: PythonProjectLayoutCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="python_layout",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert all(fragment in result.stderr for fragment in test_case.expected_stderr_fragments), (
        result.stderr
    )


@pytest.mark.parametrize(
    "test_case",
    (
        PythonLayoutBuildTestCase(
            description="a provider imported by its python.providers path keeps its class identity",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "provider_identity"\nadapter = "duckdb"\n\n'
                    '[connection]\ndatabase = ":memory:"\n'
                ),
                "python/providers/warehouse/provider.py": (
                    "from sqlbuild.providers import Provider\n\n\n"
                    "class Warehouse(Provider):\n"
                    "    '''Test provider Warehouse.'''\n\n"
                    "    region: str = 'north'\n"
                ),
                "python/nodes/utils/regions.py": (
                    "from python.providers.warehouse.provider import Warehouse\n\n\n"
                    "def region_of(warehouse: Warehouse) -> str:\n"
                    "    return warehouse.region\n"
                ),
                "python/nodes/orders/export/asset.py": (
                    "from python.nodes.utils.regions import region_of\n"
                    "from python.providers.warehouse.provider import Warehouse\n"
                    "from sqlbuild.assets import asset\n\n\n"
                    "@asset\n"
                    "def orders_export(ctx, warehouse: Warehouse):\n"
                    "    '''Test task orders_export.'''\n"
                    "    assert isinstance(warehouse, Warehouse)\n"
                    "    return ctx.result(payload={'region': region_of(warehouse)})\n"
                ),
            },
            expected_build_fragments=("orders_export", "✓ Completed successfully"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_provider_imported_by_path_when_building_then_its_type_check_holds(
    test_case: PythonLayoutBuildTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_identity",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    assert all(fragment in output for fragment in test_case.expected_build_fragments)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
