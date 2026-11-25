"""Which DuckDB failures of converted SQL mean the test belongs on the warehouse."""

import pytest

from sqlbuild.errors.contracts.exceptions import LocalRunRefusedError
from sqlbuild.executor.local._helpers.runtime import local_failure_refusal, local_run_refusal
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import (
    FailureTextRefusalTestCase,
    LocalRunRefusalTestCase,
    OwnFailureTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        LocalRunRefusalTestCase(
            description="a guard's raise names the guard",
            error_type="ConversionException",
            message=(
                "Conversion Error: Could not convert string 'array_agg_order#0123456789ab: "
                "ARRAY_AGG of several values in no defined order' to INT32"
            ),
            fatal=False,
            expected_title="SQLQuack guard: array_agg_order",
        ),
        LocalRunRefusalTestCase(
            description="a DuckDB internal error names its first line",
            error_type="InternalException",
            message="INTERNAL Error: an expression should not be used here yet!\n\nStack Trace:",
            fatal=True,
            expected_title="DuckDB internal error: an expression should not be used here yet!",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_guard_or_engine_bug_when_classifying_then_it_refuses_naming_it(
    test_case: LocalRunRefusalTestCase,
) -> None:
    error: Exception = type(test_case.error_type, (Exception,), {})(test_case.message)

    refused: LocalRunRefusedError | None = local_run_refusal(error=error, fatal=test_case.fatal)

    assert refused is not None
    assert f"{refused.category}: {refused.construct}" == test_case.expected_title


@pytest.mark.parametrize(
    "test_case",
    [
        OwnFailureTestCase(
            description="an emulated Snowflake error is the test's own result",
            error_type="ConversionException",
            message=(
                "Conversion Error: Could not convert string 'sqlquack:100038#0123456789ab: "
                "Numeric value is not recognized' to INT32"
            ),
            fatal=False,
            expected_refused=False,
        ),
        OwnFailureTestCase(
            description="any other DuckDB error is the test's own result",
            error_type="BinderException",
            message="Binder Error: Referenced column a not found",
            fatal=False,
            expected_refused=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_error_snowflake_would_raise_too_when_classifying_then_it_is_the_tests_own(
    test_case: OwnFailureTestCase,
) -> None:
    error: Exception = type(test_case.error_type, (Exception,), {})(test_case.message)

    refused: LocalRunRefusedError | None = local_run_refusal(error=error, fatal=test_case.fatal)

    assert (refused is not None) is test_case.expected_refused


@pytest.mark.parametrize(
    "test_case",
    [
        FailureTextRefusalTestCase(
            description="a model step whose guard raised",
            message=(
                "local model 'order_id_lists' failed: Conversion Error: Could not convert string"
                " 'array_agg_order#0123456789ab: ARRAY_AGG of several values in no defined order'"
                " to INT32"
            ),
            expected_title="SQLQuack guard: array_agg_order",
        ),
        FailureTextRefusalTestCase(
            description="a model step DuckDB failed internally",
            message="local model 'orders' failed: INTERNAL Error: unexpected plan\nStack Trace:",
            expected_title="DuckDB internal error: unexpected plan",
        ),
        FailureTextRefusalTestCase(
            description="a model step that failed as Snowflake would",
            message="local model 'orders' failed: Binder Error: Referenced column a not found",
            expected_title=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_scenario_step_failure_when_classifying_its_text_then_guards_and_bugs_refuse(
    test_case: FailureTextRefusalTestCase,
) -> None:
    refused: LocalRunRefusedError | None = local_failure_refusal(test_case.message)

    title: str | None = None if refused is None else f"{refused.category}: {refused.construct}"
    assert title == test_case.expected_title


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
