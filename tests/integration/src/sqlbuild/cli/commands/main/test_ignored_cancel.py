"""Real CLI coverage for a cancel of discovered steps that ignore it."""

import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    HungSetupCancelTestCase,
    IgnoredCancelTestCase,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_STEPS: str = '''\
import time
from pathlib import Path

from providers.marker import MarkerProvider
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})


@discover
def fulfilment_plan() -> Discovered:
    """Two steps at once."""
    return Discovered(items=[Item(id="pack"), Item(id="pick")], max_parallel=2)


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext, marker_provider: MarkerProvider) -> None:
    """Ignore any cancel."""
    (DIRECTORY / f"{{ctx.discovered.item}}.running").touch()
    time.sleep(120)
'''
# One Ctrl+C may force an exit only while no statement has ever run, so this adapter records none.
_UNRECORDED_ADAPTER: str = '''\
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class DuckDbUnrecordedAdapter(DuckDbAdapter):
    """Run statements without recording them as running."""

    adapter_name = "duckdb_unrecorded"

    def execute(self, *, connection, sql):
        return self._execute(connection=connection, sql=sql)
'''
_PROVIDER: str = '''\
from pathlib import Path

from sqlbuild.providers import Provider


class MarkerProvider(Provider):
    """Say when it was torn down."""

    def teardown(self) -> None:
        Path({directory!r}, "torn-down").touch()
'''

_HUNG_STEPS: str = """\
from providers.gateway import GatewayProvider
from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover


@discover
def fulfilment_plan() -> Discovered:
    \"\"\"Two steps at once.\"\"\"
    return Discovered(items=[Item(id="pack"), Item(id="pick")], max_parallel=2)


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext, gateway_provider: GatewayProvider) -> None:
    \"\"\"Use the gateway.\"\"\"
"""
_HUNG_PROVIDER: str = """\
import time
from pathlib import Path

from sqlbuild.providers import Provider


class GatewayProvider(Provider):
    \"\"\"A gateway whose first connection never completes.\"\"\"

    def setup(self, ctx) -> None:
        Path({directory!r}, "setup-started").touch()
        time.sleep(3600)

    def teardown(self) -> None:
        Path({directory!r}, "torn-down").touch()
"""


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signals")
@pytest.mark.parametrize(
    "test_case",
    [
        IgnoredCancelTestCase(
            description="SIGTERM exits after the 5 seconds without provider teardown",
            sent_signal=signal.SIGTERM,
            expected_exit_code=143,
            expected_line="error: terminated by SIGTERM",
            expected_within_seconds=20,
        ),
        IgnoredCancelTestCase(
            description="Ctrl+C exits after the 5 seconds without provider teardown",
            sent_signal=signal.SIGINT,
            expected_exit_code=130,
            expected_line="error: interrupted (Ctrl+C)",
            expected_within_seconds=20,
        ),
        IgnoredCancelTestCase(
            description="one Ctrl+C waits the 5 seconds though no statement has ever run",
            sent_signal=signal.SIGINT,
            expected_exit_code=130,
            expected_line="error: interrupted (Ctrl+C)",
            expected_within_seconds=20,
            adapter="duckdb_unrecorded",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_steps_ignoring_cancel_when_signalled_then_the_command_exits_without_teardown(
    test_case: IgnoredCancelTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": f'name = "fulfilment"\nadapter = "{test_case.adapter}"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "python/steps.py": _STEPS.format(directory=str(tmp_path)),
            "providers/marker.py": _PROVIDER.format(directory=str(tmp_path)),
            "adapters/duckdb_unrecorded.py": _UNRECORDED_ADAPTER,
        },
    )
    log: Path = tmp_path / "build.log"
    with log.open("wb") as output:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [str(Path(sys.executable).with_name("sqb")), "--no-color"]
            + ["--project-dir", str(project), "build", "--discover"],
            stdout=output,
            stderr=subprocess.STDOUT,
            env=os.environ | {"HOME": str(tmp_path)},
        )
        try:
            deadline: float = time.monotonic() + 60
            while len(list(tmp_path.glob("*.running"))) < 2:
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.05)
            process.send_signal(test_case.sent_signal)
            exit_code: int = process.wait(timeout=test_case.expected_within_seconds)
        finally:
            process.kill()
            _ = process.wait(timeout=5)

    assert exit_code == test_case.expected_exit_code, log.read_text()
    assert test_case.expected_line in log.read_text()
    assert "Stopped waiting for fulfil[pack], fulfil[pick] after 5 seconds" in log.read_text()
    assert not (tmp_path / "torn-down").exists()


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signals")
@pytest.mark.parametrize(
    "test_case",
    [
        HungSetupCancelTestCase(
            description="SIGTERM exits after the 5 seconds",
            sent_signal=signal.SIGTERM,
            expected_exit_code=143,
            expected_within_seconds=20,
        ),
        HungSetupCancelTestCase(
            description="Ctrl+C exits after the 5 seconds",
            sent_signal=signal.SIGINT,
            expected_exit_code=130,
            expected_within_seconds=20,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_step_in_a_provider_setup_that_never_returns_when_cancelled_then_the_command_exits(
    test_case: HungSetupCancelTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "python/steps.py": _HUNG_STEPS,
            "providers/gateway.py": _HUNG_PROVIDER.format(directory=str(tmp_path)),
        },
    )
    log: Path = tmp_path / "build.log"
    with log.open("wb") as output:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [str(Path(sys.executable).with_name("sqb")), "--no-color"]
            + ["--project-dir", str(project), "build", "--discover"],
            stdout=output,
            stderr=subprocess.STDOUT,
            env=os.environ | {"HOME": str(tmp_path)},
        )
        try:
            deadline: float = time.monotonic() + 60
            while not (tmp_path / "setup-started").exists():
                assert process.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.05)
            process.send_signal(test_case.sent_signal)
            exit_code: int = process.wait(timeout=test_case.expected_within_seconds)
        finally:
            process.kill()
            _ = process.wait(timeout=5)

    assert exit_code == test_case.expected_exit_code, log.read_text()
    assert "Stopped waiting for fulfil[pack], fulfil[pick] after 5 seconds" in log.read_text()
    assert not (tmp_path / "torn-down").exists()
