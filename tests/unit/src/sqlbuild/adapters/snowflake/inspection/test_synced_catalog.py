"""A plan from synced state lists and describes schemas as a live read would, checking first."""

from __future__ import annotations

from datetime import UTC, datetime

import pytest

from sqlbuild.adapter.relations.classes.inspection_catalog import InspectionCatalog
from sqlbuild.adapter.relations.classes.synced_catalog import SyncedCatalog
from sqlbuild.adapter.relations.constants import SYNCED_CATALOG_ATTRIBUTE
from tests.unit.src.sqlbuild.adapters.snowflake.inspection._test_types import (
    SyncedCatalogTestCase,
    SyncedFallbackTestCase,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeColumn,
    FakeRelation,
    ListedSchema,
    build_offline_snowflake,
    listed_schema,
    live_inspection_catalog,
    relations_after_sync,
    synced_copy,
    unavailable_relation_changes,
)

_DATABASE: str = "analytics"
_SCHEMA: str = "staging"
_SYNCED_AT: datetime = datetime(2026, 2, 1, tzinfo=UTC)
_AFTER_SYNC: datetime = datetime(2026, 3, 1, tzinfo=UTC)
_RELATIONS: tuple[FakeRelation, ...] = (
    FakeRelation(
        database="ANALYTICS",
        schema="STAGING",
        name="ORDERS",
        columns=(FakeColumn(name="ID", data_type="NUMBER"), FakeColumn(name="STATUS")),
    ),
    FakeRelation(database="ANALYTICS", schema="STAGING", name="CUSTOMERS"),
    FakeRelation(database="ANALYTICS", schema="STAGING", name="ORDERS_V", table_type="VIEW"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        SyncedCatalogTestCase(
            description="an unchanged warehouse is checked once and described from the copy",
            changed=(),
            dropped=(),
            expected_query_kinds=("tables",),
            expected_refreshed=0,
        ),
        SyncedCatalogTestCase(
            description="a table altered after the copy is the only one described again",
            changed=(
                FakeRelation(
                    database="ANALYTICS",
                    schema="STAGING",
                    name="ORDERS",
                    columns=(
                        FakeColumn(name="ID", data_type="NUMBER"),
                        FakeColumn(name="STATUS"),
                        FakeColumn(name="REGION"),
                    ),
                    last_ddl=_AFTER_SYNC,
                ),
            ),
            dropped=(),
            expected_query_kinds=("tables", "show_columns"),
            expected_refreshed=1,
        ),
        SyncedCatalogTestCase(
            description="a table created after the copy is listed and described",
            changed=(
                FakeRelation(
                    database="ANALYTICS", schema="STAGING", name="PAYMENTS", last_ddl=_AFTER_SYNC
                ),
            ),
            dropped=(),
            expected_query_kinds=("tables", "show_columns"),
            expected_refreshed=1,
        ),
        SyncedCatalogTestCase(
            description="a table dropped after the copy is no longer listed",
            changed=(),
            dropped=("CUSTOMERS",),
            expected_query_kinds=("tables",),
            expected_refreshed=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_synced_copy_when_listing_a_schema_then_matches_a_live_read_after_one_check(
    test_case: SyncedCatalogTestCase,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(relations=_RELATIONS)
    synced: SyncedCatalog = synced_copy(
        warehouse=warehouse, database=_DATABASE, schema=_SCHEMA, observed_at=_SYNCED_AT
    )
    warehouse.replace_relations(
        relations=relations_after_sync(
            relations=_RELATIONS, changed=test_case.changed, dropped=test_case.dropped
        )
    )
    warehouse.queries.clear()
    setattr(adapter, SYNCED_CATALOG_ATTRIBUTE, synced)

    planned: ListedSchema = listed_schema(
        catalog=InspectionCatalog(adapter=adapter, connection=connection),
        database=_DATABASE,
        schema=_SCHEMA,
    )

    assert sorted(query.kind for query in warehouse.queries) == sorted(
        test_case.expected_query_kinds
    )
    assert planned == listed_schema(
        catalog=live_inspection_catalog(warehouse), database=_DATABASE, schema=_SCHEMA
    )
    noun: str = ("tables", "table")[test_case.expected_refreshed == 1]
    report: str | None = synced.report()
    assert report is not None and report.startswith("Planned from synced state: checked in ")
    assert report.endswith(f", {test_case.expected_refreshed} {noun} refreshed")


@pytest.mark.parametrize(
    "test_case",
    [
        SyncedFallbackTestCase(
            description="a failing check reads the schema live and says why",
            empty_copy=False,
            failing_check=True,
            expected_message="Planning live: the synced state check failed",
        ),
        SyncedFallbackTestCase(
            description="a schema outside the copy is read live without a check",
            empty_copy=True,
            failing_check=False,
            expected_message="",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unusable_copy_when_listing_then_reads_the_schema_live(
    test_case: SyncedFallbackTestCase,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    adapter, connection, warehouse = build_offline_snowflake(relations=_RELATIONS)
    copies: tuple[SyncedCatalog, SyncedCatalog] = (
        synced_copy(
            warehouse=warehouse, database=_DATABASE, schema=_SCHEMA, observed_at=_SYNCED_AT
        ),
        SyncedCatalog(relations=()),
    )
    setattr(adapter, SYNCED_CATALOG_ATTRIBUTE, copies[test_case.empty_copy])
    monkeypatch.setattr(
        adapter,
        "read_relation_changes",
        (adapter.read_relation_changes, unavailable_relation_changes)[test_case.failing_check],
    )
    warehouse.queries.clear()

    planned: ListedSchema = listed_schema(
        catalog=InspectionCatalog(adapter=adapter, connection=connection),
        database=_DATABASE,
        schema=_SCHEMA,
    )

    assert {"show_tables", "show_views"} <= {query.kind for query in warehouse.queries}
    assert "tables" not in {query.kind for query in warehouse.queries}
    assert planned == listed_schema(
        catalog=live_inspection_catalog(warehouse), database=_DATABASE, schema=_SCHEMA
    )
    assert test_case.expected_message in capsys.readouterr().err
