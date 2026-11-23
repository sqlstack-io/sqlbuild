"""The real CLI republishes a `sqb dev` location, which rediscovers, and reads the new tags back."""

import ipaddress
import json
import os
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentRepublishTestCase,
    KeyedRepublishTestCase,
    SessionRepublishTestCase,
    SnapshotApi,
    WaitingRepublishTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    location_process,
    network_address,
    wait_for_development_ready,
    wait_for_generation,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.login.helpers import (
    authentication_arguments,
    browser,
    gateway,
    pending_login,
    request,
    sign_in,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    administer,
    environment,
    serve,
    successful,
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
_CHECKED: str = '''\
import json
import time
from pathlib import Path

from sqlbuild.assets import AssetContext, asset
from sqlbuild.discovery import Discovered, Item, discover

DIRECTORY = Path({directory!r})


@discover(check_every="60s")
def fulfilment_plan() -> Discovered:
    """Steps of the fulfilment plan; only the first check is slow."""
    plan = json.loads((DIRECTORY / "plan.json").read_text())
    if (DIRECTORY / "discovered").exists() and not (DIRECTORY / "check.started").exists():
        (DIRECTORY / "check.started").touch()
        time.sleep({check_seconds})
        (DIRECTORY / "check.done").touch()
    (DIRECTORY / "discovered").touch()
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
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes.py": _NODES.format(plan=str(plan)),
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
        process.send_signal(signal.SIGINT)
        process.wait(timeout=30)


@pytest.mark.skipif(sys.platform != "linux", reason="Linux owner-only credential storage")
@pytest.mark.parametrize(
    "test_case",
    [
        SessionRepublishTestCase(
            description="a saved login session queues a republish over HTTPS",
            reason="deploy",
            expected_exit_code=0,
            expected_state="pending",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_saved_login_session_when_republishing_then_the_request_names_the_person(
    test_case: SessionRepublishTestCase, tmp_path: Path
) -> None:
    binary: str | None = shutil.which("sqb")
    assert binary is not None
    installation: tuple[Path, Path] = Path(binary), tmp_path
    home: Path = tmp_path / "home"
    home.mkdir()
    values: dict[str, str] = environment({"HOME": str(home)})
    for name in ("DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR", "SQLBUILD_API_KEY"):
        values.pop(name, None)
    successful(administer(installation, ["create-user", "orders-admin"]))
    person: dict[str, Any] = successful(
        administer(installation, ["create-principal", "orders-deploy"])
    )
    password: str = "orders-private-password"
    successful(administer(installation, ["set-password", person["principal_id"]], password))
    successful(administer(installation, ["grant-admin", person["principal_id"]]))
    location: dict[str, Any] = successful(administer(installation, ["create-location", "orders"]))
    credential: dict[str, Any] = successful(
        administer(
            installation,
            ["issue-location-token", location["location_id"]]
            + ["--expires-at", "2099-01-01T00:00:00Z"],
        )
    )
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n'
            'default_target = "production"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.production]\nschema = "main"\n',
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
        },
    )
    with gateway(tmp_path) as (origin, certificate, transport):
        arguments: tuple[str, ...] = authentication_arguments(
            "password", origin, certificate, person["principal_id"]
        )
        with serve(installation, authentication_arguments=arguments) as backend:
            transport["backend"] = backend.removeprefix("http://")
            published: subprocess.CompletedProcess[str] = subprocess.run(
                [binary, "--project-dir", str(project), "location", "publish"]
                + ["--server", backend, "--location-id", location["location_id"]]
                + ["--target", "production", "--generation", "1"],
                env=values | {"SQLBUILD_LOCATION_TOKEN": credential["bearer"]},
                capture_output=True,
                text=True,
                timeout=60,
                check=False,
            )
            assert published.returncode == 0, published.stderr
            client, _ = browser(origin, certificate)
            csrf: str = sign_in(client, origin, "password", person["name"], password)
            common: list[str] = ["--server", origin, "--ca-cert", str(certificate)]
            with pending_login(
                [binary, "login", *common, "--no-browser"], values, tmp_path, tmp_path / "cli.log"
            ) as (process, url):
                approval: dict[str, Any] = {"browser_code": url.split("#", 1)[1], "approve": True}
                approved: int = request(
                    client, origin + "/api/auth/cli/approve", approval, csrf=csrf
                )[0]
                assert approved == 204
                process.communicate(timeout=20)
                assert process.returncode == 0
            republished: subprocess.CompletedProcess[str] = subprocess.run(
                [binary, "location", "republish", *common, "--target", "production"]
                + ["--location-id", location["location_id"], "--reason", test_case.reason],
                cwd=tmp_path,
                env=values,
                capture_output=True,
                text=True,
                timeout=20,
                check=False,
            )
            assert republished.returncode == test_case.expected_exit_code, republished.stderr
            status: dict[str, Any] = json.loads(republished.stdout)
            assert status["requested_by"] == person["name"]
            assert status["state"] == test_case.expected_state


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        WaitingRepublishTestCase(
            description="a check that never returns gives way to the republish",
            check_seconds=3600,
            release="0.59.0",
            expected_exit_code=0,
            expected_check_finished=False,
        ),
        WaitingRepublishTestCase(
            description="a check that finishes inside the 10 seconds is not abandoned",
            check_seconds=3,
            release="0.59.0",
            expected_exit_code=0,
            expected_check_finished=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_republish_waiting_on_a_check_when_the_grace_passes_then_the_republish_completes(
    test_case: WaitingRepublishTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    plan: Path = tmp_path / "plan.json"
    log: Path = tmp_path / "dev.log"
    plan.write_text(json.dumps({"steps": ["pick"], "release": "0.58.0"}))
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes.py": _CHECKED.format(
                directory=str(tmp_path), check_seconds=test_case.check_seconds
            ),
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
        deadline: float = time.monotonic() + 120
        while not (tmp_path / "check.started").exists():
            assert process.poll() is None, log.read_text()
            assert time.monotonic() < deadline, log.read_text()
            time.sleep(0.2)
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
        with urlopen(
            f"{url}/api/control/locations/{location}/targets/development/discovered", timeout=5
        ) as response:
            readback: dict[str, Any] = json.load(response)
        assert readback["sets"][0]["tags"] == {"release": test_case.release}
        assert (tmp_path / "check.done").exists() == test_case.expected_check_finished
    finally:
        process.send_signal(signal.SIGINT)
        process.wait(timeout=30)


@pytest.mark.skipif(sys.platform == "win32", reason="Unix signal and process ownership contract")
@pytest.mark.parametrize(
    "test_case",
    [
        KeyedRepublishTestCase(
            description="an operator's key publishes the changed plan",
            role="operator",
            sign_in_arguments=(),
            expected_exit_code=0,
            expected_stderr="Republish published a snapshot",
            expected_generation=2,
            expected_release="0.58.0",
        ),
        KeyedRepublishTestCase(
            description="an operator's key needs neither the password origin nor a CSRF proof",
            role="operator",
            sign_in_arguments=("--password-origin", "https://control.example.com"),
            expected_exit_code=0,
            expected_stderr="Republish published a snapshot",
            expected_generation=2,
            expected_release="0.58.0",
        ),
        KeyedRepublishTestCase(
            description="a viewer's key is refused and nothing is published",
            role="viewer",
            sign_in_arguments=(),
            expected_exit_code=2,
            expected_stderr="API rejected the request with HTTP 403",
            expected_generation=1,
            expected_release="0.57.0",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_service_account_key_over_network_http_when_republishing_then_its_role_decides(
    test_case: KeyedRepublishTestCase, tmp_path: Path
) -> None:
    executable: Path = Path(sys.executable).with_name("sqb")
    installation: tuple[Path, Path] = executable, tmp_path
    project: Path = tmp_path / "project"
    plan: Path = tmp_path / "plan.json"
    log: Path = tmp_path / "location.log"
    plan.write_text(json.dumps({"steps": ["pick"], "release": "0.57.0"}))
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": 'name = "fulfilment"\nadapter = "duckdb"\n'
            'default_target = "development"\n\n[connection]\ndatabase = "warehouse.duckdb"\n\n'
            '[targets.development]\nschema = "main"\n',
            "sql/models/orders.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n",
            "python/nodes/plan/nodes.py": _NODES.format(plan=str(plan)),
        },
    )
    successful(administer(installation, ["create-user", "orders-admin"]))
    created: dict[str, Any] = successful(administer(installation, ["create-location", "orders"]))
    location: str = created["location_id"]
    credential: dict[str, Any] = successful(
        administer(
            installation,
            ["issue-location-token", location, "--expires-at", "2099-01-01T00:00:00Z"],
        )
    )
    account: dict[str, Any] = successful(
        administer(
            installation, ["create-service-account", "orders-deploy", "--role", test_case.role]
        )
    )
    key: str = account["key"]
    address: str = network_address()
    assert not ipaddress.ip_address(address).is_loopback
    with serve(
        installation, authentication_arguments=("--host", "0.0.0.0", *test_case.sign_in_arguments)
    ) as listening:
        port: str = listening.rsplit(":", 1)[1]
        api: SnapshotApi = SnapshotApi(
            url=f"http://127.0.0.1:{port}", location_id=location, bearer=credential["bearer"]
        )
        with location_process(
            [str(executable), "--project-dir", str(project), "location", "serve"]
            + ["--server", api.url, "--location-id", location, "--target", "development"],
            log,
            tmp_path / "location.stdout",
            env=environment({"SQLBUILD_LOCATION_TOKEN": api.bearer}),
        ) as served:
            _ = wait_for_generation(api, "development", 1, served, log)
            plan.write_text(json.dumps({"steps": ["pick", "pack"], "release": "0.58.0"}))
            republished: subprocess.CompletedProcess[str] = subprocess.run(
                [str(executable), "location", "republish", "--location-id", location]
                + ["--server", f"http://{address}:{port}", "--target", "development"]
                + ["--reason", "deploy", "--wait", "--timeout", "45"],
                cwd=tmp_path,
                env=environment({"SQLBUILD_API_KEY": key, "NO_PROXY": address}),
                capture_output=True,
                text=True,
                timeout=60,
                check=False,
            )
            assert republished.returncode == test_case.expected_exit_code, republished.stderr
            assert test_case.expected_stderr in republished.stderr
            assert republished.stderr.count("travels unencrypted over HTTP") == 1
            current: dict[str, Any] = current_snapshot(api, "development")
            assert current["record"]["generation"] == test_case.expected_generation, log.read_text()
            readback_request: Request = Request(
                f"{api.url}/api/control/locations/{location}/targets/development/discovered",
                headers={"Authorization": f"Bearer {key}"},
            )
            with urlopen(readback_request, timeout=5) as response:
                readback: dict[str, Any] = json.load(response)
            assert readback["snapshot_id"] == current["record"]["snapshot_id"]
            assert readback["sets"][0]["tags"] == {"release": test_case.expected_release}
            assert key not in republished.stdout + republished.stderr
        assert key not in log.read_text() + (tmp_path / "server.log").read_text()
