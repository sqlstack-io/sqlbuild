"""CLI e2e coverage: where Python node results go, so later builds read them back."""

from __future__ import annotations

import base64
import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    PreservedTargetNodeResultsE2ETestCase,
    PreservedTargetRefusalE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_LOADER: str = (
    "from sqlbuild.loaders import loader\n\n\n@loader\n"
    "def raw_orders(ctx: object) -> list[dict[str, object]]:\n"
    '    """Raw orders."""\n    _ = ctx\n    return [{"order_id": 1}]\n'
)
_TASK: str = (
    "from sqlbuild.refs import source\nfrom sqlbuild.tasks import task\n\n\n"
    '@task(depends_on=source("raw_orders"))\n'
    "def announce_orders(ctx):\n"
    '    """Announce the loaded orders."""\n'
    "    return ctx.result(payload={'announced': True})\n"
)


def _source_yml(location: str) -> str:
    return (
        "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
        f"    managed: true\n{location}    write_strategy: table\n"
        "    columns:\n      - name: order_id\n        type: INTEGER\n"
    )


def _project(*, tmp_path: Path, target_lines: str, files: dict[str, str]) -> Path:
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                '[connection]\ndatabase = "dev.duckdb"\n\n'
                f'[defaults]\ndatabase = "dev"\nschema = "mart"\n\n[targets.dev]\n{target_lines}'
            ),
            "models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n\n"
                'SELECT order_id FROM __source("raw_orders")\n'
            ),
        }
        | files,
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
            description="a preserved database keeps node results in the connection database",
            target_lines='database = "preserve"\nschema = "analytics"\nloader_schema = "raw"\n',
            source_location="    database: dev\n    schema: raw\n",
            expected_results_schema="analytics",
            expected_runs_per_node=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_preserved_target_when_building_twice_then_node_results_stay_in_one_real_location(
    test_case: PreservedTargetNodeResultsE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = _project(
        tmp_path=tmp_path,
        target_lines=test_case.target_lines,
        files={
            "sources/raw.yml": _source_yml(test_case.source_location),
            "python/loaders/orders.py": _LOADER,
            "python/tasks/announce.py": _TASK,
        },
    )

    results: list[subprocess.CompletedProcess[str]] = [
        run_sqb(command=("--no-color", "build"), project_dir=project_dir) for _ in range(2)
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
        ("loader", "raw_orders", test_case.expected_runs_per_node),
        ("task", "announce_orders", test_case.expected_runs_per_node),
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        PreservedTargetRefusalE2ETestCase(
            description="a task under a preserved schema needs a loader schema",
            python_files={
                "python/tasks/announce.py": _TASK.replace('depends_on=source("raw_orders")', "")
            },
            expected_error="error[S105]: Target 'dev' keeps each model's own schema",
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
            ),
            "models/orders.sql": "MODEL (description 'Orders.');\n\nSELECT 1 AS order_id\n",
        }
        | test_case.python_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert test_case.expected_error in result.stderr


_EDGE_CASES: Path = Path("benchmarks/duckshop/edge_cases/loaders")


def _results(*, db_path: Path, schema: str, node_type: str) -> list[tuple[str, str, str]]:
    return query_duckdb(
        db_path=db_path,
        sql=(
            f"SELECT node_name, run_id, payload_json_b64 FROM {schema}._sqlbuild_node_results "
            f"WHERE node_type = '{node_type}' ORDER BY node_name"
        ),
    )


def test_given_results_in_the_adapter_default_when_one_loader_rebuilds_then_both_are_still_read(
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                '[connection]\ndatabase = "dev.duckdb"\n\n[targets.dev]\n'
            ),
            "sources/events.yml": (_EDGE_CASES / "loader_results.yml").read_text(),
            "python/loaders/events.py": (_EDGE_CASES / "loader_results.py").read_text(),
        },
    )
    db_path: Path = project_dir / "dev.duckdb"
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    old_runs: dict[str, str] = {
        name: run for name, run, _ in _results(db_path=db_path, schema="main", node_type="loader")
    }
    toml: Path = project_dir / "sqlbuild_project.toml"
    toml.write_text(toml.read_text(encoding="utf-8") + 'schema = "mart"\n', encoding="utf-8")

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "raw_order_events", "announce_events"),
        project_dir=project_dir,
    )

    assert second.returncode == 0, second.stdout + second.stderr
    new_runs: dict[str, str] = {
        name: run for name, run, _ in _results(db_path=db_path, schema="mart", node_type="loader")
    }
    assert set(new_runs) == {"raw_order_events"}
    assert new_runs["raw_order_events"] != old_runs["raw_order_events"]
    [(_, task_run, payload)] = _results(db_path=db_path, schema="mart", node_type="task")
    assert task_run == new_runs["raw_order_events"]
    assert json.loads(base64.b64decode(payload)) == {
        "orders": new_runs["raw_order_events"],
        "customers": old_runs["raw_customer_events"],
    }
    assert {
        name: run for name, run, _ in _results(db_path=db_path, schema="main", node_type="loader")
    } == old_runs
