"""An item depending on what the project doesn't have fails its set, not the publish."""

import os
import subprocess
import sys
from pathlib import Path
from typing import Any
from urllib.error import HTTPError

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    UnknownDependencyDiscoverTestCase,
    UnknownDependencyTestCase,
    UnknownTagTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    location_health_reports,
    location_process,
    snapshot_api,
    wait_for_generation,
    wait_for_log,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_PLAN: str = '''\
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover
from sqlbuild.refs import job, model, seed, source, tag


@discover
def fulfilment_plan() -> Discovered:
    """Steps of the fulfilment plan."""
    return Discovered(items=[Item(id="pack", depends_on=({dependency},))])


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext) -> None:
    """Run one step."""
'''
_GOOD: str = '''\
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover
from sqlbuild.refs import job, model, seed, source


@discover
def support_plan() -> Discovered:
    """Steps of the support plan, after what the project has."""
    after = (model("orders"), source("raw_orders"), seed("regions"), job("orders_job"))
    return Discovered(items=[Item(id="triage", depends_on=after)])


@asset(for_each=support_plan)
def support(ctx: AssetContext) -> None:
    """Run one step."""
'''
_KEPT: frozenset[str] = frozenset({"asset:fulfil[pack]", "asset:support[triage]"})
_PLACEHOLDER: frozenset[str] = frozenset({"asset:fulfil[*]", "asset:support[triage]"})


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        UnknownDependencyTestCase(
            description="an unknown model",
            first='model("returns")',
            then='model("returns")',
            removed=(),
            missing="returns",
            expected_nodes=_PLACEHOLDER,
        ),
        UnknownDependencyTestCase(
            description="an unknown source",
            first='source("returns")',
            then='source("returns")',
            removed=(),
            missing="returns",
            expected_nodes=_PLACEHOLDER,
        ),
        UnknownDependencyTestCase(
            description="an unknown seed",
            first='seed("returns")',
            then='seed("returns")',
            removed=(),
            missing="returns",
            expected_nodes=_PLACEHOLDER,
        ),
        UnknownDependencyTestCase(
            description="an unknown job",
            first='job("returns_job")',
            then='job("returns_job")',
            removed=(),
            missing="returns_job",
            expected_nodes=_PLACEHOLDER,
        ),
        UnknownDependencyTestCase(
            description="the last good stays while the project still has what it names",
            first='model("inventory")',
            then='model("returns")',
            removed=(),
            missing="returns",
            expected_nodes=_KEPT,
        ),
        UnknownDependencyTestCase(
            description="a reused set whose model was removed",
            first='model("inventory")',
            then='model("inventory")',
            removed=("sql/models/inventory.sql",),
            missing="inventory",
            expected_nodes=_PLACEHOLDER,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_item_naming_what_the_project_lacks_when_publishing_then_only_its_set_fails(
    test_case: UnknownDependencyTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    first: str = _PLAN.format(dependency=test_case.first)
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "sql/models/inventory.sql": "MODEL (description 'Stock.');\nSELECT 1 AS item_id\n",
            "python/nodes/plan/nodes.py": first,
            "python/nodes/support/nodes.py": _GOOD,
            "sql/sources/raw.yml": (
                "sources:\n  - name: raw_orders\n    description: Raw orders.\n    schema: main\n"
            ),
            "sql/seeds/regions.csv": "region_id\n1\n",
            "sql/seeds/regions.yml": (
                "seeds:\n  - name: regions\n    description: Regions.\n    columns:\n"
                "      - name: region_id\n        type: INTEGER\n"
            ),
            "orchestration/jobs/orders_job.toml": 'name = "orders_job"\n'
            'description = "Build the orders."\nselect = "orders"\n',
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
            _ = wait_for_generation(api, "development", 1, process, log)
            (project / "sql/models/orders.sql").write_text(
                "MODEL (description 'Orders.');\nSELECT 2 AS order_id\n"
            )
            (project / "python/nodes/plan/nodes.py").write_text(
                _PLAN.format(dependency=test_case.then)
            )
            for removed in test_case.removed:
                (project / removed).unlink()
            published: dict[str, Any] = wait_for_generation(api, "development", 2, process, log)
            health: str = str(location_health_reports(api))

    assets: set[str] = set(filter(lambda key: key.startswith("asset:"), published["nodes"]))
    assert assets == test_case.expected_nodes, log.read_text()
    saved: dict[str, Any] = published["documents"]["discovered"]
    assert saved["support_plan"]["status"] == "ok"
    assert saved["fulfilment_plan"]["status"] == "failed"
    error: str = saved["fulfilment_plan"]["error"]
    assert "'pack'" in error and f"'{test_case.missing}'" in error, error
    assert "fulfilment_plan" in health and "failed" in health, health


@pytest.mark.parametrize(
    "test_case",
    [
        UnknownDependencyDiscoverTestCase(
            description="an unknown model",
            dependency='model("returns")',
            expected_error=(
                "Discover function 'fulfilment_plan' failed: item 'pack' depends on model "
                "'returns', which the project does not have"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_item_naming_what_the_project_lacks_when_compiling_with_discover_then_it_stops(
    test_case: UnknownDependencyDiscoverTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes.py": _PLAN.format(dependency=test_case.dependency),
        },
    )

    compiled: subprocess.CompletedProcess[str] = subprocess.run(
        [str(Path(sys.executable).with_name("sqb")), "--no-color", "--project-dir", str(project)]
        + ["compile", "--discover"],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
        env=os.environ | {"HOME": str(tmp_path)},
    )

    assert compiled.returncode != 0, compiled.stdout
    assert test_case.expected_error in compiled.stdout + compiled.stderr


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        UnknownTagTestCase(
            description="an unknown tag still fails the whole publish",
            dependency='tag("returns")',
            expected_error="asset 'fulfil[pack]' depends on tag 'returns', which no node carries",
            expected_status=404,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_item_naming_a_tag_no_node_carries_when_publishing_then_nothing_publishes(
    test_case: UnknownTagTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes.py": _PLAN.format(dependency=test_case.dependency),
            "python/nodes/support/nodes.py": _GOOD.replace(
                'after = (model("orders"), source("raw_orders"), seed("regions"), job("orders_job"))',
                'after = (model("orders"),)',
            ),
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
            wait_for_log(process, log, test_case.expected_error, timeout=60)
            with pytest.raises(HTTPError) as unpublished:
                _ = current_snapshot(api, "development")

    assert unpublished.value.code == test_case.expected_status
