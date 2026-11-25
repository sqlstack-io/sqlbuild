"""Local test checks scale with unique statements: each converts or binds once per run."""

import uuid

import pytest

from sqlbuild.executor.local._helpers.conversion import local_work_counts
from sqlbuild.executor.local.models import LocalWorkCounts
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import (
    BindingWorkTestCase,
    ConversionWorkTestCase,
    PooledConversionWorkTestCase,
)
from tests.unit.src.sqlbuild.executor.local._helpers.helpers import (
    ORDERS_SCHEMA_RELATIONS,
    check_each,
    counting_convert,
    work_delta,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConversionWorkTestCase(
            description="two tests read three times convert twice, serialising the schema once",
            statements=(
                "SELECT id FROM shop.raw.orders WHERE amount > 1",
                "SELECT id, amount FROM shop.raw.orders",
            ),
            repeats=3,
            expected_conversions=2,
            expected_schema_keys=1,
            expected_relations=frozenset({"SHOP.RAW.ORDERS"}),
        ),
        ConversionWorkTestCase(
            description="a repeated statement converts once",
            statements=("SELECT id FROM shop.raw.customers",) * 4,
            repeats=2,
            expected_conversions=1,
            expected_schema_keys=1,
            expected_relations=frozenset({"SHOP.RAW.CUSTOMERS"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_tests_when_checking_locally_then_each_statement_converts_once_with_its_relations(
    monkeypatch: pytest.MonkeyPatch, test_case: ConversionWorkTestCase
) -> None:
    calls: list[frozenset[str]] = counting_convert(monkeypatch)
    schema: dict[str, object] = {"relations": dict(ORDERS_SCHEMA_RELATIONS)}
    tag: str = uuid.uuid4().hex
    statements: tuple[str, ...] = tuple(f"{sql} /* {tag} */" for sql in test_case.statements)
    before: LocalWorkCounts = local_work_counts()

    _pooled, checks = check_each(
        statements=statements, schema=schema, repeats=test_case.repeats, convert=True
    )

    delta: LocalWorkCounts = work_delta(before)
    assert len(checks) == len(statements) * test_case.repeats
    assert len(calls) == test_case.expected_conversions
    assert delta.conversions == test_case.expected_conversions
    assert delta.schema_keys == test_case.expected_schema_keys
    assert delta.bindings == 0
    assert set(calls) == {test_case.expected_relations}


@pytest.mark.parametrize(
    "test_case",
    [
        PooledConversionWorkTestCase(
            description="ten tests convert ten times in workers, none again when the tests run",
            statement_count=10,
            expected_conversions=10,
            expected_schema_keys=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_many_tests_when_checking_in_workers_then_running_them_converts_nothing_more(
    test_case: PooledConversionWorkTestCase,
) -> None:
    schema: dict[str, object] = {"relations": dict(ORDERS_SCHEMA_RELATIONS)}
    tag: str = uuid.uuid4().hex
    statements: tuple[str, ...] = tuple(
        f"SELECT id + {index} AS id FROM shop.raw.orders /* {tag} */"
        for index in range(test_case.statement_count)
    )
    before: LocalWorkCounts = local_work_counts()

    pooled, _checks = check_each(statements=statements, schema=schema, repeats=2, convert=True)

    delta: LocalWorkCounts = work_delta(before)
    assert pooled == test_case.expected_conversions
    assert delta.conversions == test_case.expected_conversions
    assert delta.schema_keys == test_case.expected_schema_keys


@pytest.mark.parametrize(
    "test_case",
    [
        BindingWorkTestCase(
            description="DuckDB statements bind once each and name a relation only DuckDB lacks",
            statements=("SELECT 1 AS one", "SELECT * FROM nowhere_table"),
            repeats=3,
            expected_bindings=2,
            expected_missing=(None, "nowhere_table"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_tests_when_checking_locally_then_each_binds_once_in_an_empty_database(
    test_case: BindingWorkTestCase,
) -> None:
    tag: str = uuid.uuid4().hex
    statements: tuple[str, ...] = tuple(f"{sql} /* {tag} */" for sql in test_case.statements)
    before: LocalWorkCounts = local_work_counts()

    _pooled, checks = check_each(
        statements=statements, schema=None, repeats=test_case.repeats, convert=False
    )

    delta: LocalWorkCounts = work_delta(before)
    assert delta.bindings == test_case.expected_bindings
    assert delta.conversions == 0
    assert tuple(check.missing for check in checks[: len(statements)]) == test_case.expected_missing


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
