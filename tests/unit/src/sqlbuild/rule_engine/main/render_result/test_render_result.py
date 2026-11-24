"""Policy text and JSON rendering parity tests."""

import json
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import Finding, RulesResult
from sqlbuild._native import format_rules_result as format_result
from tests.unit.src.sqlbuild.rule_engine.main.render_result._test_types import (
    RenderResultParityTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (
        RenderResultParityTestCase(
            description="text and JSON contain identical complete fault facts",
            result=RulesResult(
                findings=(
                    Finding(
                        code="SQBRCONTRACT101",
                        path=Path("sql/models/mart/commerce__mart__orders.sql"),
                        line=3,
                        column=7,
                        message="model SQL must keep transformation logic in top-level CTEs",
                        remediation=(
                            "Move transformation logic into named top-level CTEs before the terminal SELECT."
                        ),
                    ),
                ),
                evaluated_models=1,
                cache_hits=0,
                cache_misses=1,
            ),
            expected_finding_count=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_policy_faults_when_rendering_json_and_text_then_both_contain_identical_facts(
    test_case: RenderResultParityTestCase,
) -> None:
    result: RulesResult = test_case.result

    text_output: str = format_result(result=result, json_output=False)
    json_payload: dict[str, Any] = json.loads(format_result(result=result, json_output=True))

    assert json_payload["finding_count"] == test_case.expected_finding_count
    for finding in json_payload["findings"]:
        assert (
            f"{finding['path']}:{finding['line']}:{finding['column']} "
            f"[{finding['code']}] {finding['message']}"
        ) in text_output
        assert f"  Remediation: {finding['remediation']}" in text_output
