from __future__ import annotations

import re
import tomllib
from pathlib import Path

import pytest

from tests.unit.src.sqlbuild.cli.project_metadata._test_types import ProjectDependencyTestCase

REPO_ROOT: Path = Path(__file__).resolve().parents[6]
DEPENDENCY_SPECIFIER_PATTERN: re.Pattern[str] = re.compile(r"[\[<>=!~]")


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectDependencyTestCase(
            description="upstream polyglot distribution is not a direct dependency",
            dependency_name="polyglot-sql",
            expected_in_core_dependencies=False,
            expected_optional_extra_absent=True,
        ),
        ProjectDependencyTestCase(
            description="the maintained polyglot fork is no longer a dependency",
            dependency_name="polyglot-sql-chio",
            expected_in_core_dependencies=False,
            expected_optional_extra_absent=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_metadata_when_reading_dependencies_then_required_packages_are_core(
    test_case: ProjectDependencyTestCase,
) -> None:
    pyproject: dict[str, object] = tomllib.loads(
        (REPO_ROOT / "pyproject.toml").read_text(encoding="utf-8")
    )
    project: dict[str, object] = pyproject["project"]  # type: ignore[assignment]
    dependencies: list[str] = project["dependencies"]  # type: ignore[assignment]
    optional_dependencies: dict[str, list[str]] = project.get("optional-dependencies", {})  # type: ignore[assignment]

    dependency_names: set[str] = {
        DEPENDENCY_SPECIFIER_PATTERN.split(dependency, maxsplit=1)[0] for dependency in dependencies
    }

    assert (
        test_case.dependency_name in dependency_names
    ) is test_case.expected_in_core_dependencies
    assert (
        test_case.dependency_name not in optional_dependencies
    ) is test_case.expected_optional_extra_absent
