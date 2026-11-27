from __future__ import annotations

from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

import sqlbuild._native as dbt_init_module
from sqlbuild._native import DbtInitCommandRequest
from sqlbuild.integrations.dbt.exceptions import DbtProfileError
from tests.unit.src.sqlbuild.cli.commands.main.dbt._test_types import (
    DbtInitOutputTestCase,
    DbtInitProjectDirDefaultTestCase,
    DbtInitValidationOrderTestCase,
)
from tests.unit.src.sqlbuild.integrations.dbt.profile.helpers import write_duckdb_dbt_workspace
from tests.unit.src.sqlbuild.presentation.helpers import use_color_terminal


@pytest.mark.parametrize(
    "test_case",
    [
        DbtInitOutputTestCase(
            description="init outputs progress and dbt-first next steps",
            dry_run=False,
            expected_fragments=(
                "Inspecting dbt project and profile...",
                "dbt project and profile inspected",
                "SQLBuild project created",
                "Setup summary:",
                "Config file",
                "sqlbuild_project/sqlbuild_project.toml",
                "What SQLBuild created:",
                "SQLBuild twin config",
                "This points SQLBuild at your dbt project and profile.",
                "sqb dbt debug",
                "sqb dbt build",
            ),
            unexpected_fragments=("Add SQLBuild models", "Production git ref"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_init_when_running_then_outputs_progress_and_dbt_first_next_steps(
    test_case: DbtInitOutputTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_duckdb_dbt_workspace(tmp_path)

    exit_code: int = dbt_init_module.run_dbt_init_command(
        DbtInitCommandRequest(
            cwd=tmp_path,
            dbt_project_dir="dbt_project",
            profiles_dir="profiles",
            profile_name=None,
            target_name=None,
            sqb_output_dir="sqlbuild_project",
            dry_run=test_case.dry_run,
            overwrite=False,
            skip_dbt_debug=True,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    for fragment in test_case.expected_fragments:
        assert fragment in captured.out
    for fragment in test_case.unexpected_fragments:
        assert fragment not in captured.out


@pytest.mark.parametrize(
    "test_case",
    [
        DbtInitOutputTestCase(
            description="dry-run outputs progress and preview document",
            dry_run=True,
            expected_fragments=(
                "Rendering dbt profile connection...",
                "Rendered dbt profile connection.",
                "SQLBuild project preview",
                "Generated config:",
                'source = "dbt_profile"',
            ),
            unexpected_fragments=("Production git ref", "Production schema macro"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_init_dry_run_when_running_then_outputs_preview_document(
    test_case: DbtInitOutputTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_duckdb_dbt_workspace(tmp_path)

    exit_code: int = dbt_init_module.run_dbt_init_command(
        DbtInitCommandRequest(
            cwd=tmp_path,
            dbt_project_dir="dbt_project",
            profiles_dir="profiles",
            profile_name=None,
            target_name=None,
            sqb_output_dir="sqlbuild_project",
            dry_run=test_case.dry_run,
            overwrite=False,
            skip_dbt_debug=True,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    for fragment in test_case.expected_fragments:
        assert fragment in captured.out
    for fragment in test_case.unexpected_fragments:
        assert fragment not in captured.out


@pytest.mark.parametrize(
    "test_case",
    [
        DbtInitOutputTestCase(
            description="init output uses SQLBuild colors when terminal supports color",
            dry_run=False,
            expected_fragments=(),
            expected_color_fragments=(
                "\033[32m⠋\033[0m Inspecting dbt project and profile...",
                "\033[34m\033[1mSQLBuild project created\033[0m",
                "\033[1mSetup summary\033[0m:",
                "\033[34mProject",
                "shop",
                "\033[1mWhat SQLBuild created\033[0m:",
                "\033[34mSQLBuild twin config\033[0m",
                "\033[1mNext steps\033[0m:",
                "\033[33mReview the config file above.\033[0m",
                "  3. \033[2msqb dbt debug\033[0m",
                "  4. \033[2msqb dbt build\033[0m",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_color_terminal_when_running_dbt_init_then_it_styles_output(
    test_case: DbtInitOutputTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_duckdb_dbt_workspace(tmp_path)
    use_color_terminal(monkeypatch)

    exit_code: int = dbt_init_module.run_dbt_init_command(
        DbtInitCommandRequest(
            cwd=tmp_path,
            dbt_project_dir="dbt_project",
            profiles_dir="profiles",
            profile_name=None,
            target_name=None,
            sqb_output_dir="sqlbuild_project",
            dry_run=test_case.dry_run,
            overwrite=False,
            skip_dbt_debug=True,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    for fragment in test_case.expected_color_fragments:
        assert fragment in captured.out


@pytest.mark.parametrize(
    "test_case",
    [
        DbtInitProjectDirDefaultTestCase(
            description="defaults project dir to cwd when dbt project file exists",
            dbt_project_text="name: analytics\nprofile: analytics\n",
            expected_config_line='project_dir = "../{cwd}"',
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_project_in_cwd_when_running_dbt_init_then_project_dir_defaults_to_cwd(
    test_case: DbtInitProjectDirDefaultTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "dbt_project.yml").write_text(test_case.dbt_project_text, encoding="utf-8")
    (tmp_path / "profiles.yml").write_text(
        "analytics:\n  target: dev\n  outputs:\n    dev:\n      type: duckdb\n"
        "      path: warehouse.duckdb\n      schema: main\n",
        encoding="utf-8",
    )
    monkeypatch.delenv("DBT_PROFILES_DIR", raising=False)

    exit_code: int = dbt_init_module.run_dbt_init_command(
        DbtInitCommandRequest(
            cwd=tmp_path,
            dbt_project_dir=None,
            profiles_dir=None,
            profile_name=None,
            target_name=None,
            sqb_output_dir=None,
            dry_run=True,
            overwrite=False,
            skip_dbt_debug=True,
        )
    )

    assert exit_code == 0
    assert test_case.expected_config_line.format(cwd=tmp_path.name) in capsys.readouterr().out


@pytest.mark.parametrize(
    "test_case",
    [
        DbtInitValidationOrderTestCase(
            description="validates init inputs before running init",
            expected_error_fragment="profiles.yml",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_dbt_init_inputs_when_running_then_validation_happens_before_init(
    test_case: DbtInitValidationOrderTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "dbt_project.yml").write_text(
        "name: analytics\nprofile: analytics\n", encoding="utf-8"
    )
    monkeypatch.setenv("DBT_PROFILES_DIR", str(tmp_path / "profiles"))

    with pytest.raises(DbtProfileError, match=test_case.expected_error_fragment):
        dbt_init_module.run_dbt_init_command(
            DbtInitCommandRequest(
                cwd=tmp_path,
                dbt_project_dir=".",
                profiles_dir=None,
                profile_name=None,
                target_name=None,
                sqb_output_dir="sqlbuild_project",
                dry_run=False,
                overwrite=False,
                skip_dbt_debug=True,
            )
        )

    assert not (tmp_path / "sqlbuild_project").exists()
