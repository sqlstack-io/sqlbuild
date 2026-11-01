"""Real-CLI coverage for configuration-only Rule exceptions and rejected inline suppressions."""

from __future__ import annotations

from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    FormatExceptionPolicyTestCase,
    RuleExceptionPolicyTestCase,
)

_EXCEPTION: str = (
    '[[rules.rule_exceptions]]\nrule = "SQLCLEAN004"\npath = "models/customers.sql"\n'
    'reason = "The sample intentionally keeps one row."\n'
)
_IGNORE: str = (
    '[[rules.rule_ignores]]\nrules = ["SQLCLEAN004"]\npaths = ["models/**"]\n'
    'reason = "Samples intentionally keep one row."\n'
)
_DIRECTIVE: str = "-- sqb: ignore SQLCLEAN004 because the sample intentionally keeps one row\n"
_PRINTED_ENTRY: str = (
    "[[rules.rule_exceptions]]\n"
    '            rule = "SQLCLEAN004"\n'
    '            path = "models/customers.sql"\n'
    '            reason = "the sample intentionally keeps one row"'
)
_REMOVED: str = "sets [rules] allow_exceptions, which SQLBuild no longer supports"


@pytest.mark.parametrize(
    "test_case",
    [
        RuleExceptionPolicyTestCase(
            description="inline directive is an error printing its exception",
            rules_toml="",
            directive=_DIRECTIVE,
            expected_exit_code=1,
            expected_fragments=(
                "SQBRSUPPRESSION000",
                "Inline Rule suppressions are not supported",
                _PRINTED_ENTRY,
                "SQLCLEAN004",
            ),
        ),
        RuleExceptionPolicyTestCase(
            description="allow_exceptions false is a removed setting",
            rules_toml=f"allow_exceptions = false\n\n{_EXCEPTION}",
            directive="",
            expected_exit_code=1,
            expected_fragments=(_REMOVED, "remove the allow_exceptions line from [rules]"),
        ),
        RuleExceptionPolicyTestCase(
            description="allow_exceptions true is a removed setting",
            rules_toml="allow_exceptions = true\n",
            directive="",
            expected_exit_code=1,
            expected_fragments=(_REMOVED,),
        ),
        RuleExceptionPolicyTestCase(
            description="configured exception",
            rules_toml=_EXCEPTION,
            directive="",
            expected_exit_code=0,
        ),
        RuleExceptionPolicyTestCase(
            description="scoped ignore",
            rules_toml=_IGNORE,
            directive="",
            expected_exit_code=0,
        ),
        RuleExceptionPolicyTestCase(
            description="project-wide ignore",
            rules_toml='ignore = ["SQLCLEAN004"]\n',
            directive="",
            expected_exit_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rule_exception_mechanism_when_compiling_then_only_configuration_applies(
    test_case: RuleExceptionPolicyTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n'
        f"{test_case.rules_toml}",
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "customers.sql").write_text(
        "MODEL (description 'Customer sample.');\n"
        "WITH customers AS (SELECT 1 AS customer_id)\n"
        f"{test_case.directive}SELECT customer_id FROM customers LIMIT 1\n",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])
    output: CaptureResult[str] = capsys.readouterr()

    assert exit_code == test_case.expected_exit_code, output.out + output.err
    for fragment in test_case.expected_fragments:
        assert fragment in output.out + output.err


@pytest.mark.parametrize(
    "test_case",
    [
        FormatExceptionPolicyTestCase(
            description="removed setting is rejected like compile",
            setting="allow_exceptions = false",
            expected_exit_code=1,
            expected_fragment=_REMOVED,
            expected_compile_fragment=_REMOVED,
        ),
        FormatExceptionPolicyTestCase(
            description="inline directive is rejected like compile",
            setting="",
            expected_exit_code=1,
            expected_fragment="Inline Rule suppressions are not supported",
            expected_compile_fragment="Inline Rule suppressions are not supported",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_exception_policy_when_formatting_then_it_is_validated_like_compile(
    test_case: FormatExceptionPolicyTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN004"]\n'
        f"{test_case.setting}\n",
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "customers.sql").write_text(
        "MODEL (description 'Customer sample.');\n"
        "WITH customers AS (\n  SELECT 1 AS customer_id\n)\n\n"
        f"SELECT customer_id\nFROM customers\n{_DIRECTIVE}LIMIT 1\n",
        encoding="utf-8",
    )

    format_exit_code: int = main(["--project-dir", str(tmp_path), "format", "--check"])
    format_output: CaptureResult[str] = capsys.readouterr()
    compile_exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache"])
    compile_output: CaptureResult[str] = capsys.readouterr()

    assert format_exit_code == test_case.expected_exit_code, format_output.out + format_output.err
    assert compile_exit_code == test_case.expected_exit_code
    assert test_case.expected_fragment in format_output.out + format_output.err
    assert test_case.expected_compile_fragment in compile_output.out + compile_output.err


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
