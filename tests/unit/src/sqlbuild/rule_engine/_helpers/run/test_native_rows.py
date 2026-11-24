"""Externally visible outcomes of the natively built rules request."""

from pathlib import Path

import pytest

import sqlbuild._native as native
from sqlbuild._native import RulesCacheConfig, RulesConfig, RulesError, RulesResult
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.rule_engine._helpers.engine.helpers import evaluate_contract_rule
from tests.unit.src.sqlbuild.rule_engine._helpers.run._test_types import (
    NativeRowsEncodeErrorTestCase,
    NativeRowsMemoTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.run.helpers import (
    memo_reuses,
    rebuild_identity,
)
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project


@pytest.mark.parametrize(
    "test_case",
    [
        NativeRowsEncodeErrorTestCase(
            description="integer beyond 64 bits", rejected_value=2**70, expected_message="64-bit"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_config_value_the_encoder_rejects_when_building_natively_then_rules_error_is_raised(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    test_case: NativeRowsEncodeErrorTestCase,
) -> None:
    project: CompiledProjectView = build_project(
        name="orders",
        relative_path="sql/models/orders.sql",
        sql="SELECT 1",
        config_values={"priority": test_case.rejected_value},
    )

    with pytest.raises(RulesError, match=test_case.expected_message):
        native.evaluate_native_rules(
            project=project,
            config=RulesConfig(cache=RulesCacheConfig(enabled=False)),
            project_dir=tmp_path,
            catalogue=(),
        )


@pytest.mark.parametrize(
    "test_case",
    [NativeRowsMemoTestCase("memoized response follows the native build", (False, False, True))],
    ids=lambda case: case.description,
)
def test_given_memoized_response_from_another_build_when_evaluating_natively_then_rules_rerun(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    test_case: NativeRowsMemoTestCase,
) -> None:
    _ = native.compile_stage_work_info(reset=True)
    reused: list[bool] = []
    config_values: dict[str, object] = {"materialized": "table"}

    cold: RulesResult = evaluate_contract_rule(
        config_values=config_values, project_dir=tmp_path, cache_enabled=True
    )
    reused.append(memo_reuses() == 1)
    rebuild_identity(monkeypatch=monkeypatch)
    rebuilt: RulesResult = evaluate_contract_rule(
        config_values=config_values, project_dir=tmp_path, cache_enabled=True
    )
    reused.append(memo_reuses() == 1)
    warm: RulesResult = evaluate_contract_rule(
        config_values=config_values, project_dir=tmp_path, cache_enabled=True
    )
    reused.append(memo_reuses() == 1)

    assert tuple(reused) == test_case.expected_reused
    assert rebuilt.findings == warm.findings == cold.findings
