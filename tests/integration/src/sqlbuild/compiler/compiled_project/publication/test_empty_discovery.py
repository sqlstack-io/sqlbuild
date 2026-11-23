"""A discovery that finds no items fails its set, at publish and on a `check_every` check."""

import os
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    EmptyDiscoveryTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_health_reports,
    location_process,
    snapshot_api,
    wait_for_generation,
    wait_for_log,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_NODES: str = '''\
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

ITEMS = Path({items!r})


@discover(check_every="60s")
def fulfilment_plan() -> Discovered:
    """Steps of the fulfilment plan."""
    return Discovered(items=[Item(id=step) for step in ITEMS.read_text().split()])


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext) -> None:
    """Run one step."""
'''
_ERROR: str = "the discover function returned no items"


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        EmptyDiscoveryTestCase(
            description="the last good items stay",
            first_items=("pick",),
            expected_nodes=frozenset({"asset:fulfil[pick]"}),
            expected_kept_items=("pick",),
        ),
        EmptyDiscoveryTestCase(
            description="a set that never found items is the placeholder",
            first_items=(),
            expected_nodes=frozenset({"asset:fulfil[*]"}),
            expected_kept_items=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_items_when_checking_and_publishing_then_the_set_fails_and_the_location_publishes(
    test_case: EmptyDiscoveryTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    items: Path = tmp_path / "items.txt"
    items.write_text(" ".join(test_case.first_items))
    nodes: str = _NODES.format(items=str(items))
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "python/nodes/plan/nodes.py": nodes,
        },
    )
    log: Path = tmp_path / "location.log"
    with snapshot_api(tmp_path / "control") as api:
        arguments: list[str] = [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(project),
            "location",
            "serve",
            "--server",
            api.url,
            "--location-id",
            api.location_id,
            "--target",
            "development",
        ]
        environment: dict[str, str] = os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer}
        with location_process(
            arguments, log, tmp_path / "location.stdout", env=environment
        ) as process:
            first: dict[str, Any] = wait_for_generation(api, "development", 1, process, log)
            assert frozenset(first["nodes"]) == test_case.expected_nodes, log.read_text()
            items.write_text("")
            wait_for_log(
                process,
                log,
                f"Discovered set fulfilment_plan check failed: {_ERROR}",
                timeout=100,
            )
            (project / "python/nodes/plan/nodes.py").write_text(
                nodes + "\n# An edit runs it again.\n"
            )
            second: dict[str, Any] = wait_for_generation(api, "development", 2, process, log)
            health: str = str(location_health_reports(api))

    assert frozenset(second["nodes"]) == test_case.expected_nodes, log.read_text()
    saved: dict[str, Any] = second["documents"]["discovered"]["fulfilment_plan"]
    assert (saved["status"], saved["error"]) == ("failed", _ERROR)
    kept: list[dict[str, Any]] = saved.get("result", {"items": []})["items"]
    assert tuple(item["id"] for item in kept) == test_case.expected_kept_items
    assert "fulfilment_plan" in health and "failed" in health, health
