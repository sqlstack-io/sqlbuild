"""Conversions kept on disk: a later run reads them instead of converting again."""

from pathlib import Path

import pytest
import sqlquack

from sqlbuild.executor.local._helpers.conversion import use_conversion_cache, use_local_inputs
from sqlbuild.executor.local._helpers.local_inputs import local_inputs
from sqlbuild.executor.local.models import LocalWorkCounts
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import (
    CachedConversionTestCase,
    LocalInputsCacheTestCase,
)
from tests.unit.src.sqlbuild.executor.local._helpers.helpers import (
    ORDERS_SCHEMA_RELATIONS,
    check_each,
    fresh_run,
    work_counts,
    work_delta,
)

_HASH_OVERRIDE: str = (
    "FUNCTION (\n  arguments (x VARCHAR),\n  returns NUMBER(19,0),\n  replaces HASH,\n"
    "  dialect duckdb\n);\n{body}\n"
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
    first: LocalWorkCounts = work_counts()
    _pooled, first_checks = check_each(
        statements=statements, schema=schema, repeats=1, convert=True
    )
    first_work: LocalWorkCounts = work_delta(first)
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    later: LocalWorkCounts = work_counts()

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
    first: LocalWorkCounts = work_counts()
    _ = check_each(statements=statements, schema=schema, repeats=1, convert=True)
    first_work: LocalWorkCounts = work_delta(first)
    fresh_run(monkeypatch)
    use_conversion_cache(tmp_path)
    monkeypatch.setattr(sqlquack, "__version__", f"{sqlquack.__version__}.other")
    later: LocalWorkCounts = work_counts()

    _ = check_each(statements=statements, schema=schema, repeats=1, convert=True)

    later_work: LocalWorkCounts = work_delta(later)
    assert first_work.conversions == test_case.expected_first_conversions
    assert later_work.conversions == test_case.expected_later_conversions
    assert later_work.cached == test_case.expected_later_cached


@pytest.mark.parametrize(
    "test_case",
    [
        LocalInputsCacheTestCase(
            description="the same allowances and overrides read the kept conversions",
            first=({"hash": True}, {"hash.sql": _HASH_OVERRIDE.format(body="hash(x)")}),
            later=({"hash": True}, {"hash.sql": _HASH_OVERRIDE.format(body="hash(x)")}),
            expected_later_conversions=0,
            expected_later_cached=2,
        ),
        LocalInputsCacheTestCase(
            description="another allowance converts again",
            first=({}, {}),
            later=({"hash": True}, {}),
            expected_later_conversions=2,
            expected_later_cached=0,
        ),
        LocalInputsCacheTestCase(
            description="an edited override converts again",
            first=({}, {"hash.sql": _HASH_OVERRIDE.format(body="hash(x)")}),
            later=({}, {"hash.sql": _HASH_OVERRIDE.format(body="hash(x) + 0")}),
            expected_later_conversions=2,
            expected_later_cached=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_local_test_inputs_when_a_later_run_checks_then_only_the_same_inputs_read_the_cache(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, test_case: LocalInputsCacheTestCase
) -> None:
    schema: dict[str, object] = {"relations": dict(ORDERS_SCHEMA_RELATIONS)}
    statements: tuple[str, ...] = tuple(
        f"SELECT id - {index} AS id FROM shop.raw.orders" for index in range(2)
    )
    project: Path = tmp_path / "project"
    overrides: Path = project / "tests" / "overrides"
    overrides.mkdir(parents=True)
    runs: list[LocalWorkCounts] = []
    for allow, files in (test_case.first, test_case.later):
        for stale in overrides.iterdir():
            stale.unlink()
        for name, text in files.items():
            (overrides / name).write_text(text)
        fresh_run(monkeypatch)
        use_conversion_cache(tmp_path / "cache")
        use_local_inputs(local_inputs(project_dir=project, allow=allow))
        before: LocalWorkCounts = work_counts()
        _ = check_each(statements=statements, schema=schema, repeats=1, convert=True)
        runs.append(work_delta(before))

    assert (runs[1].conversions, runs[1].cached) == (
        test_case.expected_later_conversions,
        test_case.expected_later_cached,
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
