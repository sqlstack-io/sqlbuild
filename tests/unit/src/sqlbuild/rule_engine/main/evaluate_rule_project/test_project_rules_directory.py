"""The harness copies a project rule's `rules/` directory and writes the rule under test last."""

from pathlib import Path
from typing import cast

import pytest

from sqlbuild.rule_engine.types import RuleCheck
from sqlbuild.rules.testing import RuleCase, RuleFile, RuleResult, evaluate_rule
from tests.unit.src.sqlbuild.rule_engine.main.evaluate_rule_project._test_types import (
    ProjectRulesDirectoryTestCase,
)
from tests.unit.src.sqlbuild.rule_engine.main.evaluate_rule_project.helpers import (
    PROJECT_RULE_FILES,
    load_project_rule,
)


@pytest.mark.parametrize(
    "test_case",
    (
        ProjectRulesDirectoryTestCase(
            description="a case file at the rule's path does not replace the rule",
            extra_project_files={},
            case_files=(("rules/model_names.py", "raise RuntimeError('replaced')\n"),),
            expected_finding_count=1,
        ),
        ProjectRulesDirectoryTestCase(
            description="tool caches and dot-directories under rules/ are not copied",
            extra_project_files={
                "rules/.scratch/broken.py": "@rule(\n",
                "rules/.pytest_cache/broken.py": "@rule(\n",
                "rules/__pycache__/broken.py": "@rule(\n",
            },
            case_files=(),
            expected_finding_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_project_rule_when_evaluating_then_uses_its_own_rules_directory(
    test_case: ProjectRulesDirectoryTestCase, tmp_path: Path
) -> None:
    model_names: RuleCheck = cast(
        RuleCheck,
        load_project_rule(
            root=tmp_path, files={**PROJECT_RULE_FILES, **test_case.extra_project_files}
        ),
    )

    result: RuleResult = evaluate_rule(
        rule=model_names,
        test_case=RuleCase(
            description=test_case.description,
            source='MODEL (description "Orders");\nSELECT 1 AS order_id\n',
            path="sql/models/orders.sql",
            files=tuple(
                RuleFile(path=path, source=source) for path, source in test_case.case_files
            ),
            expected_finding_count=test_case.expected_finding_count,
        ),
    )

    assert result.finding_count == test_case.expected_finding_count
