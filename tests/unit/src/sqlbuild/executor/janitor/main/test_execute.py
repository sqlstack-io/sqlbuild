"""Tests for janitor plan execution."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta

import pytest

from sqlbuild._native import JANITOR_EVENTS_TABLE_NAME, build_janitor_plan
from sqlbuild.executor.janitor.main.execute import execute_janitor_plan
from sqlbuild.executor.janitor.models import (
    JanitorExecutionResult,
    JanitorPlan,
)
from tests.unit.src.sqlbuild.executor.janitor.main._test_types import (
    JanitorArchiveEventFailureTestCase,
    JanitorArchiveExecutionTestCase,
    relation_info,
)
from tests.unit.src.sqlbuild.executor.janitor.main.helpers import (
    FailingJanitorEventAdapter,
    FakeJanitorAdapter,
    build_project,
)

OLD_TIME: datetime = datetime.now(UTC) - timedelta(days=30)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorArchiveExecutionTestCase(
            description="renames stale table and view, drops expired archive, and audits each per phase",
            relation_infos=(
                relation_info("old_orders", created_at=OLD_TIME),
                relation_info("old_customers_view", relation_type="VIEW", created_at=OLD_TIME),
                relation_info("_sqb_archive__20200101t000000z__old_products"),
            ),
            archive_retention_days=14,
            expected_renamed_origins=("analytics.old_orders",),
            expected_view_rename_origins=("analytics.old_customers_view",),
            expected_dropped_targets=("analytics._sqb_archive__20200101t000000z__old_products",),
            expected_dropped_view_targets=(),
            expected_event_insert_count=2,
            expected_event_row_count=3,
            expected_event_table_create_count=1,
        ),
        JanitorArchiveExecutionTestCase(
            description="zero archive retention archives then drops in the same run",
            relation_infos=(
                relation_info("old_orders", created_at=OLD_TIME),
                relation_info("old_customers_view", relation_type="VIEW", created_at=OLD_TIME),
            ),
            archive_retention_days=0,
            expected_renamed_origins=("analytics.old_orders",),
            expected_view_rename_origins=("analytics.old_customers_view",),
            expected_dropped_targets=("analytics._sqb_archive__",),
            expected_dropped_view_targets=("analytics._sqb_archive__",),
            expected_event_insert_count=2,
            expected_event_row_count=4,
            expected_event_table_create_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_direct_archive_plan_when_executing_then_renames_drops_and_writes_audit_events(
    test_case: JanitorArchiveExecutionTestCase,
) -> None:
    adapter: FakeJanitorAdapter = FakeJanitorAdapter(relation_infos=test_case.relation_infos)
    plan: JanitorPlan = build_janitor_plan(
        project=build_project(),
        adapter=adapter,
        connection=object(),
        retention_days=7,
        delete_tracked_only=False,
        direct_mode=True,
        archive_retention_days=test_case.archive_retention_days,
    )

    result: JanitorExecutionResult = execute_janitor_plan(
        plan=plan, adapter=adapter, connection=object()
    )

    assert tuple(origin for origin, _ in adapter.renamed_targets) == (
        test_case.expected_renamed_origins
    )
    view_rename_statements: tuple[str, ...] = tuple(
        filter(lambda sql: sql.startswith("ALTER VIEW "), adapter.executed_sql)
    )
    view_renames: tuple[str, ...] = tuple(
        sql.removeprefix("ALTER VIEW ").split(" RENAME TO ")[0] for sql in view_rename_statements
    )
    assert view_renames == test_case.expected_view_rename_origins
    assert len(adapter.dropped_targets) == len(test_case.expected_dropped_targets)
    for dropped, expected_prefix in zip(
        adapter.dropped_targets, test_case.expected_dropped_targets, strict=True
    ):
        assert dropped.startswith(expected_prefix)
    assert len(adapter.dropped_view_targets) == len(test_case.expected_dropped_view_targets)
    for dropped, expected_prefix in zip(
        adapter.dropped_view_targets, test_case.expected_dropped_view_targets, strict=True
    ):
        assert dropped.startswith(expected_prefix)
    event_statements: tuple[str, ...] = tuple(
        filter(lambda sql: JANITOR_EVENTS_TABLE_NAME in sql, adapter.executed_sql)
    )
    event_inserts: tuple[str, ...] = tuple(
        filter(lambda sql: sql.startswith("INSERT INTO"), event_statements)
    )
    event_creates: tuple[str, ...] = tuple(
        filter(lambda sql: sql.startswith("CREATE TABLE"), event_statements)
    )
    assert len(event_inserts) == test_case.expected_event_insert_count
    assert sum(sql.count("), (") + 1 for sql in event_inserts) == (
        test_case.expected_event_row_count
    )
    assert len(tuple(filter(lambda sql: sql.startswith("SELECT"), event_statements))) == 1
    assert len(event_creates) == test_case.expected_event_table_create_count
    assert tuple(candidate.key.name for candidate in result.archived) == tuple(
        origin.rsplit(".", maxsplit=1)[-1]
        for origin in (*test_case.expected_renamed_origins, *test_case.expected_view_rename_origins)
    )
    assert len(result.deleted_archives) == len(test_case.expected_dropped_targets) + len(
        test_case.expected_dropped_view_targets
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorArchiveEventFailureTestCase(
            description="audit write failure after rename stops before further actions",
            relation_infos=(
                relation_info("old_orders", created_at=OLD_TIME),
                relation_info("_sqb_archive__20200101t000000z__old_products"),
            ),
            expected_error_fragment="simulated janitor event write failure",
            expected_renamed_origins=("analytics.old_orders",),
            expected_dropped_targets=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_audit_write_failure_when_archiving_then_rename_persists_and_error_propagates(
    test_case: JanitorArchiveEventFailureTestCase,
) -> None:
    adapter: FailingJanitorEventAdapter = FailingJanitorEventAdapter(
        relation_infos=test_case.relation_infos
    )
    plan: JanitorPlan = build_janitor_plan(
        project=build_project(),
        adapter=adapter,
        connection=object(),
        retention_days=7,
        delete_tracked_only=False,
        direct_mode=True,
    )

    with pytest.raises(RuntimeError) as exc_info:
        execute_janitor_plan(plan=plan, adapter=adapter, connection=object())

    assert test_case.expected_error_fragment in str(exc_info.value)
    assert tuple(origin for origin, _ in adapter.renamed_targets) == (
        test_case.expected_renamed_origins
    )
    assert tuple(adapter.dropped_targets) == test_case.expected_dropped_targets
