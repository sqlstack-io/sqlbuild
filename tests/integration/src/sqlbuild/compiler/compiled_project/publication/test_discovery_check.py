"""A `check_every` call that never returns doesn't hold a location's shutdown."""

import os
import sys
import time
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    HungCheckTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    snapshot_api,
    stop_location,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_NODES: str = '''\
import os
import time
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})


@discover(check_every={check_every!r})
def fulfilment_plan() -> Discovered:
    """Steps of the fulfilment plan; every call after the first never returns."""
    if (DIRECTORY / "discovered").exists():
        (DIRECTORY / "check.pid").write_text(str(os.getpid()))
        while True:
            time.sleep(1)
    (DIRECTORY / "discovered").touch()
    return Discovered(items=[Item(id="pick")])


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext) -> None:
    """Run one step."""
'''


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        HungCheckTestCase(
            description="SIGTERM during a check that never returns",
            check_every="60s",
            check_starts_within_seconds=100,
            expected_message="Stopped checking discovered set fulfilment_plan",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_check_that_never_returns_when_the_location_is_stopped_then_it_exits_promptly(
    test_case: HungCheckTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    nodes: str = _NODES.format(directory=str(tmp_path), check_every=test_case.check_every)
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "python/plan/nodes.py": nodes,
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
            pidfile: Path = tmp_path / "check.pid"
            deadline: float = time.monotonic() + test_case.check_starts_within_seconds
            while not (pidfile.exists() and pidfile.read_text()):
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.2)
            stop_location(process, int(pidfile.read_text()), log)
    assert test_case.expected_message in log.read_text()
