"""Housekeeping statements of an offline Snowflake build, and the cases each one guards.

The simulated warehouse fails like Snowflake when a statement names an object of the other kind,
so a build that drops or replaces only the object kind it created is checked against the same
errors a real account would raise.
"""

from __future__ import annotations

import re
from functools import partial
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.executor.build.snowflake_metadata._test_types import (
    ExternalChangeTestCase,
    MaterializationSwitchTestCase,
    RetentionLookupTestCase,
)
from tests.integration.src.sqlbuild.executor.build.snowflake_metadata.helpers import (
    OfflineBuild,
    SimulatedSnowflakeWarehouse,
    build_offline_snowflake_project,
    create_external_table,
    drop_external_relation,
    failed_models,
    information_schema_statements,
    matching_statements,
    orders_warehouse,
    relation_types,
    retention_by_prefix,
    write_model_copies,
    write_orders_project,
)

_RETENTION_ALTER: re.Pattern[str] = re.compile(
    r"^ALTER TABLE \S+ SET DATA_RETENTION_TIME_IN_DAYS = (?P<days>\d+)$"
)
_TABLE_LOOKUP: re.Pattern[str] = re.compile(r"^SHOW TABLES LIKE ")
_SCHEMA_LISTING: re.Pattern[str] = re.compile(r"^SHOW (?:TABLES|VIEWS) IN SCHEMA ")
_QUERY_TAG: re.Pattern[str] = re.compile(r"^ALTER SESSION SET QUERY_TAG")
_OWNED_VIEW_DROP: re.Pattern[str] = re.compile(r"^DROP VIEW IF EXISTS \S+__(?:staging|delta)$")


