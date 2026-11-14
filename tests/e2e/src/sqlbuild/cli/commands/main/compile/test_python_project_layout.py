"""E2E coverage for fail-closed Python project layout validation."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    FactoryModuleNodePlanTestCase,
    PythonProjectLayoutCompileTestCase,
    UnrelatedPythonPackageBuildTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (
        PythonProjectLayoutCompileTestCase(
            description="unsupported project Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "audit_helpers/measurements.py": "def build_cases(): return []\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): audit_helpers/measurements.py",
                "Move pipeline Python nodes and the helper modules they import under python/",
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
                    "orders = sensor('orders_sensor', description='Orders.', "
                    "run=job('nightly', description='Orders.', select='orders'))\n"
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
        *(
            PythonProjectLayoutCompileTestCase(
                description=description,
                repo_files={
                    "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                    "models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
                    "orchestration/jobs/orders_job.toml": (
                        'name = "orders_job"\ndescription = "Orders."\n' + job_values
                    ),
                },
                expected_exit_code=1,
                expected_stderr_fragments=("error[D017]", location, message),
            )
            for description, job_values, location, message in (
                (
                    "an invalid cron is reported at its value",
                    'select = "orders"\nschedule = { cron = "not a cron", timezone = "UTC" }\n',
                    "orchestration/jobs/orders_job.toml:4:21:",
                    "schedule cron: invalid cron expression",
                ),
                (
                    "an unknown time zone is reported at its value",
                    'select = "orders"\n'
                    'schedule = { cron = "0 2 * * *", timezone = "Mars/Olympus" }\n',
                    "orchestration/jobs/orders_job.toml:4:45:",
                    "schedule timezone is not an IANA time zone",
                ),
                (
                    "a starts_at that is not RFC 3339 is reported at its value",
                    'select = "orders"\n'
                    'schedule = { cron = "0 2 * * *", timezone = "UTC", starts_at = "yesterday" }\n',
                    "orchestration/jobs/orders_job.toml:4:64:",
                    "schedule starts_at must be an RFC 3339 timestamp",
                ),
                (
                    "an invalid build setting is reported at its value",
                    'select = "orders"\n\n[settings]\nconcurrency = "lots"\n',
                    "orchestration/jobs/orders_job.toml:6:15:",
                    "concurrency",
                ),
                (
                    "an unknown selector type is reported at the selector",
                    'select = "orders tagg:finance"\n',
                    "orchestration/jobs/orders_job.toml:3:10:",
                    "unknown selector type 'tagg' in 'tagg:finance'",
                ),
            )
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed tasks Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "tasks/orders.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): tasks/orders.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed assets Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "assets/orders.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): assets/orders.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed checks Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "checks/orders.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): checks/orders.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed loaders Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "loaders/orders.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): loaders/orders.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed factories Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "factories/orders.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): factories/orders.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed libs Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "libs/cleaning.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): libs/cleaning.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed dagster Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "dagster/definitions.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): dagster/definitions.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed rivers_pipeline Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "rivers_pipeline/definitions.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): rivers_pipeline/definitions.py",
            ),
        ),
        PythonProjectLayoutCompileTestCase(
            description="removed root definitions.py Python root",
            repo_files={
                "sqlbuild_project.toml": 'name = "python_layout"\nadapter = "duckdb"\n',
                "definitions.py": "from sqlbuild.tasks import task\n",
            },
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[D016]",
                "Unsupported project Python path(s): definitions.py",
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
                "models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n",
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
                "sources/raw.yml": (
                    "sources:\n"
                    "  - name: raw_orders\n    description: Test source raw_orders.\n"
                    "    managed: true\n"
                    "    write_strategy: table\n"
                    "    columns:\n"
                    "      - name: order_id\n"
                    "        type: INTEGER\n"
                    "      - name: status\n"
                    "        type: VARCHAR\n"
                ),
                "models/fact_orders.sql": (
                    "MODEL (description 'Test model fact_orders.', materialized table);\n\n"
                    'SELECT order_id, status FROM __source("raw_orders")\n'
                ),
                "python/_helpers.py": (
                    "def normalize_status(value):\n    return value.strip().lower()\n"
                ),
                "python/orders/__init__.py": "",
                "python/orders/utils.py": (
                    "from python._helpers import normalize_status\n\n\n"
                    "def order_rows():\n"
                    "    return [\n"
                    "        {'order_id': 1, 'status': normalize_status(' Shipped ')},\n"
                    "        {'order_id': 2, 'status': normalize_status('PENDING')},\n"
                    "    ]\n"
                ),
                "python/loaders/orders.py": (
                    "from python.orders.utils import order_rows\n"
                    "from sqlbuild.loaders import loader\n\n\n"
                    "@loader\n"
                    "def raw_orders(ctx):\n"
                    "    '''Test loader raw_orders.'''\n    return order_rows()\n"
                ),
                "python/orders/exports/assets.py": (
                    "from sqlbuild.assets import asset\n"
                    "from sqlbuild.refs import model\n\n\n"
                    "@asset(depends_on=model('fact_orders'))\n"
                    "def orders_export(ctx):\n"
                    "    '''Test asset orders_export.'''\n    relation = ctx.relation(model('fact_orders'))\n"
                    "    rows = ctx.query(f'SELECT COUNT(*) FROM {relation}').fetchall()\n"
                    "    return ctx.result(payload={'order_count': int(rows[0][0])})\n"
                ),
                "python/orders/exports/checks.py": (
                    "from python.orders.exports.assets import orders_export\n"
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
                "python/factories/orders.py": (
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.tasks import task\n\n\n"
                    "@task\n"
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
                "python/factories/orders.py": (
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.tasks import task\n\n\n"
                    "@factory\n"
                    "def order_nodes():\n"
                    "    global orders\n\n"
                    "    @task\n"
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
                "python/factories/orders.py": (
                    "from sqlbuild.factories import factory\n"
                    "from sqlbuild.tasks import task\n\n\n"
                    "@task\n"
                    "def orders(ctx):\n"
                    "    '''Test task orders.'''\n    return None\n\n\n"
                    "@factory\n"
                    "def order_nodes():\n"
                    "    @task(name='orders')\n"
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
                "python/helpers.py": "raise RuntimeError('unrelated python package imported')\n",
            },
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "python_collision"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
                ),
                "python/helpers/__init__.py": "",
                "python/helpers/values.py": "STATUS = 'shipped'\n",
                "python/helpers/clean.py": (
                    "from .values import STATUS\n\n\ndef label():\n    return STATUS\n"
                ),
                "python/orders.py": (
                    "import python.helpers.clean\n"
                    "from sqlbuild.tasks import task\n\n\n"
                    "@task\n"
                    "def orders(ctx):\n"
                    "    '''Test task orders.'''\n    return ctx.result(payload={'status': python.helpers.clean.label()})\n"
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


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
