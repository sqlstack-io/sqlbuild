"""Conversions kept on disk: a later run reads them instead of converting again."""

from pathlib import Path

import pytest
import sqlquack

from sqlbuild.executor.local._helpers.conversion import local_work_counts, use_conversion_cache
from sqlbuild.executor.local.models import LocalWorkCounts
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import CachedConversionTestCase
from tests.unit.src.sqlbuild.executor.local._helpers.helpers import (
    ORDERS_SCHEMA_RELATIONS,
    check_each,
    fresh_run,
    work_delta,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CachedConversionTestCase(
            description="two statements, checked one at a time",
            statement_count=2,
            expected_first_conversions=2,
            expected_later_conversions=0,
            expected_later_cached=2,
        ),
        CachedConversionTestCase(
            description="ten statements, enough to convert in worker processes",
            statement_count=10,
            expected_first_conversions=10,
            expected_later_conversions=0,
            expected_later_cached=10,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_conversions_kept_on_disk_when_a_later_run_checks_them_then_none_convert_again(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, test_case: CachedConversionTestCase
) -> None:
    schema: dict[str, object] = {"relations": dict(ORDERS_SCHEMA_RELATIONS)}
    statements: tuple[str, ...] = tuple(
        f"SELECT id + {index} AS id FROM shop.raw.orders"
        for index in range(test_case.statement_count)
    )
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    first: LocalWorkCounts = local_work_counts()
    _pooled, first_checks = check_each(
        statements=statements, schema=schema, repeats=1, convert=True
    )
    first_work: LocalWorkCounts = work_delta(first)
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    later: LocalWorkCounts = local_work_counts()

    _pooled, later_checks = check_each(
        statements=statements, schema=schema, repeats=1, convert=True
    )

    later_work: LocalWorkCounts = work_delta(later)
    assert first_work.conversions == test_case.expected_first_conversions
    assert later_work.conversions == test_case.expected_later_conversions
    assert later_work.cached == test_case.expected_later_cached
    assert later_checks == first_checks


@pytest.mark.parametrize(
    "test_case",
    [
        CachedConversionTestCase(
            description="another SQLQuack version converts again",
            statement_count=2,
            expected_first_conversions=2,
            expected_later_conversions=2,
            expected_later_cached=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_another_sqlquack_version_when_checking_then_kept_conversions_are_not_read(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, test_case: CachedConversionTestCase
) -> None:
    schema: dict[str, object] = {"relations": dict(ORDERS_SCHEMA_RELATIONS)}
    statements: tuple[str, ...] = tuple(
        f"SELECT id * {index} AS id FROM shop.raw.orders"
        for index in range(test_case.statement_count)
    )
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    first: LocalWorkCounts = local_work_counts()
    _ = check_each(statements=statements, schema=schema, repeats=1, convert=True)
    first_work: LocalWorkCounts = work_delta(first)
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    monkeypatch.setattr(sqlquack, "__version__", f"{sqlquack.__version__}.other")
    later: LocalWorkCounts = local_work_counts()

    _ = check_each(statements=statements, schema=schema, repeats=1, convert=True)

    later_work: LocalWorkCounts = work_delta(later)
    assert first_work.conversions == test_case.expected_first_conversions
    assert later_work.conversions == test_case.expected_later_conversions
    assert later_work.cached == test_case.expected_later_cached


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
