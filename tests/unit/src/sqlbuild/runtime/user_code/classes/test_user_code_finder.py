"""The user-code finder claims only project modules that normal import would load."""

import sys
from importlib.machinery import ModuleSpec
from pathlib import Path

import pytest

from sqlbuild.runtime.user_code.classes.source_only_loader import SourceOnlyLoader
from sqlbuild.runtime.user_code.classes.user_code_finder import UserCodeFinder
from tests.unit.src.sqlbuild.runtime.user_code.classes._test_types import (
    FinderDeletedDirectoryTestCase,
    FinderSpecTestCase,
    FinderWorkingDirectoryTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        FinderSpecTestCase(
            description="a project module loads from source",
            module_name="order_rules",
            earlier_entries=(),
            expected_source_only=True,
        ),
        FinderSpecTestCase(
            description="an installed module is left to normal import",
            module_name="pytest",
            earlier_entries=(),
            expected_source_only=False,
        ),
        FinderSpecTestCase(
            description="a project module shadowed by an earlier path entry is left alone",
            module_name="order_rules",
            earlier_entries=("shadow",),
            expected_source_only=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_module_name_when_finding_spec_then_only_project_modules_load_from_source(
    test_case: FinderSpecTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    project.mkdir()
    _ = (project / "order_rules.py").write_text("LIMIT: int = 1\n", encoding="utf-8")
    shadow: Path = tmp_path / "shadow"
    shadow.mkdir()
    _ = (shadow / "order_rules.py").write_text("LIMIT: int = 2\n", encoding="utf-8")
    earlier: list[str] = [str(tmp_path / name) for name in test_case.earlier_entries]
    monkeypatch.setattr(sys, "path", [*earlier, str(project), *sys.path])
    finder: UserCodeFinder = UserCodeFinder()
    finder.roots.add(project.resolve())

    spec: ModuleSpec | None = finder.find_spec(test_case.module_name)

    source_only: bool = spec is not None and isinstance(spec.loader, SourceOnlyLoader)
    assert source_only == test_case.expected_source_only


@pytest.mark.parametrize(
    "test_case",
    [
        FinderWorkingDirectoryTestCase(
            description="moving into the project makes its modules load from source",
            first_directory="elsewhere",
            second_directory="project",
            expected_source_only=(False, True),
        ),
        FinderWorkingDirectoryTestCase(
            description="moving out of the project leaves its modules to normal import",
            first_directory="project",
            second_directory="elsewhere",
            expected_source_only=(True, False),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_working_directory_entry_when_directory_changes_then_finder_follows_it(
    test_case: FinderWorkingDirectoryTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    project.mkdir()
    _ = (project / "order_rules.py").write_text("LIMIT: int = 1\n", encoding="utf-8")
    (tmp_path / "elsewhere").mkdir()
    monkeypatch.setattr(sys, "path", ["", *sys.path])
    finder: UserCodeFinder = UserCodeFinder()
    finder.roots.add(project.resolve())

    monkeypatch.chdir(tmp_path / test_case.first_directory)
    first: ModuleSpec | None = finder.find_spec("order_rules")
    monkeypatch.chdir(tmp_path / test_case.second_directory)
    second: ModuleSpec | None = finder.find_spec("order_rules")

    assert (
        first is not None and isinstance(first.loader, SourceOnlyLoader),
        second is not None and isinstance(second.loader, SourceOnlyLoader),
    ) == test_case.expected_source_only


@pytest.mark.parametrize(
    "test_case",
    [
        FinderDeletedDirectoryTestCase(
            description="a project module is left to normal import", module_name="order_rules"
        ),
        FinderDeletedDirectoryTestCase(
            description="a standard-library module is left to normal import", module_name="json"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deleted_working_directory_when_finding_spec_then_relative_entry_is_skipped(
    test_case: FinderDeletedDirectoryTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    project.mkdir()
    _ = (project / "order_rules.py").write_text("LIMIT: int = 1\n", encoding="utf-8")
    deleted: Path = tmp_path / "deleted"
    deleted.mkdir()
    monkeypatch.setattr(sys, "path", ["", *sys.path])
    finder: UserCodeFinder = UserCodeFinder()
    finder.roots.add(project.resolve())
    monkeypatch.chdir(deleted)
    deleted.rmdir()

    spec: ModuleSpec | None = finder.find_spec(test_case.module_name)

    assert spec == test_case.expected_spec


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
