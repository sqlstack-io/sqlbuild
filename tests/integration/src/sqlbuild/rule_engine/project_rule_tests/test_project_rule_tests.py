"""A project's own Rule tests load shared helpers under `rules/`, whatever pytest's import mode."""

import subprocess
import sys
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.rule_engine.project_rule_tests._test_types import (
    ProjectRuleTestsTestCase,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "rules/_helpers/naming.py": (
        'LAYER_PREFIXES: tuple[str, ...] = ("stg_", "int_", "fct_")\n\n\n'
        "def has_layer_prefix(name: str) -> bool:\n"
        "    return name.startswith(LAYER_PREFIXES)\n"
    ),
    "rules/model_names.py": (
        "from rules._helpers.naming import has_layer_prefix\n"
        "from sqlbuild.rules import Finding, Model, RuleContext, rule\n\n\n"
        "@rule(\n"
        '    code="XSQBRNAME001",\n'
        '    message="Model names start with their layer",\n'
        '    remediation="Rename the model with a stg_, int_ or fct_ prefix.",\n'
        ")\n"
        "def model_names(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        "    if has_layer_prefix(model.name):\n"
        "        return []\n"
        "    return [ctx.finding(subject=model, line=1)]\n"
    ),
}
_RULE_TEST: str = (
    "from rules.model_names import model_names\n"
    "from sqlbuild.rules.testing import RuleCase, evaluate_rule\n\n\n"
    "def test_given_unprefixed_model_when_evaluating_then_reports_finding() -> None:\n"
    "    result = evaluate_rule(\n"
    "        rule=model_names,\n"
    "        test_case=RuleCase(\n"
    '            description="unprefixed orders model",\n'
    "            source='MODEL (description \"Orders\");\\nSELECT 1 AS order_id\\n',\n"
    '            path="sql/models/orders.sql",\n'
    "            expected_finding_count=1,\n"
    "        ),\n"
    "    )\n"
    "    assert result.finding_count == 1\n\n\n"
    "def test_given_prefixed_model_when_evaluating_then_reports_nothing() -> None:\n"
    "    result = evaluate_rule(\n"
    "        rule=model_names,\n"
    "        test_case=RuleCase(\n"
    '            description="staged orders model",\n'
    "            source='MODEL (description \"Orders\");\\nSELECT 1 AS order_id\\n',\n"
    '            path="sql/models/stg_orders.sql",\n'
    "            expected_finding_count=0,\n"
    "        ),\n"
    "    )\n"
    "    assert result.finding_count == 0\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        ProjectRuleTestsTestCase(
            description="prepend import mode",
            import_mode="prepend",
            rule_test_path="tests/rules/test_model_names.py",
        ),
        ProjectRuleTestsTestCase(
            description="importlib import mode",
            import_mode="importlib",
            rule_test_path="tests/rules/test_model_names.py",
        ),
        ProjectRuleTestsTestCase(
            description="importlib import mode inside the rules package",
            import_mode="importlib",
            rule_test_path="rules/tests/test_model_names.py",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_rule_importing_a_shared_helper_when_running_its_tests_then_they_pass(
    test_case: ProjectRuleTestsTestCase, tmp_path: Path
) -> None:
    for relative_path, contents in {**_PROJECT_FILES, test_case.rule_test_path: _RULE_TEST}.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")

    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("pytest")),
            "-q",
            "-p",
            "no:cacheprovider",
            f"--import-mode={test_case.import_mode}",
            "-o",
            "pythonpath=.",
            test_case.rule_test_path,
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert test_case.expected_summary in result.stdout


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
