"""A `check_every` call that never returns holds up neither shutdown nor a waiting request."""

import os
import sys
import time
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    HungCheckTestCase,
    WaitingCheckTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    snapshot_api,
    stop_location,
    wait_for_generation,
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
_SLOW_ONCE: str = '''\
import os
import time
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})


@discover(check_every="60s")
def fulfilment_plan() -> Discovered:
    """Steps of the fulfilment plan; only the first check is slow."""
    if (DIRECTORY / "discovered").exists() and not (DIRECTORY / "check.pid").exists():
        (DIRECTORY / "check.pid").write_text(str(os.getpid()))
        time.sleep({check_seconds})
        (DIRECTORY / "check.done").touch()
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
            stop_location(process, int(pidfile.read_text()), log, expected_exit_code=0)
    assert test_case.expected_message in log.read_text()


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        WaitingCheckTestCase(
            description="a check that never returns gives way to a waiting publish",
            check_seconds=3600,
            expected_finished=False,
            expected_restarts=1,
        ),
        WaitingCheckTestCase(
            description="a check that finishes inside the 10 seconds is not abandoned",
            check_seconds=3,
            expected_finished=True,
            expected_restarts=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_check_when_a_publish_waits_then_only_a_check_past_its_grace_is_abandoned(
    test_case: WaitingCheckTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/plan/nodes.py": _SLOW_ONCE.format(
                directory=str(tmp_path), check_seconds=test_case.check_seconds
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
            _ = wait_for_generation(api, "development", 1, process, log)
            pidfile: Path = tmp_path / "check.pid"
            deadline: float = time.monotonic() + 100
            while not pidfile.exists():
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.2)
            (project / "models/orders.sql").write_text(
                "MODEL (description 'Orders.');\nSELECT 2 AS order_id\n"
            )
            publish_deadline: float = time.monotonic() + 45
            while '"generation":2' not in (tmp_path / "location.stdout").read_text().replace(
                " ", ""
            ):
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < publish_deadline, log.read_text()
                time.sleep(0.2)
            _ = wait_for_generation(api, "development", 2, process, log)

    assert (tmp_path / "check.done").exists() == test_case.expected_finished
    assert log.read_text().count("did not finish within 10 seconds") == (
        test_case.expected_restarts
    )
