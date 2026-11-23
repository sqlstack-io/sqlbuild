"""Real-CLI coverage of discovered nodes outside a location: the placeholder, and `--discover`."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    DiscoveredNodesE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT: str = (
    'name = "discovered"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
    '[connection]\ndatabase = "discovered.duckdb"\n\n[targets.dev]\nschema = "dev"\n'
)
_NODES: str = '''\
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover
from sqlbuild.refs import model


@discover
def plan() -> Discovered:
    """Steps from an external plan."""
    return Discovered(
        items=[Item(id="load", args={"step": "load"}), Item(id="fit", depends_on=("load",), args={"step": "fit"})],
        tags={"release": "r1"},
    )


@asset(for_each=plan, depends_on=model("orders"))
def run_step(ctx: AssetContext, step: str) -> None:
    """Run one step."""
    with (Path(__file__).parents[2] / "runs.txt").open("a") as runs:
        runs.write(f"{ctx.discovered.item}:{step}:{ctx.discovered.tags['release']}\\n")
'''


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveredNodesE2ETestCase(
            description="a plain build refuses the placeholder it cannot run",
            command=("build",),
            expected_exit_code=1,
            expected_output_patterns=(
                r"run_step is discovered at publish; run it from a location or `sqb dev`, "
                r"or pass --discover",
            ),
            expected_runs=(),
        ),
        DiscoveredNodesE2ETestCase(
            description="a build that selects the placeholder refuses it",
            command=("build", "--select", "+run_step"),
            expected_exit_code=1,
            expected_output_patterns=(
                r"run_step is discovered at publish; run it from a location or `sqb dev`, "
                r"or pass --discover",
            ),
            expected_runs=(),
        ),
        DiscoveredNodesE2ETestCase(
            description="a plain plan shows the placeholder",
            command=("plan",),
            expected_exit_code=0,
            expected_output_patterns=(r"Plan ready", r"run_step\[\*\]"),
            expected_runs=(),
        ),
        DiscoveredNodesE2ETestCase(
            description="a plan that selects the asset shows the placeholder",
            command=("plan", "--select", "+run_step"),
            expected_exit_code=0,
            expected_output_patterns=(r"Plan ready", r"run_step\[\*\]"),
            expected_runs=(),
        ),
        DiscoveredNodesE2ETestCase(
            description="--discover runs each item in order with its args and the set's tags",
            command=("build", "--discover"),
            expected_exit_code=0,
            expected_output_patterns=(r"run_step\[load\]\s+OK", r"run_step\[fit\]\s+OK"),
            expected_runs=("load:load:r1", "fit:fit:r1"),
        ),
        DiscoveredNodesE2ETestCase(
            description="a bracketed selector names one item, and + its upstream items",
            command=("build", "--discover", "--select", "+run_step[fit]"),
            expected_exit_code=0,
            expected_output_patterns=(r"run_step\[load\]\s+OK", r"run_step\[fit\]\s+OK"),
            expected_runs=("load:load:r1", "fit:fit:r1"),
        ),
        DiscoveredNodesE2ETestCase(
            description="the asset's name selects every item",
            command=("build", "--discover", "--select", "asset:run_step"),
            expected_exit_code=0,
            expected_output_patterns=(r"run_step\[load\]\s+OK", r"run_step\[fit\]\s+OK"),
            expected_runs=("load:load:r1", "fit:fit:r1"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_for_each_asset_when_planning_or_building_outside_a_location_then_only_discover_runs_its_items(
    tmp_path: Path, test_case: DiscoveredNodesE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="discovered",
        repo_files={
            "sqlbuild_project.toml": _PROJECT,
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes/asset.py": _NODES,
        },
    )
    runs_file: Path = project_dir / "runs.txt"
    runs_file.touch()
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == test_case.expected_exit_code, output
    for pattern in test_case.expected_output_patterns:
        assert re.search(pattern, output), output
    assert tuple(runs_file.read_text().split()) == test_case.expected_runs


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveredNodesE2ETestCase(
            description="compile shows the placeholder, and --discover each item",
            command=("compile", "--dag"),
            expected_exit_code=0,
            expected_output_patterns=(r'"asset:run_step\[\*\]"',),
            expected_runs=('"asset:run_step[load]"', '"asset:run_step[fit]"'),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_for_each_asset_when_compiling_then_the_dag_shows_the_placeholder_or_the_items(
    tmp_path: Path, test_case: DiscoveredNodesE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="discovered",
        repo_files={
            "sqlbuild_project.toml": _PROJECT,
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes/asset.py": _NODES,
        },
    )
    dag: Path = project_dir / "target" / "sqlbuild_dag.json"

    placeholder: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )
    placeholder_dag: str = dag.read_text()
    discovered: subprocess.CompletedProcess[str] = run_sqb(
        command=(*test_case.command, "--discover"), project_dir=project_dir
    )
    discovered_dag: str = dag.read_text()

    assert (placeholder.returncode, discovered.returncode) == (test_case.expected_exit_code,) * 2
    for pattern in test_case.expected_output_patterns:
        assert re.search(pattern, placeholder_dag), placeholder_dag
        assert not re.search(pattern, discovered_dag), discovered_dag
    for node in test_case.expected_runs:
        assert node in discovered_dag, discovered_dag


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveredNodesE2ETestCase(
            description=f"{command} --discover stops on a set without items",
            command=(command, "--discover"),
            expected_exit_code=1,
            expected_output_patterns=(
                r"Discover function 'plan' failed: the discover function returned no items",
            ),
            expected_runs=(),
        )
        for command in ("compile", "plan", "build")
    ],
    ids=lambda case: case.description,
)
def test_given_a_set_without_items_when_discovering_for_a_command_then_it_stops_with_the_error(
    tmp_path: Path, test_case: DiscoveredNodesE2ETestCase
) -> None:
    nodes: str = re.sub(r"items=\[.*\],", "items=[],", _NODES)
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="discovered",
        repo_files={
            "sqlbuild_project.toml": _PROJECT,
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes/asset.py": nodes,
        },
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project_dir
    )
    output: str = result.stdout + result.stderr

    assert result.returncode == test_case.expected_exit_code, output
    for pattern in test_case.expected_output_patterns:
        assert re.search(pattern, output), output
    assert not (project_dir / "runs.txt").exists(), test_case.expected_runs


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
