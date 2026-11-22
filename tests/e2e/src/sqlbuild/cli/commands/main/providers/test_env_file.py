"""E2E tests for loading provider settings from the project env file."""

from __future__ import annotations

import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.providers._test_types import EnvFileE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)

TOKEN_VARIABLE: str = "ENVFILE_E2E_TOKEN"
REGION_VARIABLE: str = "ENVFILE_E2E_REGION"
# A value that must never reach any output.
TOKEN_VALUE: str = "t0ken-never-printed"
PROJECT_HEADER: str = 'name = "env_file_project"\nadapter = "duckdb"\n'
CONNECTION_TABLE: str = '\n[connection]\ndatabase = "env_file_project.duckdb"\n'
PROVIDER_FILES: dict[str, str] = {
    "providers/settings_api.py": dedent(
        f"""
        from pydantic import Field, SecretStr
        from sqlbuild.providers import Provider


        class SettingsApi(Provider):
            '''Test provider SettingsApi.'''
            token: SecretStr = Field(validation_alias="{TOKEN_VARIABLE}")
            region: str = Field(default="default", validation_alias="{REGION_VARIABLE}")
        """
    ).strip()
    + "\n",
    "python/assets/report_region.py": dedent(
        """
        import os
        from pathlib import Path

        from providers.settings_api import SettingsApi
        from sqlbuild.assets import asset


        @asset
        def report_region(ctx, settings_api: SettingsApi):
            '''Test asset report_region.'''
            Path(os.environ["MARKER_PATH"]).write_text(settings_api.region, encoding="utf-8")
        """
    ).strip()
    + "\n",
}
ENV_FILE: str = (
    f"# provider settings\n{TOKEN_VARIABLE}={TOKEN_VALUE}\n{REGION_VARIABLE}=from-file\n"
)
PRIVATE: int = 0o600
BUILD: tuple[str, ...] = (
    "--no-color",
    "build",
    "--no-tests",
    "--no-audits",
    "--select",
    "report_region",
)


@pytest.mark.parametrize(
    "test_case",
    [
        EnvFileE2ETestCase(
            description="the default .env sets the provider's settings",
            project_toml="",
            files={".env": (ENV_FILE, PRIVATE)},
            command=BUILD,
            env={},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_region="from-file",
            unexpected_output_fragments=(TOKEN_VALUE,),
        ),
        EnvFileE2ETestCase(
            description="the default .env loads when --concurrency is given",
            project_toml="",
            files={".env": (ENV_FILE, PRIVATE)},
            command=(*BUILD, "--concurrency", "2"),
            env={},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_region="from-file",
        ),
        EnvFileE2ETestCase(
            description="the default .env loads when SQLBUILD_CONCURRENCY is set",
            project_toml="",
            files={".env": (ENV_FILE, PRIVATE)},
            command=BUILD,
            env={"SQLBUILD_CONCURRENCY": "2"},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_region="from-file",
        ),
        EnvFileE2ETestCase(
            description="a variable already in the environment wins over the file",
            project_toml="",
            files={".env": (ENV_FILE, PRIVATE)},
            command=BUILD,
            env={REGION_VARIABLE: "from-environment"},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_region="from-environment",
        ),
        EnvFileE2ETestCase(
            description="the selected target's env_file replaces the project default",
            project_toml='\n[targets.uat]\nenv_file = "config/uat.env"\n',
            files={
                ".env": (ENV_FILE, PRIVATE),
                "config/uat.env": (
                    f"{TOKEN_VARIABLE}={TOKEN_VALUE}\n{REGION_VARIABLE}=from-uat\n",
                    PRIVATE,
                ),
            },
            command=(*BUILD, "--target", "uat"),
            env={},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_region="from-uat",
        ),
        EnvFileE2ETestCase(
            description='env_file = "" turns loading off',
            project_toml='env_file = ""\n',
            files={".env": (ENV_FILE, PRIVATE)},
            command=BUILD,
            env={},
            expected_exit_code=1,
            expected_output_fragments=(TOKEN_VARIABLE, "nothing was run"),
            expected_region="",
        ),
        EnvFileE2ETestCase(
            description="a configured env file that does not exist is an error naming it",
            project_toml='env_file = "secrets/missing.env"\n',
            files={},
            command=BUILD,
            env={},
            expected_exit_code=1,
            expected_output_fragments=(
                "env file secrets/missing.env does not exist "
                "(set by env_file in sqlbuild_project.toml)",
            ),
            expected_region="",
        ),
        EnvFileE2ETestCase(
            description="an env file others can read loads with a warning",
            project_toml="",
            files={".env": (ENV_FILE, 0o644)},
            command=BUILD,
            env={},
            expected_exit_code=0,
            expected_output_fragments=(
                "env file .env is readable by group or others (mode 644); "
                "restrict it with chmod 600 .env",
            ),
            expected_region="from-file",
        ),
        EnvFileE2ETestCase(
            description="debug names the file and the variables it set, never their values",
            project_toml="",
            files={".env": (ENV_FILE, PRIVATE)},
            command=("--no-color", "debug", "--no-connection", "--verbose"),
            env={},
            expected_exit_code=0,
            expected_output_fragments=(
                "/.env [OK loaded, 2 variables]\n",
                f"  settings_api: {TOKEN_VARIABLE} [OK set from env file]\n",
            ),
            expected_region="",
            unexpected_output_fragments=(TOKEN_VALUE, "from-file"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_project_env_file_when_running_commands_then_providers_read_it_and_the_environment_wins(
    test_case: EnvFileE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "region.txt"
    _ = marker_path.write_text("", encoding="utf-8")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="env_file_project",
        repo_files={
            "sqlbuild_project.toml": PROJECT_HEADER + test_case.project_toml + CONNECTION_TABLE
        }
        | PROVIDER_FILES,
    )
    relative_path: str
    contents: str
    mode: int
    for relative_path, (contents, mode) in test_case.files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
        path.chmod(mode)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path), **test_case.env},
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_output_fragments:
        assert fragment not in output, output
    assert marker_path.read_text(encoding="utf-8") == test_case.expected_region, output
