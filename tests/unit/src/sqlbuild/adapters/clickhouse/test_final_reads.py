from __future__ import annotations

import pytest

from sqlbuild._native import Dialect, SourceEntry, render_source_read_relation
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.discovery.main.builtins import builtin_adapter_classes
from tests.unit.src.sqlbuild.adapters.clickhouse._test_types import FinalSourceReadTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        FinalSourceReadTestCase(
            description="clickhouse reads the table with FINAL",
            adapter_name="clickhouse",
            expected_relation="(SELECT * FROM raw.customers FINAL)",
            expected_code=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_final_source_when_rendering_on_clickhouse_then_read_is_deduplicated(
    test_case: FinalSourceReadTestCase,
) -> None:
    adapter: BaseAdapter = builtin_adapter_classes()[test_case.adapter_name]()
    entry: SourceEntry = SourceEntry(name="customers", schema="raw", table="customers", final=True)

    assert render_source_read_relation(dialect=Dialect(adapter), source_entry=entry) == (
        test_case.expected_relation
    )


@pytest.mark.parametrize(
    "test_case",
    [
        FinalSourceReadTestCase(
            description="duckdb refuses final",
            adapter_name="duckdb",
            expected_relation=None,
            expected_code="A010",
        ),
        FinalSourceReadTestCase(
            description="postgres refuses final",
            adapter_name="postgres",
            expected_relation=None,
            expected_code="A010",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_final_source_when_rendering_on_other_adapter_then_coded_error(
    test_case: FinalSourceReadTestCase,
) -> None:
    adapter: BaseAdapter = builtin_adapter_classes()[test_case.adapter_name]()
    entry: SourceEntry = SourceEntry(name="customers", schema="raw", table="customers", final=True)

    with pytest.raises(AdapterUserError) as error:
        render_source_read_relation(dialect=Dialect(adapter), source_entry=entry)

    assert error.value.code == test_case.expected_code
