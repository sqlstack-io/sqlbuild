"""The --expect option parses into exact outcomes."""

from __future__ import annotations

import argparse

import pytest

from scripts.compiler_differential._helpers.running.options import parse_expected_outcome
from scripts.compiler_differential.models import ExpectedOutcome
from tests.unit.scripts.compiler_differential._helpers.running._test_types import (
    ExpectedOutcomeErrorTestCase,
    ExpectedOutcomeTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedOutcomeTestCase(
            description="success", value="success", expected_outcome=ExpectedOutcome()
        ),
        ExpectedOutcomeTestCase(
            description="failure_code",
            value="failure:P001",
            expected_outcome=ExpectedOutcome(error_code="P001"),
        ),
        ExpectedOutcomeTestCase(
            description="rule_code",
            value="failure:SQLCLEAN005",
            expected_outcome=ExpectedOutcome(error_code="SQLCLEAN005"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_expect_value_when_parsing_then_outcome_matches(
    test_case: ExpectedOutcomeTestCase,
) -> None:
    assert parse_expected_outcome(test_case.value) == test_case.expected_outcome


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedOutcomeErrorTestCase(
            description=value.replace(":", "_") or "empty",
            value=value,
            expected_message="success or failure:<CODE>",
        )
        for value in ("", "failure", "failure:", "failure:p001", "P001", "ok")
    ],
    ids=lambda case: case.description,
)
def test_given_malformed_expect_value_when_parsing_then_argument_error_is_raised(
    test_case: ExpectedOutcomeErrorTestCase,
) -> None:
    with pytest.raises(argparse.ArgumentTypeError, match=test_case.expected_message):
        _ = parse_expected_outcome(test_case.value)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
