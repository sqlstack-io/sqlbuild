from __future__ import annotations

from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

import sqlbuild._native as native_module
from sqlbuild._native import run_init_command
from tests.unit.src.sqlbuild.cli.commands.main.init._test_types import InitScaffoldTestCase
from tests.unit.src.sqlbuild.presentation.helpers import use_color_terminal, use_plain_terminal


@pytest.mark.parametrize(
    "test_case",
    [
        InitScaffoldTestCase(
            description="creates blank project directories for all resource types",
            project_dir_name="demo-project",
            expected_directories=(
                "models/staging",
                "models/marts",
                "schemas",
                "sources",
                "seeds",
                "python",
                "hooks/sql",
                "hooks/python",
                "tests/unit",
                "tests/scenarios",
                "functions/sql",
                "functions/python",
                "macros/sql",
                "macros/python",
                "audits/generic",
                "audits/singular",
            ),
            expected_gitkeep_files=(
                "models/staging/.gitkeep",
                "models/marts/.gitkeep",
                "schemas/.gitkeep",
                "sources/.gitkeep",
                "seeds/.gitkeep",
                "python/.gitkeep",
                "hooks/sql/.gitkeep",
                "hooks/python/.gitkeep",
                "tests/unit/.gitkeep",
                "tests/scenarios/.gitkeep",
                "functions/sql/.gitkeep",
                "functions/python/.gitkeep",
                "macros/sql/.gitkeep",
                "macros/python/.gitkeep",
                "audits/generic/.gitkeep",
                "audits/singular/.gitkeep",
            ),
            expected_config_fragment=(
                '[connections.developer]\ndatabase = "demo_project.duckdb"\n\n[settings]'
            ),
            expected_stdout_fragments=(
                "SQLBuild project created",
                "Project: demo_project",
                "Config:  sqlbuild_project.toml",
                "Next steps:",
                "1. Add sources to sources/",
                "2. Add seeds to seeds/",
                "3. Add Python loaders, assets, checks, and factories to python/",
                "4. Add SQL hooks to hooks/sql/, Python hooks to hooks/python/",
                "reusable audits to audits/generic/",
                "standalone audits to audits/singular/",
                "7. sqb compile",
                "8. sqb build",
            ),
            expected_color_fragments=(
                "\033[34m\033[1mSQLBuild project created\033[0m",
                "  \033[34mProject\033[0m: demo_project",
                "\033[1mNext steps\033[0m:",
                "  7. \033[2msqb compile\033[0m",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_empty_directory_when_running_init_then_scaffolds_project_directories(
    test_case: InitScaffoldTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / test_case.project_dir_name
    project_dir.mkdir()
    monkeypatch.setattr(native_module, "update_sqlbuild_skills", lambda *, project_dir: None)
    use_plain_terminal(monkeypatch)

    result: int = run_init_command(project_dir)

    captured: CaptureResult[str] = capsys.readouterr()
    assert result == 0
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in captured.out
    assert test_case.expected_config_fragment in (project_dir / "sqlbuild_project.toml").read_text(
        encoding="utf-8"
    )
    assert all((project_dir / directory).is_dir() for directory in test_case.expected_directories)
    assert all((project_dir / gitkeep).is_file() for gitkeep in test_case.expected_gitkeep_files)


@pytest.mark.parametrize(
    "test_case",
    [
        InitScaffoldTestCase(
            description="styles init heading",
            project_dir_name="color-project",
            expected_directories=(),
            expected_gitkeep_files=(),
            expected_config_fragment='name = "color_project"',
            expected_color_fragments=(
                "\033[34m\033[1mSQLBuild project created\033[0m",
                "  \033[34mProject\033[0m: color_project",
                "\033[1mNext steps\033[0m:",
                "  7. \033[2msqb compile\033[0m",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_color_terminal_when_running_init_then_it_styles_heading(
    test_case: InitScaffoldTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / test_case.project_dir_name
    project_dir.mkdir()
    monkeypatch.setattr(native_module, "update_sqlbuild_skills", lambda *, project_dir: None)
    use_color_terminal(monkeypatch)

    result: int = run_init_command(project_dir)

    captured: CaptureResult[str] = capsys.readouterr()
    assert result == 0
    expected_color_fragment: str
    for expected_color_fragment in test_case.expected_color_fragments:
        assert expected_color_fragment in captured.out
