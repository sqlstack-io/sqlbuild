"""E2E tests for providers whose secrets are not set."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.providers._test_types import (
    ChangedCantRunTestCase,
    ProjectDirSpellingTestCase,
    ProviderCommandDiagnosticE2ETestCase,
    ProviderSecretE2ETestCase,
    StrictPlanE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.providers.helpers import (
    blocked,
    linked_project,
    relative_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
)

SECRET_VARIABLE: str = "SECRETS_E2E_API_KEY"

PROJECT_TOML: str = (
    'name = "provider_secret_project"\nadapter = "duckdb"\n\n'
    '[connection]\ndatabase = "provider_secret_project.duckdb"\n'
)


MARKING_NODE: str = (
    dedent(
        """
        import os
        from pathlib import Path

        from providers.api import ApiProvider
        from sqlbuild.{decorator}s import {decorator}


        @{decorator}{arguments}
        def {name}(ctx{parameter}):
            '''Test node {name}.'''
            with Path(os.environ["MARKER_PATH"]).open("a", encoding="utf-8") as marker:
                marker.write("{name}\\n")
        """
    ).strip()
    + "\n{body}"
)
USES_API: str = ", api_provider: ApiProvider"


PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": PROJECT_TOML,
    "providers/api.py": dedent(
        f"""
        from pydantic import Field, SecretStr
        from sqlbuild.providers import Provider


        class ApiProvider(Provider):
            '''Test provider ApiProvider.'''
            api_key: SecretStr = Field(validation_alias="{SECRET_VARIABLE}")
            region: str = "eu"
        """
    ).strip()
    + "\n",
    "python/tasks/uses_api.py": MARKING_NODE.format(
        decorator="asset", arguments="", name="uses_api", parameter=USES_API, body=""
    ),
    "python/tasks/no_api.py": MARKING_NODE.format(
        decorator="asset", arguments="", name="no_api", parameter="", body=""
    ),
    "python/checks/api_check.py": MARKING_NODE.format(
        decorator="check",
        arguments="(depends_on=())",
        name="api_check",
        parameter=USES_API,
        body="    return True\n",
    ),
    "python/loaders/raw_api_events.py": MARKING_NODE.format(
        decorator="loader",
        arguments="",
        name="raw_api_events",
        parameter=USES_API,
        body='    return [{"event_id": 1}]\n',
    ),
    "sources/raw.yml": dedent(
        """
        sources:
          - name: raw_api_events
            description: Test source raw_api_events.
            managed: true
            write_strategy: table
            columns:
              - name: event_id
                type: INTEGER
        """
    ).strip()
    + "\n",
}

SECRET_FREE_SINK: dict[str, str] = {
    "providers/event_log.py": dedent(
        """
        from sqlbuild.providers import Provider


        class EventLog(Provider):
            '''Test provider EventLog.'''
            region: str = "eu"
        """
    ).strip()
    + "\n",
    "sinks/events.py": dedent(
        """
        from providers.event_log import EventLog
        from sqlbuild.sinks import LifecycleEvent, lifecycle_event_sink


        @lifecycle_event_sink
        def log_event(event: LifecycleEvent, event_log: EventLog) -> None:
            del event, event_log
        """
    ).strip()
    + "\n",
}
API_SINK: dict[str, str] = {
    "sinks/events.py": dedent(
        """
        from providers.api import ApiProvider
        from sqlbuild.sinks import LifecycleEvent, lifecycle_event_sink


        @lifecycle_event_sink
        def export_event(event: LifecycleEvent, api_provider: ApiProvider) -> None:
            del event, api_provider
        """
    ).strip()
    + "\n",
}
READING_MODEL: dict[str, str] = {
    "models/api_events.sql": (
        "MODEL (description 'Test model api_events.', materialized table);\n\n"
        'SELECT event_id FROM __source("raw_api_events")\n'
    ),
}
TARGET_ALLOWS_API: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "provider_secret_project"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "provider_secret_project.duckdb"\n\n'
        '[targets.dev]\nschema = "main"\nallow_missing_secrets = ["api_provider"]\n'
    ),
}
STRICT_TARGET: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "provider_secret_project"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "provider_secret_project.duckdb"\n\n'
        '[targets.dev]\nschema = "main"\n'
    ),
}
SET_SECRET_VARIABLE: str = "SECRETS_E2E_SIGNING_KEY"
SET_SECRET_VALUE: str = "signing-key-canary"
# One secret is missing and one is set, and setting up the provider leaves a mark.
MARKING_PROVIDER: dict[str, str] = {
    "providers/api.py": dedent(
        f"""
        import os
        from pathlib import Path

        from pydantic import Field, SecretStr
        from sqlbuild.providers import Provider


        class ApiProvider(Provider):
            '''Test provider ApiProvider.'''
            api_key: SecretStr = Field(validation_alias="{SECRET_VARIABLE}")
            signing_key: SecretStr = Field(validation_alias="{SET_SECRET_VARIABLE}")

            def setup(self, ctx):
                with Path(os.environ["MARKER_PATH"]).open("a", encoding="utf-8") as marker:
                    marker.write("setup\\n")
        """
    ).strip()
    + "\n",
}
REQUIRED_SETTING: dict[str, str] = {
    "providers/region.py": (
        "from pydantic import Field\n"
        "from sqlbuild.providers import Provider\n\n\n"
        "class RegionProvider(Provider):\n"
        '    """Test provider RegionProvider."""\n'
        '    region_code: str = Field(validation_alias="SECRETS_E2E_REGION")\n'
    ),
}
POST_BUILD_CHECK: dict[str, str] = {
    "python/checks/no_api_check.py": dedent(
        """
        import os
        from pathlib import Path

        from providers.api import ApiProvider
        from python.tasks.no_api import no_api
        from sqlbuild.checks import check


        @check(depends_on=(no_api,))
        def no_api_check(ctx, api_provider: ApiProvider):
            '''Test check no_api_check.'''
            with Path(os.environ["MARKER_PATH"]).open("a", encoding="utf-8") as marker:
                marker.write("no_api_check\\n")
            return True
        """
    ).strip()
    + "\n",
}

MISSING_ERROR: str = (
    f"provider 'api_provider' (providers/api.py) is missing secret {SECRET_VARIABLE}"
)
ALLOWED_NOTICE: str = "1 provider has missing secrets (allowed): api_provider"
SELECTION_ERROR: str = (
    "selected nodes need secrets that are missing from the environment and from .env:"
)
# Lines a build prints once it starts connecting to the warehouse.
CONNECTING: tuple[str, ...] = ("Connecting to", "Warehouse connected")
UNBOUND_NOTICE: str = (
    f"sink 'export_event' is not bound: provider 'api_provider' is missing {SECRET_VARIABLE}"
)
BUILD_WITH_CHECKS: tuple[str, ...] = ("--no-color", "build", "--no-tests", "--select")
BUILD: tuple[str, ...] = ("--no-color", "build", "--no-tests", "--no-audits", "--select")


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderSecretE2ETestCase(
            description="compile fails and names the variable",
            command=("--no-color", "compile"),
            env={},
            extra_repo_files={},
            expected_exit_code=1,
            expected_output_fragments=(MISSING_ERROR, "--allow-missing-secrets"),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="compile passes with the flag",
            command=("--no-color", "compile", "--allow-missing-secrets"),
            env={},
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=(ALLOWED_NOTICE,),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="compile passes with the environment variable",
            command=("--no-color", "compile"),
            env={"SQLBUILD_ALLOW_MISSING_SECRETS": "true"},
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=(ALLOWED_NOTICE,),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="compile passes with the local setting",
            command=("--no-color", "compile"),
            env={},
            extra_repo_files={"sqlbuild_local.toml": "[settings]\nallow_missing_secrets = true\n"},
            expected_exit_code=0,
            expected_output_fragments=(ALLOWED_NOTICE,),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="build of a selection that does not use the provider runs",
            command=(*BUILD, "no_api"),
            env={},
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_marker_entries=("no_api",),
        ),
        ProviderSecretE2ETestCase(
            description="build of a selection that uses the provider runs nothing",
            command=(*BUILD, "no_api", "uses_api"),
            env={},
            extra_repo_files={},
            expected_exit_code=1,
            expected_output_fragments=(SELECTION_ERROR, SECRET_VARIABLE, "nothing was run"),
            expected_marker_entries=(),
            unexpected_output_fragments=CONNECTING,
        ),
        ProviderSecretE2ETestCase(
            description="build stops before the warehouse and names the nodes that read the blocked one",
            command=(*BUILD, "+api_events"),
            env={},
            extra_repo_files=READING_MODEL,
            expected_exit_code=1,
            expected_output_fragments=(
                SELECTION_ERROR,
                f"api_provider  {SECRET_VARIABLE}  (raw_api_events; read by api_events)",
                "nothing was run",
            ),
            expected_marker_entries=(),
            unexpected_output_fragments=CONNECTING,
        ),
        ProviderSecretE2ETestCase(
            description="compile passes when the target allows the provider's missing secrets",
            command=("--no-color", "compile"),
            env={},
            extra_repo_files=TARGET_ALLOWS_API,
            expected_exit_code=0,
            expected_output_fragments=(
                "1 provider has missing secrets (allowed on target dev): api_provider",
            ),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="build still stops nodes whose provider the target allows to miss secrets",
            command=(*BUILD, "uses_api"),
            env={},
            extra_repo_files=TARGET_ALLOWS_API,
            expected_exit_code=1,
            expected_output_fragments=(SELECTION_ERROR, "nothing was run"),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="a required setting that isn't a secret type is a missing secret",
            command=("--no-color", "compile"),
            env={SECRET_VARIABLE: "token"},
            extra_repo_files=REQUIRED_SETTING,
            expected_exit_code=1,
            expected_output_fragments=(
                "provider 'region_provider' (providers/region.py) is missing secret SECRETS_E2E_REGION",
            ),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="a missing required setting is allowed like any secret",
            command=("--no-color", "compile", "--allow-missing-secrets"),
            env={SECRET_VARIABLE: "token"},
            extra_repo_files=REQUIRED_SETTING,
            expected_exit_code=0,
            expected_output_fragments=(
                "1 provider has missing secrets (allowed): region_provider",
            ),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="build of a selection that uses the provider runs once the secret is set",
            command=(*BUILD, "uses_api"),
            env={SECRET_VARIABLE: "token"},
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_marker_entries=("uses_api",),
        ),
        ProviderSecretE2ETestCase(
            description="check that uses the provider runs nothing",
            command=("--no-color", "check", "--select", "api_check"),
            env={},
            extra_repo_files={},
            expected_exit_code=1,
            expected_output_fragments=(SELECTION_ERROR, SECRET_VARIABLE, "nothing was run"),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="load that uses the provider runs nothing",
            command=("--no-color", "load", "--select", "raw_api_events"),
            env={},
            extra_repo_files={},
            expected_exit_code=1,
            expected_output_fragments=(SELECTION_ERROR, SECRET_VARIABLE, "nothing was run"),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="build whose post-build check uses the provider runs nothing",
            command=(*BUILD_WITH_CHECKS, "no_api"),
            env={},
            extra_repo_files=POST_BUILD_CHECK,
            expected_exit_code=1,
            expected_output_fragments=(SELECTION_ERROR, SECRET_VARIABLE, "nothing was run"),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="build whose post-build check uses the provider runs it once the secret is set",
            command=(*BUILD_WITH_CHECKS, "no_api"),
            env={SECRET_VARIABLE: "token"},
            extra_repo_files=POST_BUILD_CHECK,
            expected_exit_code=0,
            expected_output_fragments=("Completed successfully",),
            expected_marker_entries=("no_api", "no_api_check"),
        ),
        ProviderSecretE2ETestCase(
            description="compile with a sink still fails on another provider's missing secret",
            command=("--no-color", "compile"),
            env={},
            extra_repo_files=SECRET_FREE_SINK,
            expected_exit_code=1,
            expected_output_fragments=(MISSING_ERROR,),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="compile with a sink and the flag notes the allowed provider",
            command=("--no-color", "compile", "--allow-missing-secrets"),
            env={},
            extra_repo_files=SECRET_FREE_SINK,
            expected_exit_code=0,
            expected_output_fragments=(ALLOWED_NOTICE,),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="compile whose sink's provider is missing the secret fails with the coded error",
            command=("--no-color", "compile"),
            env={},
            extra_repo_files=API_SINK,
            expected_exit_code=1,
            expected_output_fragments=(MISSING_ERROR,),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="compile with the flag leaves the sink unbound",
            command=("--no-color", "compile", "--allow-missing-secrets"),
            env={},
            extra_repo_files=API_SINK,
            expected_exit_code=0,
            expected_output_fragments=(UNBOUND_NOTICE, ALLOWED_NOTICE),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="build whose sink's provider is missing the secret runs nothing",
            command=(*BUILD, "no_api"),
            env={},
            extra_repo_files=API_SINK,
            expected_exit_code=1,
            expected_output_fragments=(
                SELECTION_ERROR,
                f"sink export_event needs {SECRET_VARIABLE}; every run uses sinks",
                "nothing was run",
            ),
            expected_marker_entries=(),
            unexpected_output_fragments=("(sink:export_event", "(export_event)"),
        ),
        ProviderSecretE2ETestCase(
            description="plan whose sink's provider is missing the secret can't run at all",
            command=("--no-color", "plan", "--select", "no_api"),
            env={},
            extra_repo_files=API_SINK | TARGET_ALLOWS_API,
            expected_exit_code=0,
            expected_output_fragments=(
                f"Can't run: sink export_event needs {SECRET_VARIABLE}; every run uses sinks",
            ),
            expected_marker_entries=(),
            unexpected_output_fragments=("needs  export_event",),
        ),
        ProviderSecretE2ETestCase(
            description="debug with a sink lists the missing secret",
            command=("--no-color", "debug", "--no-connection"),
            env={},
            extra_repo_files=API_SINK,
            expected_exit_code=0,
            expected_output_fragments=(f"  api_provider: {SECRET_VARIABLE} [WARN missing]\n",),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="debug lists the missing secret",
            command=("--no-color", "debug", "--no-connection"),
            env={},
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=(
                "  providers: 0 of 1 ready [WARN missing secrets]\n"
                f"  api_provider: {SECRET_VARIABLE} [WARN missing]\n",
            ),
            expected_marker_entries=(),
        ),
        ProviderSecretE2ETestCase(
            description="verbose debug lists set secrets",
            command=("--no-color", "debug", "--no-connection", "--verbose"),
            env={SECRET_VARIABLE: "token"},
            extra_repo_files={},
            expected_exit_code=0,
            expected_output_fragments=(
                "  providers: 1 of 1 ready [OK secrets set]\n"
                f"  api_provider: {SECRET_VARIABLE} [OK set]\n",
            ),
            expected_marker_entries=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_provider_secret_unset_when_running_commands_then_only_users_of_it_fail(
    test_case: ProviderSecretE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "marker.log"
    marker_path.write_text("", encoding="utf-8")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_secret_project",
        repo_files={**PROJECT_FILES, **test_case.extra_repo_files},
    )

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
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderCommandDiagnosticE2ETestCase(
            description="the project file cannot allow missing secrets",
            command=("--no-color", "compile"),
            expected_error_fragment="cannot set [settings] allow_missing_secrets",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_file_allows_missing_secrets_when_compiling_then_fails(
    test_case: ProviderCommandDiagnosticE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_secret_project",
        repo_files={
            **PROJECT_FILES,
            "sqlbuild_project.toml": PROJECT_TOML + "\n[settings]\nallow_missing_secrets = true\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode != 0
    assert test_case.expected_error_fragment in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectDirSpellingTestCase(
            description="a symlinked project names its env file relative to the project",
            project_argument=linked_project,
            expected_output_fragment=SELECTION_ERROR,
        ),
        ProjectDirSpellingTestCase(
            description="a relative project directory names its env file relative to the project",
            project_argument=relative_project,
            expected_output_fragment=SELECTION_ERROR,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_project_dir_spelling_when_a_secret_is_missing_then_the_env_file_shows_as_dot_env(
    test_case: ProjectDirSpellingTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_secret_project",
        repo_files={**PROJECT_FILES, ".env": "UNRELATED_SETTING=1\n"},
    )
    (project_dir / ".env").chmod(0o600)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(*BUILD, "uses_api"),
        project_dir=test_case.project_argument(project_dir),
        env={"MARKER_PATH": str(tmp_path / "marker.log")},
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, output
    assert test_case.expected_output_fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedCantRunTestCase(
            description="an unchanged reader that --changed skips is not counted as can't run",
            select="+api_events",
            expected_cant_run_nodes=["source:raw_api_events"],
            expected_cant_run_readers=[],
            expected_unchanged=["model:api_events"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_changed_plan_when_a_secret_is_missing_then_only_nodes_it_runs_cant_run(
    test_case: ChangedCantRunTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_secret_project",
        repo_files={**PROJECT_FILES, **READING_MODEL, **TARGET_ALLOWS_API},
    )
    marker: dict[str, str] = {"MARKER_PATH": str(tmp_path / "marker.log")}
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=(*BUILD, test_case.select),
        project_dir=project_dir,
        env=marker | {SECRET_VARIABLE: "token"},
    )
    assert built.returncode == 0, built.stdout + built.stderr

    planned: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json", "--changed", "--select", test_case.select),
        project_dir=project_dir,
        env=marker,
    )

    assert planned.returncode == 0, planned.stderr
    document: dict[str, object] = json.loads(planned.stdout)
    changed: dict[str, list[str]] = document["changed"]  # type: ignore[assignment]
    cant_run: list[dict[str, list[str]]] = document["cant_run"]  # type: ignore[assignment]
    assert changed["unchanged"] == test_case.expected_unchanged, planned.stdout
    assert [entry["nodes"] for entry in cant_run] == [test_case.expected_cant_run_nodes]
    assert [entry["readers"] for entry in cant_run] == [test_case.expected_cant_run_readers]


@pytest.mark.parametrize(
    "test_case",
    [
        StrictPlanE2ETestCase(
            description="a selected node uses the provider",
            select="+api_events",
            extra_repo_files=READING_MODEL,
            expected_block=(
                f"Can't run: provider api_provider needs {SECRET_VARIABLE} "
                "(missing from the environment and from .env)\n"
                "  needs  raw_api_events, and api_events that read it\n"
            ),
            expected_cant_run=[
                {
                    "provider": "api_provider",
                    "variables": [SECRET_VARIABLE],
                    "nodes": ["source:raw_api_events"],
                    "readers": ["model:api_events"],
                    "sinks": [],
                }
            ],
        ),
        StrictPlanE2ETestCase(
            description="only an unselected node uses the provider",
            select="no_api",
            extra_repo_files={},
            expected_block="",
            expected_cant_run=[],
        ),
        StrictPlanE2ETestCase(
            description="a sink uses the provider",
            select="no_api",
            extra_repo_files=API_SINK,
            expected_block=(
                f"Can't run: sink export_event needs {SECRET_VARIABLE}; every run uses sinks "
                "(missing from the environment and from .env)\n"
            ),
            expected_cant_run=[
                {
                    "provider": "api_provider",
                    "variables": [SECRET_VARIABLE],
                    "nodes": [],
                    "readers": [],
                    "sinks": ["export_event"],
                }
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_strict_target_when_planning_without_a_secret_then_the_plan_lists_what_cant_run(
    test_case: StrictPlanE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "marker.log"
    marker_path.write_text("", encoding="utf-8")
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_secret_project",
        repo_files={
            **PROJECT_FILES,
            **STRICT_TARGET,
            **MARKING_PROVIDER,
            **test_case.extra_repo_files,
        },
    )
    env: dict[str, str] = {"MARKER_PATH": str(marker_path), SET_SECRET_VARIABLE: SET_SECRET_VALUE}
    plan: tuple[str, ...] = ("--no-color", "plan", "--select", test_case.select)

    text: subprocess.CompletedProcess[str] = run_sqb(command=plan, project_dir=project_dir, env=env)
    as_json: subprocess.CompletedProcess[str] = run_sqb(
        command=(*plan, "--json"), project_dir=project_dir, env=env
    )

    shown: str = text.stdout + text.stderr + as_json.stdout + as_json.stderr
    assert (text.returncode, as_json.returncode) == (0, 0), shown
    _, marked, block = text.stdout.partition("Can't run:")
    assert marked + block == test_case.expected_block, shown
    document: dict[str, object] = json.loads(as_json.stdout)
    cant_run: list[dict[str, object]] = document.get("cant_run", [])  # type: ignore[assignment]
    assert [blocked(entry) for entry in cant_run] == test_case.expected_cant_run
    assert SET_SECRET_VALUE not in shown
    assert marker_path.read_text(encoding="utf-8") == ""
