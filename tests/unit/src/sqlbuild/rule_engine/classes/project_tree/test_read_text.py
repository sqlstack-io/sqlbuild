"""Project-tree reads resolve every path exactly as a fully resolved read would."""

from pathlib import Path

import pytest

from sqlbuild._native import RuleUsageError
from sqlbuild.rule_engine.classes.project_tree import ProjectTree
from tests.unit.src.sqlbuild.rule_engine.classes.project_tree._test_types import ReadTextTestCase
from tests.unit.src.sqlbuild.rule_engine.classes.project_tree.helpers import (
    LIMITS_YAML,
    ORDERS_SQL,
    linked_project_tree,
)


@pytest.mark.parametrize(
    "test_case",
    (
        ReadTextTestCase("regular file", "sql/models/orders.sql", expected_text=ORDERS_SQL),
        ReadTextTestCase("nested file", "rules/inputs/limits.yaml", expected_text=LIMITS_YAML),
        ReadTextTestCase("dot prefix", "./models/orders.sql", expected_text=ORDERS_SQL),
        ReadTextTestCase("symlinked file", "sql/models/orders_alias.sql", expected_text=ORDERS_SQL),
        ReadTextTestCase("symlinked folder", "linked_models/orders.sql", expected_text=ORDERS_SQL),
    ),
    ids=lambda case: case.description,
)
def test_given_readable_project_file_when_reading_text_twice_then_returns_its_contents(
    tmp_path: Path, test_case: ReadTextTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)

    first: str = tree.read_text(test_case.path)
    second: str = tree.read_text(test_case.path)

    assert first == second == test_case.expected_text


@pytest.mark.parametrize(
    "test_case",
    (
        ReadTextTestCase("missing file", "sql/models/missing.sql", expected_error="does not exist"),
        ReadTextTestCase("directory", "models", expected_error="does not exist"),
        ReadTextTestCase("project root", "", expected_error="does not exist"),
        ReadTextTestCase(
            "beneath a file", "sql/models/orders.sql/x.sql", expected_error="not exist"
        ),
        ReadTextTestCase(
            "folder linked outside", "outside_models/customers.sql", expected_error="not exist"
        ),
        ReadTextTestCase(
            "file linked outside", "sql/models/customers.sql", expected_error="not exist"
        ),
        ReadTextTestCase("untracked suffix", "sql/models/notes.txt", expected_error="tracked file"),
        ReadTextTestCase("parent escape", "../outside/customers.sql", expected_error="escapes"),
    ),
    ids=lambda case: case.description,
)
def test_given_unreadable_project_path_when_reading_text_then_raises_usage_error(
    tmp_path: Path, test_case: ReadTextTestCase
) -> None:
    tree: ProjectTree = linked_project_tree(tmp_path)
    _ = tree.read_text("sql/models/orders.sql")

    with pytest.raises(RuleUsageError, match=test_case.expected_error):
        _ = tree.read_text(test_case.path)


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-vv"]))
