"""Installed login persists a bounded person session, reloads it, and observes live revocation."""

import json
import shutil
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CliCleanupTestCase,
    CliLoginTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.login.helpers import (
    assert_private_storage,
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


@pytest.mark.skipif(
    sys.platform != "linux", reason="Linux owner-only fallback acceptance requires a Linux runtime"
)
@pytest.mark.parametrize(
    "test_case",
    [
        CliLoginTestCase(
            "installed HTTPS password handoff and credential lifecycle", "password", "orders-reader"
        ),
        CliLoginTestCase(
            "installed HTTPS OIDC handoff and credential lifecycle", "oidc", "orders-reader"
        ),
        CliLoginTestCase(
            "installed HTTPS proxy handoff and credential lifecycle", "proxy", "orders-reader"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_installed_browser_login_when_reloaded_revoked_and_logged_out_then_person_session_and_storage_boundaries_hold(
    test_case: CliLoginTestCase,
    tmp_path: Path,
) -> None:
    binary: str | None = shutil.which("sqb")
    assert binary is not None
    installation: tuple[Path, Path] = Path(binary), tmp_path
    home: Path = tmp_path / "person-home"
    home.mkdir()
    project: Path = tmp_path / "empty-project"
    project.mkdir()
    fixture: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/cli_login.json").read_text()
    )
    successful(administer(installation, ["create-user", "orders-admin"]))
    person: dict[str, Any] = successful(
        administer(installation, ["create-principal", test_case.expected_person_name])
    )
    successful(
        administer(installation, ["set-password", person["principal_id"]], fixture["password"])
    )
    client_environment: dict[str, str] = environment({"HOME": str(home)})
    client_environment.pop("DBUS_SESSION_BUS_ADDRESS", None)
    client_environment.pop("XDG_RUNTIME_DIR", None)
    with gateway(tmp_path) as (origin, certificate, transport):
        transport.update({"proxy-subject": "orders-person"})
        arguments: tuple[str, ...] = authentication_arguments(
            test_case.authentication, origin, certificate, person["principal_id"]
        )
        with serve(installation, authentication_arguments=arguments) as backend:
            transport["backend"] = backend.removeprefix("http://")
            client, cookies = browser(origin, certificate)
            csrf: str = sign_in(
                client, origin, test_case.authentication, person["name"], fixture["password"]
            )
            assert request(client, origin + "/api/control/audit")[0] == 403
            common: list[str] = ["--server", origin, "--ca-cert", str(certificate)]
            log: Path = tmp_path / "cli-login.log"
            with pending_login(
                [binary, "login", *common, "--no-browser"], client_environment, project, log
            ) as (process, url):
                assert url.startswith(f"{origin}/auth/cli#")
                status, payload = request(client, url.split("#")[0])
                assert status == 200 and b"Authorize CLI" in payload
                status, payload = request(
                    client,
                    f"{origin}/api/auth/cli/approve",
                    {
                        "browser_code": url.split("#", 1)[1],
                        "approve": True,
                    },
                    csrf=csrf,
                )
                assert status == 204 and payload == b""
                stdout, _ = process.communicate(timeout=20)
                assert process.returncode == 0, "CLI handoff did not complete"
                assert not stdout and "signed in successfully" in log.read_text()
            assert not list(project.iterdir())
            credential_files: list[Path] = list(
                (home / ".config/sqlbuild/credentials").glob("*.json")
            )
            assert len(credential_files) == 1
            assert stat.S_IMODE(credential_files[0].stat().st_mode) == 0o600
            assert stat.S_IMODE(credential_files[0].parent.stat().st_mode) == 0o700
            saved: dict[str, Any] = json.loads(credential_files[0].read_text())
            assert (
                saved["bearer"] not in log.read_text()
                and saved["csrf_token"] not in log.read_text()
            )
            assert (
                saved["identity"]["session_id"]
                != json.loads(request(client, f"{origin}/api/auth/session")[1])["session_id"]
            )
            checked: subprocess.CompletedProcess[str] = subprocess.run(
                [binary, "login", "status", *common],
                cwd=project,
                env=client_environment,
                text=True,
                capture_output=True,
                timeout=20,
                check=False,
            )
            assert checked.returncode == 0
            assert json.loads(checked.stdout)["principal_id"] == person["principal_id"]
            assert "bearer" not in checked.stdout and "csrf_token" not in checked.stdout
            assert_private_storage(
                credential_files[0],
                [binary, "login", "status", *common],
                client_environment,
                project,
            )
            successful(
                administer(
                    installation, ["set-password", person["principal_id"]], fixture["replacement"]
                )
            )
            revoked: subprocess.CompletedProcess[str] = subprocess.run(
                [binary, "login", "status", *common],
                cwd=project,
                env=client_environment,
                text=True,
                capture_output=True,
                timeout=20,
                check=False,
            )
            assert revoked.returncode != 0 and not revoked.stdout and "revoked" in revoked.stderr
            for logout_arguments, expected_exit in [
                (["--server", origin, "--ca-cert", str(tmp_path / "missing-ca.pem")], 1),
                (common, 0),
            ]:
                logout: subprocess.CompletedProcess[str] = subprocess.run(
                    [binary, "logout", *logout_arguments],
                    cwd=project,
                    env=client_environment,
                    text=True,
                    capture_output=True,
                    timeout=20,
                    check=False,
                )
                assert logout.returncode == expected_exit and not logout.stdout
                assert not list(credential_files[0].parent.iterdir())
            assert not list(project.iterdir())
            assert len(list(cookies)) > 0


@pytest.mark.skipif(sys.platform != "linux", reason="Linux fallback path regression")
@pytest.mark.parametrize(
    "test_case",
    [
        CliCleanupTestCase(
            "absent fallback permits HTTPS logout", "https://control.example.com", 0
        ),
        CliCleanupTestCase("plaintext origin is still refused", "http://control.example.com", 1),
    ],
    ids=lambda case: case.description,
)
def test_given_absent_fallback_when_configuration_is_symlinked_then_logout_does_not_create_or_validate_unused_storage(
    test_case: CliCleanupTestCase,
    tmp_path: Path,
) -> None:
    binary: str | None = shutil.which("sqb")
    assert binary is not None
    home: Path = tmp_path / "home"
    home.mkdir()
    configuration: Path = tmp_path / "configuration"
    configuration.mkdir()
    (home / ".config").symlink_to(configuration, target_is_directory=True)
    values: dict[str, str] = environment({"HOME": str(home)})
    values.pop("DBUS_SESSION_BUS_ADDRESS", None)
    values.pop("XDG_RUNTIME_DIR", None)
    logout: subprocess.CompletedProcess[str] = subprocess.run(
        [binary, "logout", "--server", test_case.server],
        cwd=tmp_path,
        env=values,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    assert logout.returncode == test_case.expected_exit and not logout.stdout
    assert not list(configuration.iterdir())


@pytest.mark.skipif(sys.platform != "linux", reason="Linux fallback permission failure")
@pytest.mark.parametrize(
    "test_case",
    [
        CliLoginTestCase(
            "readable credential still revokes remotely when local unlink fails",
            "password",
            "orders-reader",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_live_cli_session_when_local_unlink_fails_then_logout_still_revokes_remotely(
    test_case: CliLoginTestCase,
    tmp_path: Path,
) -> None:
    binary: str | None = shutil.which("sqb")
    assert binary is not None
    installation: tuple[Path, Path] = Path(binary), tmp_path
    home: Path = tmp_path / "home"
    home.mkdir()
    values: dict[str, str] = environment({"HOME": str(home)})
    values.pop("DBUS_SESSION_BUS_ADDRESS", None)
    values.pop("XDG_RUNTIME_DIR", None)
    successful(administer(installation, ["create-user", "orders-admin"]))
    person: dict[str, Any] = successful(
        administer(installation, ["create-principal", test_case.expected_person_name])
    )
    successful(
        administer(
            installation, ["set-password", person["principal_id"]], "orders-private-password"
        )
    )
    with gateway(tmp_path) as (origin, certificate, transport):
        arguments: tuple[str, ...] = authentication_arguments(
            test_case.authentication, origin, certificate, person["principal_id"]
        )
        with serve(installation, authentication_arguments=arguments) as backend:
            transport["backend"] = backend.removeprefix("http://")
            browser_client, _ = browser(origin, certificate)
            csrf: str = sign_in(
                browser_client,
                origin,
                test_case.authentication,
                person["name"],
                "orders-private-password",
            )
            common: list[str] = ["--server", origin, "--ca-cert", str(certificate)]
            with pending_login(
                [binary, "login", *common, "--no-browser"], values, tmp_path, tmp_path / "cli.log"
            ) as (process, url):
                assert (
                    request(
                        browser_client,
                        origin + "/api/auth/cli/approve",
                        {"browser_code": url.split("#", 1)[1], "approve": True},
                        csrf=csrf,
                    )[0]
                    == 204
                )
                process.communicate(timeout=20)
                assert process.returncode == 0
            credential: Path = next((home / ".config/sqlbuild/credentials").glob("*.json"))
            saved: dict[str, Any] = json.loads(credential.read_text())
            cli_client, _ = browser(origin, certificate)
            cli_client.addheaders = [("Authorization", f"Session {saved['bearer']}")]
            assert request(cli_client, origin + "/api/auth/session")[0] == 200
            credential.parent.chmod(0o500)
            try:
                logout: subprocess.CompletedProcess[str] = subprocess.run(
                    [binary, "logout", *common],
                    cwd=tmp_path,
                    env=values,
                    text=True,
                    capture_output=True,
                    timeout=20,
                    check=False,
                )
                assert logout.returncode != 0 and not logout.stdout
                assert credential.exists()
                assert request(cli_client, origin + "/api/auth/session")[0] == 401
            finally:
                credential.parent.chmod(0o700)
