"""Kept conversions that no run used for a week are deleted when a run ends."""

from pathlib import Path

import pytest

from sqlbuild.executor.local._helpers.conversion import (
    prune_conversion_cache,
    use_conversion_cache,
)
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import CachePruningTestCase
from tests.unit.src.sqlbuild.executor.local._helpers.helpers import (
    ORDERS_SCHEMA_RELATIONS,
    check_each,
    fresh_run,
)

_FIRST_RUN: float = 1_000_000_000.0
_DAY: float = 24 * 60 * 60


@pytest.mark.parametrize(
    "test_case",
    [
        CachePruningTestCase(
            description="unused for eight days",
            statement_count=3,
            later_used_count=1,
            days_later=8,
            expected_pruned=2,
            expected_kept=1,
        ),
        CachePruningTestCase(
            description="unused for six days",
            statement_count=3,
            later_used_count=1,
            days_later=6,
            expected_pruned=0,
            expected_kept=3,
        ),
        CachePruningTestCase(
            description="all used again",
            statement_count=3,
            later_used_count=3,
            days_later=30,
            expected_pruned=0,
            expected_kept=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_kept_conversions_when_a_run_ends_then_those_unused_for_a_week_are_deleted(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, test_case: CachePruningTestCase
) -> None:
    schema: dict[str, object] = {"relations": dict(ORDERS_SCHEMA_RELATIONS)}
    statements: tuple[str, ...] = tuple(
        f"SELECT id - {index} AS id FROM shop.raw.orders"
        for index in range(test_case.statement_count)
    )
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    _ = check_each(statements=statements, schema=schema, repeats=1, convert=True)
    first_pruned: int = prune_conversion_cache(now=_FIRST_RUN)
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    _ = check_each(
        statements=statements[: test_case.later_used_count], schema=schema, repeats=1, convert=True
    )

    later_pruned: int = prune_conversion_cache(now=_FIRST_RUN + test_case.days_later * _DAY)

    assert first_pruned == 0
    assert later_pruned == test_case.expected_pruned
    assert len(list(tmp_path.glob("*/*.json"))) == test_case.expected_kept
