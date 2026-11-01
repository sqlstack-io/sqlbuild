"""Lint bodies are prepared natively in one batch; unexpandable files fail or skip in file order."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import LintConfig, LintRunResult, ProjectCompileError, run_lint
from tests.unit.src.sqlbuild.lint._helpers._test_types import (
    BatchedPreparationTestCase,
    UnexpandableFailureTestCase,
    UnexpandableSkipTestCase,
)
from tests.unit.src.sqlbuild.lint._helpers.helpers import write_lint_project

_ORDERS_MODEL: str = (
    'MODEL (description "Orders");\n'
    'SELECT o.customer_id FROM __ref("customers") AS o WHERE o.region = NULL\n'
)
_CUSTOMERS_MODEL: str = (
    "MODEL (description \"Customers\");\nSELECT 1 AS customer_id, 'west' AS region\n"
)
_ORDERS_TEST: str = (
    "TEST ();\nWITH\n__ref__customers AS (SELECT 1 AS customer_id, 'west' AS region),\n"
    "__expected__orders AS (SELECT 1 AS customer_id)\nSELECT 1\n"
)
_NOT_NULL_AUDIT: str = "AUDIT ();\nSELECT 1 FROM @relation WHERE @column IS NULL\n"
_BROKEN_MODEL: str = 'MODEL (description "Broken");\nSELECT @py.unknown()\n'
_UNEXPANDABLE_FILES: dict[str, str] = {
    "models/customers.sql": _CUSTOMERS_MODEL,
    "models/orders.sql": _ORDERS_MODEL,
    "models/aa_broken.sql": _BROKEN_MODEL,
    "models/zz_broken.sql": _BROKEN_MODEL,
}


@pytest.mark.parametrize(
    "test_case",
    [
        BatchedPreparationTestCase(
            description="models, tests, and generic audits",
            files={
                "models/customers.sql": _CUSTOMERS_MODEL,
                "models/orders.sql": _ORDERS_MODEL,
                "tests/unit/test_orders.sql": _ORDERS_TEST,
                "audits/generic/not_null.sql": _NOT_NULL_AUDIT,
            },
            expected_codes=("SQLCLEAN001",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_when_linting_then_bodies_of_every_kind_are_prepared_in_one_batch(
    tmp_path: Path, test_case: BatchedPreparationTestCase
) -> None:
    write_lint_project(root=tmp_path, files=test_case.files)
    config: LintConfig = LintConfig(dialect="duckdb")

    first: LintRunResult = run_lint(project_dir=tmp_path, config=config)
    second: LintRunResult = run_lint(project_dir=tmp_path, config=config)

    assert first == second
    assert tuple(violation.code for violation in first.violations) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    [
        UnexpandableFailureTestCase(
            description="first unexpandable file in path order is raised",
            expected_error_file="models/aa_broken.sql",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unexpandable_files_when_linting_then_first_in_file_order_is_raised(
    tmp_path: Path, test_case: UnexpandableFailureTestCase
) -> None:
    write_lint_project(root=tmp_path, files=_UNEXPANDABLE_FILES)

    with pytest.raises(ProjectCompileError) as raised:
        _ = run_lint(project_dir=tmp_path, config=LintConfig(dialect="duckdb"))

    assert str(raised.value).startswith(str(tmp_path / test_case.expected_error_file))


@pytest.mark.parametrize(
    "test_case",
    [
        UnexpandableSkipTestCase(
            description="skipped unexpandable files are reported and the rest is linted",
            expected_unexpandable=("models/aa_broken.sql", "models/zz_broken.sql"),
            expected_codes=("SQLCLEAN001",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unexpandable_files_when_linting_with_skipping_then_they_are_reported(
    tmp_path: Path, test_case: UnexpandableSkipTestCase
) -> None:
    write_lint_project(root=tmp_path, files=_UNEXPANDABLE_FILES)

    result: LintRunResult = run_lint(
        project_dir=tmp_path, config=LintConfig(dialect="duckdb"), skip_unexpandable=True
    )

    assert (
        tuple(path.relative_to(tmp_path).as_posix() for path in result.unexpandable)
        == test_case.expected_unexpandable
    )
    assert tuple(violation.code for violation in result.violations) == test_case.expected_codes


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
