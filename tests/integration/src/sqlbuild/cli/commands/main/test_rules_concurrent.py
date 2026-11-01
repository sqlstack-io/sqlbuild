"""Combined rule-policy coverage through the real compiler and custom-rule host."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    RulePassIntegrationTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (RulePassIntegrationTestCase("SQL and custom exceptions finalize together", 0, ()),),
    ids=lambda case: case.description,
)
def test_given_sql_and_custom_exceptions_when_compiling_cold_and_warm_then_both_apply(
    test_case: RulePassIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n'
        '[rules]\nselect = ["SQLCLEAN004", "XSQBRARCH001"]\n'
        "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n"
        '[[rules.rule_exceptions]]\nrule = "SQLCLEAN004"\n'
        'path = "models/orders.sql"\nreason = "A single synthetic row is intentional."\n'
        '[[rules.rule_exceptions]]\nrule = "XSQBRARCH001"\n'
        'path = "models/orders.sql"\nreason = "The synthetic model name is intentional."\n'
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id LIMIT 1\n"
    )
    rules: Path = tmp_path / "rules"
    rules.mkdir()
    (rules / "architecture.py").write_text(
        "from sqlbuild.rules import Finding, Model, RuleContext, rule\n\n"
        '@rule(code="XSQBRARCH001", message="Review model name", remediation="Rename the model.")\n'
        "def review_name(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        "    return [ctx.finding(subject=model)]\n"
    )

    cold_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    cold: dict[str, object] = json.loads(capsys.readouterr().out)
    warm_exit: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
    warm: dict[str, object] = json.loads(capsys.readouterr().out)

    assert cold_exit == warm_exit == test_case.expected_exit_code
    cold_diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], cold["diagnostics"])
    warm_diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], warm["diagnostics"])
    assert tuple(item["code"] for item in cold_diagnostics) == test_case.expected_diagnostics
    assert warm_diagnostics == cold_diagnostics
    cold_timings: dict[str, int] = cast(dict[str, int], cold["compile_timings"])
    warm_timings: dict[str, int] = cast(dict[str, int], warm["compile_timings"])
    assert cold_timings["rule_cache_misses"] > 0
    assert warm_timings["rule_cache_hits"] > 0


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
