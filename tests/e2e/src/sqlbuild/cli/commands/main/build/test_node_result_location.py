"""CLI e2e coverage: where Python node results go, so later builds read them back."""

from __future__ import annotations

import base64
import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    AdapterDefaultResultsFallbackE2ETestCase,
    PlanAfterBuildsE2ETestCase,
    PlanAfterTargetChangeE2ETestCase,
    PreservedTargetNodeResultsE2ETestCase,
    PreservedTargetRefusalE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    node_result_project,
    node_result_source_yml,
    node_results_in,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_EDGE_CASES: Path = Path("benchmarks/duckshop/edge_cases/loaders")
_LOADER: str = (
    "from sqlbuild.loaders import loader\n\n\n@loader\n"
    "def raw_orders(ctx: object) -> list[dict[str, object]]:\n"
    '    """Raw orders."""\n    _ = ctx\n    return [{"order_id": 1}]\n'
)
_TASK: str = (
    "from sqlbuild.refs import source\nfrom sqlbuild.assets import asset\n\n\n"
    '@asset(depends_on=source("raw_orders"))\n'
    "def announce_orders(ctx):\n"
    '    """Announce the loaded orders."""\n'
    "    return ctx.result(payload={'announced': True})\n"
)
_CHECK: str = (
    "from python.nodes.tasks.announce.asset import announce_orders\nfrom sqlbuild.checks import check\n\n\n"
    "@check(depends_on=announce_orders)\n"
    "def check_orders_announced(ctx):\n"
    '    """The task announced the orders."""\n'
    "    announced = ctx.result_of(node_function=announce_orders).payload['announced']\n"
    "    return ctx.pass_(message='announced') if announced else ctx.fail(message='silent')\n"
)
_COPYING_LOADER: str = (
    "from python.nodes.loaders.orders.loader import raw_orders\nfrom sqlbuild.loaders import loader\n\n\n"
    "@loader\n"
    "def raw_order_copies(ctx) -> list[dict[str, object]]:\n"
    '    """One order when the orders loader has a result to read, none otherwise."""\n'
    "    found = ctx.result_of(node_function=raw_orders, default=None) is not None\n"
    "    return [{'order_id': 1}] if found else []\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        PreservedTargetNodeResultsE2ETestCase(
            description="a preserved schema keeps node results in the loader schema",
            target_lines='schema = "preserve"\nloader_schema = "raw"\n',
            source_location="    schema: raw\n",
            expected_results_schema="raw",
            expected_runs_per_node=2,
        ),
        PreservedTargetNodeResultsE2ETestCase(
            description="a preserved schema keeps node results in a loader schema set by a var",
            target_lines=(
                'schema = "preserve"\nloader_schema = "${results_schema}"\n\n'
                '[vars]\nresults_schema = "raw"\n'
            ),
            source_location="    schema: raw\n",
            expected_results_schema="raw",
            expected_runs_per_node=2,
        ),
        PreservedTargetNodeResultsE2ETestCase(
            description="a preserved database keeps node results in the connection database",
            target_lines='database = "preserve"\nschema = "analytics"\nloader_schema = "raw"\n',
            source_location="    database: dev\n    schema: raw\n",
            expected_results_schema="analytics",
            expected_runs_per_node=2,
        ),
        PreservedTargetNodeResultsE2ETestCase(
            description="a schema set by --vars holds the node results build and load share",
            target_lines='schema = "${results_schema}"\n\n[vars]\nresults_schema = "mart"\n',
            source_location="    schema: raw\n",
            expected_results_schema="ops",
            expected_runs_per_node=2,
            cli_args=("--vars", json.dumps({"results_schema": "ops"})),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_location_when_building_and_loading_then_node_results_stay_in_one_place(
    test_case: PreservedTargetNodeResultsE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = node_result_project(
        tmp_path=tmp_path,
        target_lines=test_case.target_lines,
        files={
            "sql/sources/raw.yml": node_result_source_yml(test_case.source_location),
            "sql/sources/copies.yml": node_result_source_yml(test_case.source_location).replace(
                "raw_orders", "raw_order_copies"
            ),
            "python/nodes/loaders/orders/loader.py": _LOADER,
            "python/nodes/loaders/copies/loader.py": _COPYING_LOADER,
            "python/nodes/tasks/announce/asset.py": _TASK,
            "python/nodes/checks/orders/checks.py": _CHECK,
        },
    )

    results: list[subprocess.CompletedProcess[str]] = [
        run_sqb(command=("--no-color", "build", *test_case.cli_args), project_dir=project_dir)
        for _ in range(2)
    ] + [
        run_sqb(
            command=("--no-color", "load", "--select", "raw_order_copies", *test_case.cli_args),
            project_dir=project_dir,
        )
    ]

    for result in results:
        assert result.returncode == 0, result.stdout + result.stderr
    db_path: Path = project_dir / "dev.duckdb"
    assert (
        query_duckdb(
            db_path=db_path,
            sql=(
                "SELECT catalog_name, schema_name FROM information_schema.schemata "
                "WHERE lower(schema_name) = 'preserve' OR lower(catalog_name) = 'preserve'"
            ),
        )
        == []
    )
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT table_schema FROM information_schema.tables "
            "WHERE table_name = '_sqlbuild_node_results'"
        ),
    ) == [(test_case.expected_results_schema,)]
    assert query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT node_type, node_name, count(DISTINCT run_id) "
            f"FROM {test_case.expected_results_schema}._sqlbuild_node_results "
            "GROUP BY ALL ORDER BY ALL"
        ),
    ) == [
        ("asset", "announce_orders", test_case.expected_runs_per_node),
        ("check", "check_orders_announced", test_case.expected_runs_per_node),
        ("loader", "raw_order_copies", test_case.expected_runs_per_node),
        ("loader", "raw_orders", test_case.expected_runs_per_node),
    ]
    assert query_duckdb(db_path=db_path, sql="SELECT order_id FROM raw.raw_order_copies") == [(1,)]


