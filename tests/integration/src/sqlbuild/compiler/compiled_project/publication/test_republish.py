"""The real CLI republishes a `sqb dev` location, which rediscovers, and reads the new tags back."""

import json
import os
import signal
import subprocess
import sys
from pathlib import Path
from typing import Any
from urllib.request import urlopen

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentRepublishTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    stop_location,
    wait_for_development_ready,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_NODES: str = '''\
import json
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

PLAN = Path({plan!r})


@discover
def fulfilment_plan() -> Discovered:
    """Steps of the fulfilment plan."""
    plan = json.loads(PLAN.read_text())
    return Discovered(
        items=[Item(id=step) for step in plan["steps"]], tags={{"release": plan["release"]}}
    )


@asset(for_each=fulfilment_plan)
def fulfil(ctx: AssetContext) -> None:
    """Run one step."""
'''


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentRepublishTestCase(
            description="a republish with a key over HTTP publishes the changed plan",
            release="0.58.0",
            expected_exit_code=0,
            expected_items=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_changed_external_plan_when_republishing_then_the_readback_shows_its_tags(
    test_case: DevelopmentRepublishTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    plan: Path = tmp_path / "plan.json"
    log: Path = tmp_path / "dev.log"
    plan.write_text(json.dumps({"steps": ["pick"], "release": "0.57.0"}))
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/plan/nodes.py": _NODES.format(plan=str(plan)),
        },
    )
    executable: str = str(Path(sys.executable).with_name("sqb"))
    with log.open("wb") as output:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [executable, "dev", "--project-dir", str(project), "--port", "0"],
            stdin=subprocess.DEVNULL,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    try:
        url, location = wait_for_development_ready(process, log)
        plan.write_text(json.dumps({"steps": ["pick", "pack"], "release": test_case.release}))
        republished: subprocess.CompletedProcess[str] = subprocess.run(
            [executable, "location", "republish", "--server", url, "--location-id", location]
            + ["--target", "development", "--reason", "deploy", "--wait", "--timeout", "45"],
            env=os.environ | {"SQLBUILD_API_KEY": "synthetic-development-key"},
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
        assert republished.returncode == test_case.expected_exit_code, republished.stderr
        status: dict[str, Any] = json.loads(republished.stdout)
        with urlopen(
            f"{url}/api/control/locations/{location}/targets/development/discovered", timeout=5
        ) as response:
            readback: dict[str, Any] = json.load(response)
        assert readback["snapshot_id"] == status["snapshot_id"]
        assert readback["sets"][0]["tags"] == {"release": test_case.release}
        assert readback["sets"][0]["items"] == test_case.expected_items
    finally:
        stop_location(process, log, signal.SIGINT)
