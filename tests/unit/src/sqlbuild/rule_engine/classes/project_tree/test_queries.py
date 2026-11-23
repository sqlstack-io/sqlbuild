"""Project-tree queries select discovered paths exactly as pathlib and fnmatch would."""

from pathlib import Path

import pytest

from sqlbuild._native import RuleUsageError
from sqlbuild.rule_engine.classes.project_tree import ProjectTree
from tests.unit.src.sqlbuild.rule_engine.classes.project_tree._test_types import (
    RelativePartsTestCase,
    TreeQueryTestCase,
)
from tests.unit.src.sqlbuild.rule_engine.classes.project_tree.helpers import linked_project_tree


@pytest.mark.parametrize(
    "test_case",
    (
        TreeQueryTestCase("project root", "", ("models", "rules")),
        TreeQueryTestCase("current directory", ".", ("models", "rules")),
        TreeQueryTestCase(
            "folder with trailing slash",
            "sql/models/",
            ("sql/models/notes.txt", "sql/models/orders.sql"),
        ),
        TreeQueryTestCase("nested folder", "./rules//inputs", ("rules/inputs/limits.yaml",)),
        TreeQueryTestCase("missing folder", "missing", ()),
    ),
    ids=lambda case: case.description,
)
def test_given_directory_when_listing_children_then_returns_direct_entries(
    tmp_path: Path, test_case: TreeQueryTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)

    values: tuple[str, ...] = tuple(node.value for node in tree.children(test_case.argument))

    assert values == test_case.expected_values


@pytest.mark.parametrize(
    "test_case",
    (
        TreeQueryTestCase(
            "everything",
            "",
            (
                "models",
                "sql/models/notes.txt",
                "sql/models/orders.sql",
                "rules",
                "rules/inputs",
                "rules/inputs/limits.yaml",
            ),
        ),
        TreeQueryTestCase("folder", "rules/", ("rules/inputs", "rules/inputs/limits.yaml")),
        TreeQueryTestCase("current directory spelled out", ".", ()),
    ),
    ids=lambda case: case.description,
)
def test_given_directory_when_listing_descendants_then_returns_entries_below_it(
    tmp_path: Path, test_case: TreeQueryTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)

    values: tuple[str, ...] = tuple(node.value for node in tree.descendants(test_case.argument))

    assert values == test_case.expected_values


@pytest.mark.parametrize(
    "test_case",
    (
        TreeQueryTestCase("star crosses folders", "*.sql", ("sql/models/orders.sql",)),
        TreeQueryTestCase("single character", "rule?", ("rules",)),
        TreeQueryTestCase("negated set", "[!m]*s", ("rules", "rules/inputs")),
        TreeQueryTestCase("no match", "*.py", ()),
    ),
    ids=lambda case: case.description,
)
def test_given_pattern_when_globbing_then_returns_fnmatch_matches(
    tmp_path: Path, test_case: TreeQueryTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)

    values: tuple[str, ...] = tuple(node.value for node in tree.glob(test_case.argument))

    assert values == test_case.expected_values


@pytest.mark.parametrize(
    "test_case",
    (
        RelativePartsTestCase(
            "relative path", "sql/models/orders.sql", "", ("models", "orders.sql")
        ),
        RelativePartsTestCase(
            "beneath a folder", "sql/models/orders.sql", "sql/models/", ("orders.sql",)
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_path_when_reading_relative_parts_then_returns_project_relative_parts(
    tmp_path: Path, test_case: RelativePartsTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)

    parts: tuple[str, ...] = tree.relative_parts(path=test_case.path, under=test_case.under)

    assert parts == test_case.expected_parts


@pytest.mark.parametrize(
    "test_case",
    (
        RelativePartsTestCase(
            "through a linked folder",
            "orders/linked_models/orders.sql",
            "",
            ("models", "orders.sql"),
        ),
        RelativePartsTestCase(
            "beneath a folder", "orders/models/orders.sql", "models", ("orders.sql",)
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_absolute_path_inside_project_when_reading_relative_parts_then_strips_the_root(
    tmp_path: Path, test_case: RelativePartsTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)

    parts: tuple[str, ...] = tree.relative_parts(
        path=tmp_path / test_case.path, under=test_case.under
    )

    assert parts == test_case.expected_parts


@pytest.mark.parametrize(
    "test_case",
    (
        RelativePartsTestCase(
            "not beneath", "sql/models/orders.sql", "rules", None, "is not beneath"
        ),
        RelativePartsTestCase("absolute outside", "/", "", None, "escapes the project"),
    ),
    ids=lambda case: case.description,
)
def test_given_path_outside_scope_when_reading_relative_parts_then_raises_usage_error(
    tmp_path: Path, test_case: RelativePartsTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)

    with pytest.raises(RuleUsageError, match=test_case.expected_error):
        tree.relative_parts(path=test_case.path, under=test_case.under)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
