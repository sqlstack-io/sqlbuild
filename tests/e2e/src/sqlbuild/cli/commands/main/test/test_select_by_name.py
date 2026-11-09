"""Unit tests are selected by their globally unique names through the real DuckDB CLI."""

from pathlib import Path
from subprocess import CompletedProcess

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    SelectTestByNameE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    build_parameterized_test_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_ORDER_CASES: tuple[str, ...] = ("order_status [open_case]", "order_status [closed_case]")
_CUSTOMER_CASE: str = "customer_status [customer_only]"
_DUPLICATE_TEST_SQL: str = (
    "TEST ();\n\nWITH\n__source__raw_orders AS (SELECT 'open' AS status),\n"
    "__expected__orders AS (SELECT 'open' AS status)\nSELECT 1\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        SelectTestByNameE2ETestCase(
            description="a bare test name runs exactly that test",
            command=("test", "--select", "order_status"),
            expected_exit_code=0,
            expected_output_fragments=("PASS=2  FAIL=0", *_ORDER_CASES),
            unexpected_output_fragments=(_CUSTOMER_CASE,),
        ),
        SelectTestByNameE2ETestCase(
            description="the test: form runs exactly that test",
            command=("test", "--select", "test:customer_status"),
            expected_exit_code=0,
            expected_output_fragments=("PASS=1  FAIL=0", _CUSTOMER_CASE),
            unexpected_output_fragments=_ORDER_CASES,
        ),
        SelectTestByNameE2ETestCase(
            description="a test selector unions with a model selector",
            command=("test", "--select", "test:customer_status", "orders"),
            expected_exit_code=0,
            expected_output_fragments=("PASS=3  FAIL=0", _CUSTOMER_CASE, *_ORDER_CASES),
        ),
        SelectTestByNameE2ETestCase(
            description="case narrows a test selected by name",
            command=("test", "--select", "test:order_status", "--case", "open_case"),
            expected_exit_code=0,
            expected_output_fragments=("PASS=1  FAIL=0", "order_status [open_case]"),
            unexpected_output_fragments=("order_status [closed_case]", _CUSTOMER_CASE),
        ),
        SelectTestByNameE2ETestCase(
            description="an unknown name suggests the nearest test",
            command=("test", "--select", "ordr_status"),
            expected_exit_code=1,
            expected_output_fragments=(
                "error[S007]: unknown selector name 'ordr_status'",
                "did you mean 'order_status'?",
            ),
        ),
        SelectTestByNameE2ETestCase(
            description="plan rejects a unit-test name",
            command=("plan", "--select", "order_status"),
            expected_exit_code=1,
            expected_output_fragments=(
                "error[S013]: selector 'order_status' selects a unit test; only `sqb test` "
                "and `sqb build` accept unit-test selectors",
            ),
        ),
        SelectTestByNameE2ETestCase(
            description="build runs the named test and builds no models",
            command=("build", "--select", "test:order_status"),
            expected_exit_code=0,
            expected_output_fragments=("PASS=2", "TOTAL=2", *_ORDER_CASES),
            unexpected_output_fragments=(_CUSTOMER_CASE, "read by orders"),
        ),
        SelectTestByNameE2ETestCase(
            description="an empty select value plans nothing",
            command=("plan", "--select", ""),
            expected_exit_code=0,
            expected_output_fragments=("Plan ready  0 selected",),
            unexpected_output_fragments=("read by orders",),
        ),
        SelectTestByNameE2ETestCase(
            description="an empty select value builds nothing",
            command=("build", "--select", ""),
            expected_exit_code=0,
            expected_output_fragments=("Plan ready  0 selected",),
            unexpected_output_fragments=("read by orders", *_ORDER_CASES),
        ),
        SelectTestByNameE2ETestCase(
            description="an empty select value tests nothing",
            command=("test", "--select", ""),
            expected_exit_code=0,
            expected_output_fragments=("PASS=0  FAIL=0  TOTAL=0",),
            unexpected_output_fragments=(_CUSTOMER_CASE, *_ORDER_CASES),
        ),
        SelectTestByNameE2ETestCase(
            description="a blank select value plans nothing",
            command=("plan", "--select", " "),
            expected_exit_code=0,
            expected_output_fragments=("Plan ready  0 selected",),
            unexpected_output_fragments=("read by orders",),
        ),
        SelectTestByNameE2ETestCase(
            description="a blank select value builds nothing",
            command=("build", "--select", " "),
            expected_exit_code=0,
            expected_output_fragments=("Plan ready  0 selected",),
            unexpected_output_fragments=("read by orders", *_ORDER_CASES),
        ),
        SelectTestByNameE2ETestCase(
            description="a blank select value tests nothing",
            command=("test", "--select", " "),
            expected_exit_code=0,
            expected_output_fragments=("PASS=0  FAIL=0  TOTAL=0",),
            unexpected_output_fragments=(_CUSTOMER_CASE, *_ORDER_CASES),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_test_names_when_selecting_then_exactly_those_tests_run(
    tmp_path: Path, test_case: SelectTestByNameE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="select_by_name",
        repo_files=build_parameterized_test_project_files(),
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_output_fragments:
        assert fragment not in output, output


@pytest.mark.parametrize(
    "test_case",
    (
        SelectTestByNameE2ETestCase(
            description="a unit test named like a model is a compile error",
            command=("test",),
            expected_exit_code=1,
            expected_output_fragments=(
                "Project resource name 'orders' is declared as both model in models/orders.sql "
                "and unit test in tests/unit/orders.sql",
                "and calendar schedule names must be globally unique",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_test_sharing_a_model_name_when_testing_then_compile_fails(
    tmp_path: Path, test_case: SelectTestByNameE2ETestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="duplicate_name",
        repo_files={
            **build_parameterized_test_project_files(),
            "tests/unit/orders.sql": _DUPLICATE_TEST_SQL,
        },
    )

    result: CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=project
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
