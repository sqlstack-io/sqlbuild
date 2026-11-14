from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.debug._test_types import (
    DebugCliTestCase,
    DebugJsonCliTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DebugCliTestCase(
            description="checks duckdb connection by default",
            command=("debug",),
            expected_stdout_fragment=(
                "  connection test: [OK connected]\n  query test: [OK SELECT 1]\n"
            ),
        ),
        DebugCliTestCase(
            description="skips connection when requested",
            command=("debug", "--no-connection"),
            expected_stdout_fragment=(
                "  connection test: [SKIP skipped by --no-connection]\n"
                "  query test: [SKIP connection skipped]\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_debug_command_when_running_then_outputs_checks(
    tmp_path: Path,
    test_case: DebugCliTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="debug_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "debug_project"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "debug.duckdb"\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stderr
    assert result.stdout.startswith("\nSQLBuild Diagnostics\n\n")
    assert "Runtime:\n" in result.stdout
    assert "Configuration:\n" in result.stdout
    assert "Providers:\n" in result.stdout
    assert "Connection:\n" in result.stdout
    assert "  database: " in result.stdout
    assert test_case.expected_stdout_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        DebugCliTestCase(
            description="reports the defaults location models use, not the connection's",
            command=("debug", "--no-connection"),
            expected_stdout_fragment=(
                "  database: DEFAULTS_DB [OK resolved]\n  schema: DEFAULTS_SCHEMA [OK resolved]\n"
            ),
            project_toml=(
                'name = "debug_defaults"\nadapter = "snowflake"\n\n'
                '[connection]\naccount = "acme"\nuser = "u"\npassword = "p"\n'
                'database = "CONN_DB"\nschema = "CONN_SCHEMA"\n\n'
                '[defaults]\ndatabase = "DEFAULTS_DB"\nschema = "DEFAULTS_SCHEMA"\n'
            ),
        ),
        DebugCliTestCase(
            description="a duckdb connection file is not reported as the models' database",
            command=("debug", "--no-connection"),
            expected_stdout_fragment=(
                "  database: not configured [OK resolved]\n  schema: sales [OK resolved]\n"
            ),
            project_toml=(
                'name = "debug_defaults"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "shop.duckdb"\nschema = "sales"\n'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_location_when_debugging_then_reports_where_models_build(
    tmp_path: Path,
    test_case: DebugCliTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="debug_defaults",
        repo_files={"sqlbuild_project.toml": test_case.project_toml},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stderr
    assert test_case.expected_stdout_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        DebugJsonCliTestCase(
            description="outputs machine readable skipped connection checks",
            command=("debug", "--json", "--no-connection"),
            expected_success=True,
            expected_check_tail=[
                {
                    "label": "connection test",
                    "message": "",
                    "status": "SKIP",
                    "status_message": "skipped by --no-connection",
                },
                {
                    "label": "query test",
                    "message": "",
                    "status": "SKIP",
                    "status_message": "connection skipped",
                },
            ],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_debug_json_when_running_then_outputs_machine_readable_checks(
    tmp_path: Path,
    test_case: DebugJsonCliTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="debug_json_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "debug_json_project"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "debug.duckdb"\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert payload["success"] is test_case.expected_success
    connection: list[dict[str, str]] = payload["connection"]  # type: ignore[assignment]
    assert connection[-2:] == test_case.expected_check_tail


@pytest.mark.parametrize(
    "test_case",
    (
        DebugCliTestCase(
            description="explicit target overrides local default in text diagnostics",
            command=("debug", "--target", "test", "--no-connection"),
            expected_stdout_fragment="  target: test [OK resolved]",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_local_default_and_explicit_target_when_debugging_text_then_reports_destination(
    tmp_path: Path,
    test_case: DebugCliTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="debug_target_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "debug_target_project"
                adapter = "duckdb"
                default_target = "dev"

                [connections.local]
                database = "debug.duckdb"

                [targets.dev]
                database = "dev.duckdb"
                schema = "dev_orders"

                [targets.test]
                database = "test.duckdb"
                schema = "test_orders"
                """
            ).strip()
            + "\n",
            "sqlbuild_local.toml": dedent(
                """
                target = "dev"

                [targets.test]
                database = "local-test.duckdb"
                schema = "local_test_orders"
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    assert (
        "  adapter: duckdb [OK found]\n"
        "  target: test [OK resolved]\n"
        "  connection: local [OK resolved]\n"
        "  database: local-test.duckdb [OK resolved]\n"
        "  schema: local_test_orders [OK resolved]\n"
    ) in result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        DebugCliTestCase(
            description="explicit target overrides local default in json diagnostics",
            command=("debug", "--target", "test", "--no-connection", "--json"),
            expected_stdout_fragment='"label": "target", "message": "test"',
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_local_default_and_explicit_target_when_debugging_json_then_reports_destination(
    tmp_path: Path,
    test_case: DebugCliTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="debug_target_json_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "debug_target_json_project"
                adapter = "duckdb"
                default_target = "dev"

                [connections.local]
                database = "debug.duckdb"

                [targets.dev]
                database = "dev.duckdb"
                schema = "dev_orders"

                [targets.test]
                database = "test.duckdb"
                schema = "test_orders"
                """
            ).strip()
            + "\n",
            "sqlbuild_local.toml": dedent(
                """
                target = "dev"

                [targets.test]
                database = "local-test.duckdb"
                schema = "local_test_orders"
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stderr
    assert test_case.expected_stdout_fragment in result.stdout
    payload: dict[str, object] = json.loads(result.stdout)
    configuration: list[dict[str, str]] = payload["configuration"]  # type: ignore[assignment]
    destination: dict[str, str] = {line["label"]: line["message"] for line in configuration}
    assert destination == {
        "project file": str(project_dir / "sqlbuild_project.toml"),
        "local config": str(project_dir / "sqlbuild_local.toml"),
        "project": "debug_target_json_project",
        "adapter": "duckdb",
        "target": "test",
        "connection": "local",
        "database": "local-test.duckdb",
        "schema": "local_test_orders",
    }


@pytest.mark.parametrize(
    "test_case",
    [
        DebugCliTestCase(
            description="reports discovered providers",
            command=("debug", "--no-connection"),
            expected_stdout_fragment=(
                "Providers:\n"
                "  providers: 1 [OK discovered]\n"
                "  marker_provider: providers/marker.py:MarkerProvider [OK valid settings]\n"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_debug_command_with_providers_when_running_then_outputs_provider_summary(
    tmp_path: Path,
    test_case: DebugCliTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="debug_provider_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "debug_provider_project"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "debug_provider.duckdb"\n'
            ),
            "providers/marker.py": dedent(
                """
                from sqlbuild.providers import Provider


                class MarkerProvider(Provider):
                    '''Test provider MarkerProvider.'''
                    label: str = "debug"
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stderr
    assert test_case.expected_stdout_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        DebugJsonCliTestCase(
            description="reports discovered providers in json",
            command=("debug", "--json", "--no-connection"),
            expected_success=True,
            expected_check_tail=[
                {
                    "label": "providers",
                    "message": "1",
                    "status": "OK",
                    "status_message": "discovered",
                },
                {
                    "label": "marker_provider",
                    "message": "providers/marker.py:MarkerProvider",
                    "status": "OK",
                    "status_message": "valid settings",
                },
            ],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_debug_json_with_providers_when_running_then_outputs_provider_summary(
    tmp_path: Path,
    test_case: DebugJsonCliTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="debug_provider_json_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "debug_provider_json_project"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "debug_provider_json.duckdb"\n'
            ),
            "providers/marker.py": dedent(
                """
                from sqlbuild.providers import Provider


                class MarkerProvider(Provider):
                    '''Test provider MarkerProvider.'''
                    label: str = "debug"
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert payload["success"] is test_case.expected_success
    providers: list[dict[str, str]] = payload["providers"]  # type: ignore[assignment]
    assert providers == test_case.expected_check_tail
