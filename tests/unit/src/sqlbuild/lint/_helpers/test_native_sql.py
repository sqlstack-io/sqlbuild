"""Unit tests for the native SQL lint process boundary."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import ExpansionSpan, LintBody, LintConfig, LintViolation
from tests.unit.src.sqlbuild.lint._helpers._test_types import (
    ManyViolationLocationTestCase,
    NativeSqlFindingTestCase,
    NativeSqlFixTestCase,
    ReservedCteLintTestCase,
)
from tests.unit.src.sqlbuild.lint._helpers.helpers import (
    authored_positions,
    run_native_sql_lint,
    unused_cte_orders_file,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ReservedCteLintTestCase(
            description="SQLBuild harness CTE is excluded from generic unused-CTE lint",
            sql="WITH __expected__items AS (SELECT 1) SELECT 1",
            expected_violation_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_reserved_harness_cte_when_linting_then_framework_input_is_not_reported(
    test_case: ReservedCteLintTestCase,
    tmp_path: Path,
) -> None:
    target: Path = tmp_path / "test.sql"
    body: LintBody = LintBody(
        file_path=target,
        body_start=0,
        body_end=len(test_case.sql),
        lint_text=test_case.sql,
        passes=(),
    )

    result: dict[Path, tuple[LintViolation, ...]] = run_native_sql_lint(
        bodies=(body,),
        contents_by_path={target: test_case.sql},
        config=LintConfig(dialect="duckdb"),
    )

    assert sum(len(entries) for entries in result.values()) == test_case.expected_violation_count


@pytest.mark.parametrize(
    "test_case",
    [
        NativeSqlFixTestCase(
            description="multi-branch boolean case has no unsafe partial fix",
            sql=(
                "SELECT CASE "
                "WHEN status = 'open' THEN TRUE "
                "WHEN priority = 'high' THEN TRUE "
                "ELSE FALSE END AS actionable "
                "FROM support_tickets"
            ),
            expected_code="SQLCLEAN030",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_multi_branch_boolean_case_when_linting_then_partial_fix_is_withheld(
    test_case: NativeSqlFixTestCase,
    tmp_path: Path,
) -> None:
    target: Path = tmp_path / "support_tickets.sql"
    body: LintBody = LintBody(
        file_path=target,
        body_start=0,
        body_end=len(test_case.sql),
        lint_text=test_case.sql,
        passes=(),
    )

    result: dict[Path, tuple[LintViolation, ...]] = run_native_sql_lint(
        bodies=(body,),
        contents_by_path={target: test_case.sql},
        config=LintConfig(
            dialect="duckdb",
            enabled_native_rules=(test_case.expected_code,),
        ),
    )

    assert len(result[target]) == 1
    violation: LintViolation = result[target][0]
    assert violation.code == test_case.expected_code
    assert violation.fix is None


@pytest.mark.parametrize(
    "test_case",
    [
        NativeSqlFixTestCase(
            description="equal-length expansion wholly inside an unused CTE",
            sql="WITH unused AS (SELECT 42 AS order_id) SELECT 1 AS order_id",
            expected_code="SQLCLEAN005",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_interior_with_authored_boundaries_when_linting_then_fix_is_refused(
    test_case: NativeSqlFixTestCase, tmp_path: Path
) -> None:
    target: Path = tmp_path / "orders.sql"
    start: int = test_case.sql.index("42")
    body: LintBody = LintBody(
        file_path=target,
        body_start=0,
        body_end=len(test_case.sql),
        lint_text=test_case.sql,
        passes=(
            (
                ExpansionSpan(
                    source_start=start,
                    source_end=start + 2,
                    output_start=start,
                    output_end=start + 2,
                ),
            ),
        ),
    )
    result: dict[Path, tuple[LintViolation, ...]] = run_native_sql_lint(
        bodies=(body,),
        contents_by_path={target: test_case.sql.replace("42", "__")},
        config=LintConfig(dialect="duckdb", enabled_native_rules=(test_case.expected_code,)),
    )
    violation: LintViolation = result[target][0]
    assert violation.code == test_case.expected_code
    assert violation.fix is None
    assert (
        violation.fix_unavailable_reason
        == "fix overlaps generated SQL or a non-contiguous authored region"
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NativeSqlFindingTestCase(
            description="cross joined relation used by filter is not reported as unused",
            sql=(
                "SELECT orders.order_id "
                "FROM orders "
                "CROSS JOIN processing_cutoff AS cutoff "
                "WHERE orders.created_at < cutoff.created_at"
            ),
            selected_rule="SQLCLEAN032",
            expected_violation_count=0,
        ),
        NativeSqlFindingTestCase(
            description="lateral relation used by parenthesized later join is not reported",
            sql=(
                "SELECT orders.order_id, products.name "
                "FROM orders "
                "CROSS JOIN LATERAL UNNEST(orders.product_ids) AS item "
                "INNER JOIN products ON (products.product_id = item.product_id)"
            ),
            selected_rule="SQLCLEAN032",
            expected_violation_count=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cross_join_used_downstream_when_linting_then_no_unused_join_is_reported(
    test_case: NativeSqlFindingTestCase,
    tmp_path: Path,
) -> None:
    target: Path = tmp_path / "orders.sql"
    body: LintBody = LintBody(
        file_path=target,
        body_start=0,
        body_end=len(test_case.sql),
        lint_text=test_case.sql,
        passes=(),
    )

    result: dict[Path, tuple[LintViolation, ...]] = run_native_sql_lint(
        bodies=(body,),
        contents_by_path={target: test_case.sql},
        config=LintConfig(
            dialect="duckdb",
            enabled_native_rules=(test_case.selected_rule,),
        ),
    )

    assert sum(len(violations) for violations in result.values()) == (
        test_case.expected_violation_count
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ManyViolationLocationTestCase(
            description="many unused CTEs across several multi-line files",
            prefix="-- orders header\n-- second header line\n\n",
            unused_cte_count=40,
            file_count=3,
            expected_code="SQLCLEAN005",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_many_violations_per_file_when_linting_then_locations_match_authored_lines(
    test_case: ManyViolationLocationTestCase,
    tmp_path: Path,
) -> None:
    files: tuple[tuple[Path, str, LintBody], ...] = tuple(
        unused_cte_orders_file(
            tmp_path=tmp_path,
            prefix=test_case.prefix,
            file_index=file_index,
            unused_cte_count=test_case.unused_cte_count,
        )
        for file_index in range(test_case.file_count)
    )
    contents_by_path: dict[Path, str] = {target: contents for target, contents, _ in files}
    bodies: tuple[LintBody, ...] = tuple(body for _, _, body in files)
    config: LintConfig = LintConfig(
        dialect="duckdb", enabled_native_rules=(test_case.expected_code,)
    )

    result: dict[Path, tuple[LintViolation, ...]] = run_native_sql_lint(
        bodies=bodies, contents_by_path=contents_by_path, config=config
    )

    for target, contents in contents_by_path.items():
        assert {violation.code for violation in result[target]} == {test_case.expected_code}
        assert sorted((violation.line, violation.column) for violation in result[target]) == (
            authored_positions(
                contents=contents,
                needles=tuple(f"unused_{index:03d}" for index in range(test_case.unused_cte_count)),
            )
        )