@pytest.mark.parametrize(
    "test_case",
    [
        MaterializationSwitchTestCase(
            description="a view becomes a table and then a view again",
            materializations=("view", "table", "view"),
            expected_table_types=("VIEW", "BASE TABLE", "VIEW"),
        ),
        MaterializationSwitchTestCase(
            description="a table becomes a view and then a table again",
            materializations=("table", "view", "table"),
            expected_table_types=("BASE TABLE", "VIEW", "BASE TABLE"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_materialization_switch_when_building_then_each_kind_replaces_the_other(
    test_case: MaterializationSwitchTestCase, tmp_path: Path
) -> None:
    write_orders_project(project_dir=tmp_path)
    warehouse: SimulatedSnowflakeWarehouse = orders_warehouse()
    model: Path = tmp_path / "sql" / "models" / "marts" / "orders_view.sql"
    source: str = model.read_text(encoding="utf-8")
    table_types: list[tuple[str, ...]] = []
    builds: list[OfflineBuild] = []

    materialization: str
    for materialization in test_case.materializations:
        _ = model.write_text(
            re.sub(r"materialized \w+", f"materialized {materialization}", source),
            encoding="utf-8",
        )
        builds.append(
            build_offline_snowflake_project(
                project_dir=tmp_path, warehouse=warehouse, max_concurrency=1
            )
        )
        table_types.append(relation_types(warehouse, "ORDERS_VIEW"))

    assert [failed_models(build) for build in builds] == [()] * len(builds)
    assert tuple(table_types) == tuple((kind,) for kind in test_case.expected_table_types)
    assert tuple(matching_statements(build.statements, _OWNED_VIEW_DROP) for build in builds) == (
        test_case.expected_owned_view_drops,
    ) * len(builds)


@pytest.mark.parametrize(
    "test_case",
    [
        RetentionLookupTestCase(
            description="six tables in one schema keep their retention without INFORMATION_SCHEMA",
            table_count=6,
            retention_days=3,
            expected_first_alters=6,
            expected_max_table_lookups=7,
            expected_max_listings=4,
        ),
        RetentionLookupTestCase(
            description="a query tag set before each table keeps the schema listings",
            table_count=6,
            retention_days=3,
            header_options=("pre_hooks [inline_sql(\"ALTER SESSION SET QUERY_TAG = 'orders'\")], "),
            expected_first_alters=6,
            expected_max_table_lookups=7,
            expected_max_listings=4,
            expected_query_tags=6,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_many_tables_in_one_schema_when_building_then_retention_is_read_with_show(
    test_case: RetentionLookupTestCase, tmp_path: Path
) -> None:
    write_orders_project(project_dir=tmp_path)
    write_model_copies(
        project_dir=tmp_path,
        count=test_case.table_count,
        retention_days=test_case.retention_days,
        header_options=test_case.header_options,
    )
    warehouse: SimulatedSnowflakeWarehouse = orders_warehouse()

    first: OfflineBuild = build_offline_snowflake_project(
        project_dir=tmp_path, warehouse=warehouse, max_concurrency=4
    )
    second: OfflineBuild = build_offline_snowflake_project(
        project_dir=tmp_path, warehouse=warehouse, max_concurrency=4
    )

    copies: dict[str, int] = retention_by_prefix(warehouse, "ORDER_COPY_")
    assert (failed_models(first), failed_models(second)) == ((), ())
    assert (information_schema_statements(first), information_schema_statements(second)) == (
        test_case.expected_information_schema,
        test_case.expected_information_schema,
    )
    assert copies == dict.fromkeys(copies, test_case.retention_days)
    assert len(copies) == test_case.table_count
    assert len(matching_statements(first.statements, _RETENTION_ALTER)) >= (
        test_case.expected_first_alters
    )
    assert max(
        len(matching_statements(build.statements, _TABLE_LOOKUP)) for build in (first, second)
    ) <= (test_case.expected_max_table_lookups)
    assert max(
        len(matching_statements(build.statements, _SCHEMA_LISTING)) for build in (first, second)
    ) <= (test_case.expected_max_listings)
    assert len(matching_statements(first.statements, _QUERY_TAG)) == (test_case.expected_query_tags)


@pytest.mark.parametrize(
    "test_case",
    [
        ExternalChangeTestCase(
            description="another session drops the table the build is about to swap",
            builds_before=1,
            change=partial(drop_external_relation, name="ORDERS_TABLE"),
            before_statement=r"^ALTER TABLE \S+ SWAP WITH ",
            expected_table_types=("BASE TABLE",),
        ),
        ExternalChangeTestCase(
            description="another session creates the table the build is about to rename into",
            builds_before=0,
            change=partial(create_external_table, name="ORDERS_TABLE"),
            before_statement=r"^ALTER TABLE \S+__staging RENAME TO ",
            expected_table_types=("BASE TABLE",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_external_change_before_promotion_when_building_then_promotion_reads_it_again(
    test_case: ExternalChangeTestCase, tmp_path: Path
) -> None:
    write_orders_project(project_dir=tmp_path)
    warehouse: SimulatedSnowflakeWarehouse = orders_warehouse()
    earlier: tuple[OfflineBuild, ...] = tuple(
        build_offline_snowflake_project(
            project_dir=tmp_path, warehouse=warehouse, max_concurrency=1
        )
        for _ in range(test_case.builds_before)
    )
    warehouse.external_changes.append((re.compile(test_case.before_statement), test_case.change))
    model: Path = tmp_path / "sql" / "models" / "marts" / "orders_table.sql"
    _ = model.write_text(
        model.read_text(encoding="utf-8").replace("SELECT ", "SELECT 1 AS revision, ", 1),
        encoding="utf-8",
    )

    build: OfflineBuild = build_offline_snowflake_project(
        project_dir=tmp_path, warehouse=warehouse, max_concurrency=1
    )

    assert tuple(failed_models(done) for done in (*earlier, build)) == ((),) * (len(earlier) + 1)
    assert warehouse.external_changes == []
    assert relation_types(warehouse, "ORDERS_TABLE") == test_case.expected_table_types


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
