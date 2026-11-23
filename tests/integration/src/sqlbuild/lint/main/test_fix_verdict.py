"""Real-compile coverage for attributing Rule-fix compile differences to edited files."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import FixVerdict, compile_facts, fix_verdict
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.lint.main._test_types import FixVerdictTestCase
from tests.integration.src.sqlbuild.lint.main.helpers import compile_models, without_lineage

_ORDERS: str = 'MODEL (description "Orders");\nSELECT CAST(1 AS INTEGER) AS order_id\n'
_REFORMATTED_ORDERS: str = _ORDERS + "\n"
_BROKEN_PAYMENTS: str = (
    "MODEL (description \"Payments\");\nSELECT 1 AS payment_id WHERE 'open' > 1\n"
)
_PAYMENTS: str = 'MODEL (description "Payments");\nSELECT 1 AS payment_id\n'
_CUSTOMERS: str = (
    'MODEL (description "Customers");\nSELECT o.order_id AS customer_id FROM __ref("orders") AS o\n'
)


@pytest.mark.parametrize(
    "test_case",
    (
        FixVerdictTestCase(
            description="a changed model inventory cannot be attributed to one edit",
            before_models=(("orders.sql", _ORDERS),),
            after_models=(("orders.sql", _ORDERS), ("payments.sql", _PAYMENTS)),
            edited=("orders.sql",),
            expected_failing=(),
            expected_unattributed=True,
        ),
        FixVerdictTestCase(
            description="an edited model without inferred columns cannot verify",
            before_models=(
                (
                    "orders.sql",
                    'MODEL (description "Orders", sql_analysis false);\nSELECT 1 AS order_id\n',
                ),
            ),
            after_models=(
                (
                    "orders.sql",
                    'MODEL (description "Orders", sql_analysis false);\nSELECT 1 AS order_id\n',
                ),
            ),
            edited=("orders.sql",),
            expected_failing=(
                (
                    "orders.sql",
                    "Output columns could not be inferred, so the fix cannot be verified",
                ),
            ),
            expected_unattributed=False,
        ),
        FixVerdictTestCase(
            description="an edit that adds a diagnostic fails its own file",
            before_models=(("orders.sql", _ORDERS), ("payments.sql", _PAYMENTS)),
            after_models=(("orders.sql", _ORDERS), ("payments.sql", _BROKEN_PAYMENTS)),
            edited=("payments.sql",),
            expected_failing=(("payments.sql", "The fix would add compile diagnostics: B218"),),
            expected_unattributed=False,
        ),
        FixVerdictTestCase(
            description="an edit that removes a diagnostic verifies",
            before_models=(("orders.sql", _ORDERS), ("payments.sql", _BROKEN_PAYMENTS)),
            after_models=(("orders.sql", _ORDERS), ("payments.sql", _PAYMENTS)),
            edited=("payments.sql",),
            expected_failing=(),
            expected_unattributed=False,
        ),
        FixVerdictTestCase(
            description="an existing error in another file does not block a verified edit",
            before_models=(("orders.sql", _ORDERS), ("payments.sql", _BROKEN_PAYMENTS)),
            after_models=(
                ("orders.sql", _REFORMATTED_ORDERS),
                ("payments.sql", _BROKEN_PAYMENTS),
            ),
            edited=("orders.sql",),
            expected_failing=(),
            expected_unattributed=False,
        ),
        FixVerdictTestCase(
            description="an edit that changes an output type fails its file",
            before_models=(("orders.sql", _ORDERS), ("customers.sql", _CUSTOMERS)),
            after_models=(
                (
                    "orders.sql",
                    'MODEL (description "Orders");\nSELECT CAST(1 AS VARCHAR) AS order_id\n',
                ),
                ("customers.sql", _CUSTOMERS),
            ),
            edited=("orders.sql",),
            expected_failing=(
                ("orders.sql", "The fix would change output columns, types or nullability"),
            ),
            expected_unattributed=True,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_baseline_and_fixed_compiles_when_comparing_then_failures_belong_to_edited_files(
    test_case: FixVerdictTestCase, tmp_path: Path
) -> None:
    before_dir: Path = tmp_path / "before"
    after_dir: Path = tmp_path / "after"
    before_dir.mkdir()
    after_dir.mkdir()
    before: CompiledProjectView = compile_models(
        project_dir=before_dir, models=test_case.before_models
    )
    after: CompiledProjectView = compile_models(
        project_dir=after_dir, models=test_case.after_models
    )

    verdict: FixVerdict = fix_verdict(
        before=compile_facts(project=before, project_dir=before_dir),
        after=compile_facts(project=after, project_dir=before_dir),
        edited=frozenset((before_dir / "sql" / "models" / name).resolve() for name in test_case.edited),
    )

    assert {path.name: reason for path, reason in verdict.failing.items()} == dict(
        test_case.expected_failing
    )
    assert verdict.unattributed is test_case.expected_unattributed


@pytest.mark.parametrize(
    "test_case",
    (
        FixVerdictTestCase(
            description="lineage absent on both sides counts as unchanged when columns match",
            before_models=(("orders.sql", _ORDERS), ("customers.sql", _CUSTOMERS)),
            after_models=(
                ("orders.sql", _REFORMATTED_ORDERS),
                ("customers.sql", _CUSTOMERS),
            ),
            edited=("orders.sql",),
            expected_failing=(),
            expected_unattributed=False,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_lineage_unavailable_on_both_sides_when_comparing_then_matching_columns_verify(
    test_case: FixVerdictTestCase, tmp_path: Path
) -> None:
    before_dir: Path = tmp_path / "before"
    after_dir: Path = tmp_path / "after"
    before_dir.mkdir()
    after_dir.mkdir()
    before: CompiledProjectView = without_lineage(
        compile_models(project_dir=before_dir, models=test_case.before_models)
    )
    after: CompiledProjectView = without_lineage(
        compile_models(project_dir=after_dir, models=test_case.after_models)
    )

    verdict: FixVerdict = fix_verdict(
        before=compile_facts(project=before, project_dir=before_dir),
        after=compile_facts(project=after, project_dir=before_dir),
        edited=frozenset((before_dir / "sql" / "models" / name).resolve() for name in test_case.edited),
    )

    assert {path.name: reason for path, reason in verdict.failing.items()} == dict(
        test_case.expected_failing
    )
    assert verdict.unattributed is test_case.expected_unattributed


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
