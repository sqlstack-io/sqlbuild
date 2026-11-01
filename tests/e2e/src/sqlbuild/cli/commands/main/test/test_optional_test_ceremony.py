"""E2E coverage for SQL tests without an explicit mode or trailing SELECT 1."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    OptionalTestCeremonyE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import (
    prepare_optional_ceremony_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_MODEL_TEST_CTES: str = (
    "WITH\n"
    "__source__raw_orders AS (\n"
    "  SELECT 1 AS order_id, ' PAID ' AS status\n"
    "),\n"
    "__expected__orders AS (\n"
    "  SELECT 1 AS order_id, 'paid' AS status\n"
    ")\n"
)
_MACRO_TEST_CTES: str = (
    "WITH\n"
    "input_values AS (\n"
    "  SELECT '  PAID  ' AS raw_status\n"
    "),\n"
    "__macro_actual__ AS (\n"
    '  SELECT @py.normalize_status("raw_status") AS status\n'
    "  FROM input_values\n"
    "),\n"
    "__macro_expected__ AS (\n"
    "  SELECT 'paid' AS status\n"
    ")\n"
)
_SHORT_MODEL_TEST: str = "TEST();\n\n" + _MODEL_TEST_CTES
_SHORT_MACRO_TEST: str = "TEST();\n\n" + _MACRO_TEST_CTES
_EXPLICIT_MACRO_TEST: str = (
    'TEST (mode macro, name "normalizes_status_explicitly");\n\n' + _MACRO_TEST_CTES + "SELECT 1\n"
)
_EXPLICIT_MODEL_TEST: str = "TEST();\n\n" + _MODEL_TEST_CTES + "SELECT 1\n"
_NAMED_BLOCKS_TEST: str = (
    'TEST (name "orders_keep_paid");\n\n'
    + _MODEL_TEST_CTES
    + '\nTEST (name "orders_keep_void");\n\n'
    + _MODEL_TEST_CTES.replace("' PAID '", "'void'").replace("'paid'", "'void'")
)


@pytest.mark.parametrize(
    "test_case",
    [
        OptionalTestCeremonyE2ETestCase(
            description="short and explicit test forms run, lint, and stay formatted",
            test_files={
                "tests/unit/test_orders.sql": _SHORT_MODEL_TEST,
                "tests/unit/test_normalize_status.sql": _SHORT_MACRO_TEST,
                "tests/unit/test_orders_explicit.sql": _EXPLICIT_MODEL_TEST,
                "tests/unit/test_normalize_status_explicit.sql": _EXPLICIT_MACRO_TEST,
                "tests/unit/test_orders_named_blocks.sql": _NAMED_BLOCKS_TEST,
            },
            expected_exit_code=0,
            expected_output_fragments=("PASS=6", "FAIL=0"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_optional_test_ceremony_when_testing_then_short_and_explicit_forms_agree(
    test_case: OptionalTestCeremonyE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_optional_ceremony_project(
        tmp_path=tmp_path, test_files=test_case.test_files
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )
    linted: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "rules", "run", "SQLCLEAN"), project_dir=project_dir
    )
    formatted: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "format"), project_dir=project_dir
    )
    checked: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "format", "--check"), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    assert "tests/unit/" not in linted.stdout + linted.stderr, linted.stdout + linted.stderr
    assert formatted.returncode == 0, formatted.stdout + formatted.stderr
    assert checked.returncode == 0, checked.stdout + checked.stderr
    written: dict[str, str] = {
        path: (project_dir / path).read_text(encoding="utf-8") for path in test_case.test_files
    }
    assert {path: contents.count("TEST") for path, contents in written.items()} == {
        path: contents.count("TEST") for path, contents in test_case.test_files.items()
    }
    assert {path: contents.count("SELECT 1\n") for path, contents in written.items()} == {
        path: contents.count("SELECT 1\n") for path, contents in test_case.test_files.items()
    }


@pytest.mark.parametrize(
    "test_case",
    [
        OptionalTestCeremonyE2ETestCase(
            description="an explicit mode that contradicts the ctes is rejected",
            test_files={
                "tests/unit/test_normalize_status.sql": (
                    "TEST (mode model);\n\n" + _MACRO_TEST_CTES
                ),
            },
            expected_exit_code=1,
            expected_output_fragments=(
                "P001",
                "is mode 'model' but defines macro-test CTE '__macro_actual__'",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_contradicting_explicit_mode_when_testing_then_compile_rejects_it(
    test_case: OptionalTestCeremonyE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_optional_ceremony_project(
        tmp_path=tmp_path, test_files=test_case.test_files
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        OptionalTestCeremonyE2ETestCase(
            description="a test file without a TEST header is rejected",
            test_files={"tests/unit/test_orders.sql": _MODEL_TEST_CTES},
            expected_exit_code=1,
            expected_output_fragments=(
                "D003",
                "must start with a TEST() header as the first non-whitespace content",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_test_file_without_header_when_testing_then_discovery_rejects_it(
    test_case: OptionalTestCeremonyE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_optional_ceremony_project(
        tmp_path=tmp_path, test_files=test_case.test_files
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