@pytest.mark.parametrize(
    "test_case",
    [
        PreservedTargetRefusalE2ETestCase(
            description="a task under a preserved schema needs a loader schema",
            python_files={
                "python/nodes/tasks/announce/asset.py": _TASK.replace(
                    'depends_on=source("raw_orders")', ""
                )
            },
            expected_error="error[S105]: Target 'dev' keeps each model's own schema",
        ),
        PreservedTargetRefusalE2ETestCase(
            description="a loader schema whose var resolves to empty is no loader schema",
            python_files={
                "python/nodes/tasks/announce/asset.py": _TASK.replace(
                    'depends_on=source("raw_orders")', ""
                )
            },
            expected_error="error[S105]: Target 'dev' keeps each model's own schema",
            target_lines='loader_schema = "${results_schema}"\n\n[vars]\nresults_schema = ""\n',
        ),
        PreservedTargetRefusalE2ETestCase(
            description="a loader schema that resolves only inside a model is no loader schema",
            python_files={
                "python/nodes/tasks/announce/asset.py": _TASK.replace(
                    'depends_on=source("raw_orders")', ""
                )
            },
            expected_error="error[S105]: Target 'dev' keeps each model's own schema",
            target_lines='loader_schema = "raw_${CTX:model.name}"\n',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_preserved_schema_without_loader_schema_when_compiling_then_python_nodes_are_refused(
    test_case: PreservedTargetRefusalE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                '[connection]\ndatabase = "dev.duckdb"\n\n'
                '[defaults]\nschema = "mart"\n\n[targets.dev]\nschema = "preserve"\n'
                + test_case.target_lines
            ),
            "sql/models/orders.sql": "MODEL (description 'Orders.');\n\nSELECT 1 AS order_id\n",
        }
        | test_case.python_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert test_case.expected_error in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        PlanAfterBuildsE2ETestCase(
            description="a preserved schema reads the task's fingerprint from the loader schema",
            target_lines='schema = "preserve"\nloader_schema = "raw"\n',
            expected_plan_line="announce_orders      asset (unchanged)",
        ),
        PlanAfterBuildsE2ETestCase(
            description="a plan of the task alone reads its fingerprint from the loader schema",
            target_lines='schema = "preserve"\nloader_schema = "raw"\n',
            expected_plan_line="announce_orders      asset (unchanged)",
            plan_args=("--select", "announce_orders"),
        ),
        PlanAfterBuildsE2ETestCase(
            description="a named schema reads the task's fingerprint from the target schema",
            target_lines='schema = "analytics"\n',
            expected_plan_line="announce_orders      asset (unchanged)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_builds_when_planning_then_python_nodes_are_not_new(
    test_case: PlanAfterBuildsE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = node_result_project(
        tmp_path=tmp_path,
        target_lines=test_case.target_lines,
        files={
            "sql/sources/raw.yml": node_result_source_yml("    schema: raw\n"),
            "python/nodes/loaders/orders/loader.py": _LOADER,
            "python/nodes/tasks/announce/asset.py": _TASK,
        },
    )
    for _ in range(2):
        build: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        assert build.returncode == 0, build.stdout + build.stderr

    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", *test_case.plan_args), project_dir=project_dir
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert test_case.expected_plan_line in plan.stdout, plan.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        PlanAfterTargetChangeE2ETestCase(
            description="older task results in a later-sorting model schema do not hide current ones",
            earlier_target_lines='schema = "staging"\n',
            target_lines='schema = "preserve"\nloader_schema = "raw"\n',
            expected_plan_line="announce_orders      asset (unchanged)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_older_named_schema_results_when_planning_preserved_target_then_loader_schema_wins(
    test_case: PlanAfterTargetChangeE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = node_result_project(
        tmp_path=tmp_path,
        target_lines=test_case.earlier_target_lines,
        files={
            "sql/sources/raw.yml": node_result_source_yml("    schema: raw\n"),
            "python/nodes/loaders/orders/loader.py": _LOADER,
            "python/nodes/tasks/announce/asset.py": _TASK,
            "sql/models/staged_orders.sql": (
                "MODEL (description 'Staged orders.', schema \"staging\");\n\n"
                "SELECT 1 AS order_id\n"
            ),
            "sql/models/raw_order_ids.sql": (
                "MODEL (description 'Order ids beside the raw data.', schema \"raw\");\n\n"
                "SELECT 1 AS order_id\n"
            ),
        },
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    config: Path = project_dir / "sqlbuild_project.toml"
    config.write_text(
        config.read_text().replace(test_case.earlier_target_lines, test_case.target_lines)
    )
    task: Path = project_dir / "python/nodes/tasks/announce/asset.py"
    task.write_text(task.read_text().replace("'announced': True", "'announced': 1"))
    for _ in range(2):
        build: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        assert build.returncode == 0, build.stdout + build.stderr

    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert test_case.expected_plan_line in plan.stdout, plan.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterDefaultResultsFallbackE2ETestCase(
            description="a task reads a rebuilt loader and one whose result is only in main",
            target_schema="mart",
            rebuilt_loader="raw_order_events",
            kept_loader="raw_customer_events",
            reading_task="announce_events",
            expected_payload_keys=("orders", "customers"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_results_in_the_adapter_default_when_one_loader_rebuilds_then_both_are_still_read(
    test_case: AdapterDefaultResultsFallbackE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                '[connection]\ndatabase = "dev.duckdb"\n\n[targets.dev]\n'
            ),
            "sql/sources/events.yml": (_EDGE_CASES / "loader_results.yml").read_text(),
            "python/nodes/loaders/events.py": (_EDGE_CASES / "loader_results.py").read_text(),
        },
    )
    db_path: Path = project_dir / "dev.duckdb"
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    old_runs: dict[str, str] = {
        name: run
        for name, run, _ in node_results_in(db_path=db_path, schema="main", node_type="loader")
    }
    toml: Path = project_dir / "sqlbuild_project.toml"
    toml.write_text(
        toml.read_text(encoding="utf-8") + f'schema = "{test_case.target_schema}"\n',
        encoding="utf-8",
    )

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "build",
            "--select",
            test_case.rebuilt_loader,
            test_case.reading_task,
        ),
        project_dir=project_dir,
    )

    assert second.returncode == 0, second.stdout + second.stderr
    new_runs: dict[str, str] = {
        name: run
        for name, run, _ in node_results_in(
            db_path=db_path, schema=test_case.target_schema, node_type="loader"
        )
    }
    assert set(new_runs) == {test_case.rebuilt_loader}
    assert new_runs[test_case.rebuilt_loader] != old_runs[test_case.rebuilt_loader]
    [(_, task_run, payload)] = node_results_in(
        db_path=db_path, schema=test_case.target_schema, node_type="asset"
    )
    assert task_run == new_runs[test_case.rebuilt_loader]
    rebuilt_key, kept_key = test_case.expected_payload_keys
    assert json.loads(base64.b64decode(payload)) == {
        rebuilt_key: new_runs[test_case.rebuilt_loader],
        kept_key: old_runs[test_case.kept_loader],
    }
    assert {
        name: run
        for name, run, _ in node_results_in(db_path=db_path, schema="main", node_type="loader")
    } == old_runs
