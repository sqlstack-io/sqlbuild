"""A Snowflake build's run reuses the schema listings its planning read, through the real CLI."""

from __future__ import annotations

import re
from functools import partial
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.executor.build.snowflake_metadata._test_types import (
    PlannerListingReuseTestCase,
)
from tests.integration.src.sqlbuild.executor.build.snowflake_metadata.helpers import (
    SimulatedSnowflakeWarehouse,
    build_with_cli,
    create_external_table,
    orders_warehouse,
    relation_types,
    schema_listing_counts,
    write_orders_project,
)

_SCHEMA_LISTING: re.Pattern[str] = re.compile(
    r"^SHOW (?P<kind>TABLES|VIEWS) IN SCHEMA (?P<scope>\S+)"
)
_PLANNER_LISTING_MAX_AGE: str = (
    "sqlbuild.adapter.relations.classes.inspection_catalog.SCHEMA_LISTING_MAX_AGE_SECONDS"
)
_ONE_LISTING_EACH: dict[str, int] = {
    'TABLES "ANALYTICS"."MARTS"': 1,
    'TABLES "ANALYTICS"."RAW"': 1,
    'VIEWS "ANALYTICS"."MARTS"': 1,
    'VIEWS "ANALYTICS"."RAW"': 1,
}

_RELISTED_MARTS: dict[str, int] = {
    **_ONE_LISTING_EACH,
    'TABLES "ANALYTICS"."MARTS"': 2,
    'VIEWS "ANALYTICS"."MARTS"': 2,
}


@pytest.mark.parametrize(
    "test_case",
    [
        PlannerListingReuseTestCase(
            description="a first build and a rebuild list each schema once",
            expected_listings=(_ONE_LISTING_EACH, _ONE_LISTING_EACH),
            expected_table_types=("BASE TABLE",),
        ),
        PlannerListingReuseTestCase(
            description="a planning listing past its age is read again by the run",
            listing_max_age_seconds=0.0,
            expected_listings=(_RELISTED_MARTS, _RELISTED_MARTS),
            expected_table_types=("BASE TABLE",),
        ),
        PlannerListingReuseTestCase(
            description="another session creates a model's table between planning and the run",
            external_changes=(
                (
                    re.compile(r"^CREATE SCHEMA IF NOT EXISTS "),
                    partial(create_external_table, name="ORDERS_TABLE"),
                ),
            ),
            expected_listings=(_ONE_LISTING_EACH, _ONE_LISTING_EACH),
            expected_table_types=("BASE TABLE",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_planned_build_when_running_then_run_reuses_planner_schema_listings(
    test_case: PlannerListingReuseTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_orders_project(project_dir=tmp_path)
    warehouse: SimulatedSnowflakeWarehouse = orders_warehouse()
    monkeypatch.setattr(_PLANNER_LISTING_MAX_AGE, test_case.listing_max_age_seconds)
    warehouse.external_changes.extend(test_case.external_changes)

    builds: tuple[tuple[int, tuple[str, ...]], ...] = tuple(
        build_with_cli(project_dir=tmp_path, warehouse=warehouse, monkeypatch=monkeypatch)
        for _ in test_case.expected_listings
    )

    output: str = capsys.readouterr().out
    assert tuple(build[0] for build in builds) == (0,) * len(builds), output
    assert warehouse.external_changes == []
    assert tuple(schema_listing_counts(build[1]) for build in builds) == test_case.expected_listings
    assert relation_types(warehouse, "ORDERS_TABLE") == test_case.expected_table_types


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
