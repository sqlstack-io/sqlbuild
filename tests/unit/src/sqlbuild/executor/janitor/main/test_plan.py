from __future__ import annotations

from datetime import UTC, datetime, timedelta

import pytest

from sqlbuild._native import (
    FINGERPRINT_TABLE_NAME,
    SOURCE_FRESHNESS_TABLE_NAME,
    build_janitor_plan,
    build_scenario_artifact_name,
    compute_scenario_hash_prefix,
)
from sqlbuild.executor.janitor.main.execute import execute_janitor_plan
from sqlbuild.executor.janitor.models import (
    JanitorExecutionResult,
    JanitorPlan,
)
from tests.unit.src.sqlbuild.executor.janitor.main._test_types import (
    JanitorExecuteTestCase,
    JanitorPlanTestCase,
    JanitorTargetSchemaTestCase,
    relation_info,
)
from tests.unit.src.sqlbuild.executor.janitor.main.helpers import (
    FakeJanitorAdapter,
    build_project,
)

OLD_TIME: datetime = datetime.now(UTC) - timedelta(days=30)
NEW_TIME: datetime = datetime.now(UTC) - timedelta(days=1)
NAMESPACED_ARTIFACT: str = build_scenario_artifact_name(
    hash_prefix=compute_scenario_hash_prefix(
        project_name="orders", scenario_name="totals", run_namespace="job-1"
    ),
    kind="model",
    logical_name="orders",
)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorPlanTestCase(
            description="extra old relation is eligible",
            relation_infos=(
                relation_info("orders", created_at=OLD_TIME),
                relation_info("old_orders", created_at=OLD_TIME),
            ),
            expected_candidate_names=("old_orders",),
        ),
        JanitorPlanTestCase(
            description="desired model and seed are retained",
            relation_infos=(
                relation_info("orders", created_at=OLD_TIME),
                relation_info("countries", created_at=OLD_TIME),
            ),
            expected_candidate_names=(),
        ),
        JanitorPlanTestCase(
            description="direct schema with active source is blocked",
            relation_infos=(relation_info("old_orders", created_at=OLD_TIME),),
            source_schema="analytics",
            expected_candidate_names=(),
            expected_blocked_schema_sources=("raw_orders",),
            expected_suppressed_candidate_names=("old_orders",),
        ),
        JanitorPlanTestCase(
            description="new relation is skipped by retention",
            relation_infos=(relation_info("old_orders", created_at=NEW_TIME),),
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("relation is newer than 7 days",),
        ),
        JanitorPlanTestCase(
            description="unknown age skipped when retention is enabled",
            relation_infos=(relation_info("old_orders"),),
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("relation age is unavailable",),
        ),
        JanitorPlanTestCase(
            description="adapter without age metadata skips when retention is enabled",
            relation_infos=(relation_info("old_orders"),),
            supports_age_metadata=False,
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("adapter does not expose relation age metadata",),
        ),
        JanitorPlanTestCase(
            description="retention zero allows unknown age",
            relation_infos=(relation_info("old_orders"),),
            retention_days=0,
            supports_age_metadata=False,
            expected_candidate_names=("old_orders",),
        ),
        JanitorPlanTestCase(
            description="fingerprint table is excluded by default",
            relation_infos=(relation_info("_sqlbuild_fingerprints", created_at=OLD_TIME),),
            expected_candidate_names=(),
            expected_direct_state_table_names=(FINGERPRINT_TABLE_NAME,),
            expected_skipped_relation_reasons=(
                "relation matches exclude pattern '_sqlbuild_fingerprints'",
            ),
        ),
        JanitorPlanTestCase(
            description="SQLLoad's own tables are excluded by default",
            relation_infos=(
                relation_info("_sqlload_runs", created_at=OLD_TIME),
                relation_info("_SQLLOAD_UNFINISHED", created_at=OLD_TIME),
                relation_info("__sqlload_stage_probe", created_at=OLD_TIME),
                relation_info("orders__sqlload_staging_20261126t0300_ab12", created_at=OLD_TIME),
                relation_info(
                    "orders__sqlload_staging_20261126t0300_ab12_swap", created_at=OLD_TIME
                ),
                relation_info("orders__sqlload_deletes", created_at=OLD_TIME),
                relation_info("orders_sqlload_archive", created_at=OLD_TIME),
            ),
            expected_candidate_names=("orders_sqlload_archive",),
            expected_skipped_relation_reasons=(
                "relation matches exclude pattern '_sqlload_*'",
                "relation matches exclude pattern '_sqlload_*'",
                "relation matches exclude pattern '__sqlload_*'",
                "relation matches exclude pattern '*__sqlload_staging_*'",
                "relation matches exclude pattern '*__sqlload_staging_*'",
                "relation matches exclude pattern '*__sqlload_deletes'",
            ),
        ),
        JanitorPlanTestCase(
            description="configured exclude pattern skips matching relation",
            relation_infos=(relation_info("partition_state", created_at=OLD_TIME),),
            exclude_patterns=("partition_*",),
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("relation matches exclude pattern 'partition_*'",),
        ),
        JanitorPlanTestCase(
            description="untracked relation is skipped when tracked-only is enabled",
            relation_infos=(relation_info("old_orders", created_at=OLD_TIME),),
            delete_tracked_only=True,
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("relation is not tracked by SQLBuild",),
        ),
        JanitorPlanTestCase(
            description="tracked relation is eligible when tracked-only is enabled",
            relation_infos=(relation_info("old_orders", created_at=OLD_TIME),),
            delete_tracked_only=True,
            tracked_relations=((None, "analytics", "old_orders"),),
            expected_candidate_names=("old_orders",),
            expected_direct_state_table_names=(FINGERPRINT_TABLE_NAME,),
            expected_skipped_relation_reasons=(
                "relation matches exclude pattern '_sqlbuild_fingerprints'",
            ),
        ),
        JanitorPlanTestCase(
            description="scenario artifact is eligible when tracked-only is enabled",
            relation_infos=(
                relation_info("__sqb_a13f09c2e7b8__model__daily_revenue", created_at=OLD_TIME),
            ),
            delete_tracked_only=True,
            expected_candidate_names=("__sqb_a13f09c2e7b8__model__daily_revenue",),
        ),
        JanitorPlanTestCase(
            description="old namespaced scenario artifact is eligible without namespace knowledge",
            relation_infos=(relation_info(NAMESPACED_ARTIFACT, created_at=OLD_TIME),),
            delete_tracked_only=True,
            expected_candidate_names=(NAMESPACED_ARTIFACT,),
        ),
        JanitorPlanTestCase(
            description="recent namespaced scenario artifact is protected by retention",
            relation_infos=(relation_info(NAMESPACED_ARTIFACT, created_at=NEW_TIME),),
            delete_tracked_only=True,
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("relation is newer than 7 days",),
        ),
        JanitorPlanTestCase(
            description="dbt scenario artifact is eligible when tracked-only is enabled",
            relation_infos=(
                relation_info("__sqb_a13f09c2e7b8__dbt_ref__stripe__payments", created_at=OLD_TIME),
            ),
            delete_tracked_only=True,
            expected_candidate_names=("__sqb_a13f09c2e7b8__dbt_ref__stripe__payments",),
        ),
        JanitorPlanTestCase(
            description="scenario-like relation is not eligible when tracked-only is enabled",
            relation_infos=(
                relation_info("__sqb_a13f09c2e7b__model__daily_revenue", created_at=OLD_TIME),
            ),
            delete_tracked_only=True,
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("relation is not tracked by SQLBuild",),
        ),
        JanitorPlanTestCase(
            description="origin of a pending migration is kept",
            relation_infos=(relation_info("old_orders", created_at=OLD_TIME),),
            migrate_from="old_orders",
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("pending migration origin for orders",),
        ),
        JanitorPlanTestCase(
            description="qualified origin of a pending migration is kept",
            relation_infos=(relation_info("OLD_ORDERS", created_at=OLD_TIME),),
            migrate_from="analytics.old_orders",
            expected_candidate_names=(),
            expected_skipped_relation_reasons=("pending migration origin for orders",),
        ),
        JanitorPlanTestCase(
            description="origin is cleaned normally once the destination exists",
            relation_infos=(
                relation_info("orders", created_at=OLD_TIME),
                relation_info("old_orders", created_at=OLD_TIME),
            ),
            migrate_from="old_orders",
            expected_candidate_names=("old_orders",),
        ),
        JanitorPlanTestCase(
            description="existing direct state tables are eligible for history pruning",
            relation_infos=(
                relation_info(FINGERPRINT_TABLE_NAME, created_at=OLD_TIME),
                relation_info(SOURCE_FRESHNESS_TABLE_NAME, created_at=OLD_TIME),
            ),
            direct_state_history_versions=5,
            expected_candidate_names=(),
            expected_direct_state_table_names=(
                FINGERPRINT_TABLE_NAME,
                SOURCE_FRESHNESS_TABLE_NAME,
            ),
            expected_skipped_relation_reasons=(
                "relation matches exclude pattern '_sqlbuild_fingerprints'",
                "relation matches exclude pattern '_sqlbuild_source_freshness'",
            ),
        ),
        JanitorPlanTestCase(
            description="direct state pruning is disabled when history versions is zero",
            relation_infos=(relation_info(FINGERPRINT_TABLE_NAME, created_at=OLD_TIME),),
            direct_state_history_versions=0,
            expected_candidate_names=(),
            expected_direct_state_table_names=(),
            expected_skipped_relation_reasons=(
                "relation matches exclude pattern '_sqlbuild_fingerprints'",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_and_warehouse_when_building_janitor_plan_then_returns_expected_candidates(
    test_case: JanitorPlanTestCase,
) -> None:
    adapter: FakeJanitorAdapter = FakeJanitorAdapter(
        relation_infos=test_case.relation_infos,
        supports_age_metadata=test_case.supports_age_metadata,
        tracked_relations=test_case.tracked_relations,
    )

    plan: JanitorPlan = build_janitor_plan(
        project=build_project(
            source_schema=test_case.source_schema, migrate_from=test_case.migrate_from
        ),
        adapter=adapter,
        connection=object(),
        retention_days=test_case.retention_days,
        delete_tracked_only=test_case.delete_tracked_only,
        exclude_patterns=test_case.exclude_patterns,
        protected_relation_keys=test_case.protected_relation_keys,
        direct_mode=test_case.direct_mode,
        state_history_versions=test_case.direct_state_history_versions,
    )

    assert tuple(candidate.key.name for candidate in plan.candidates) == (
        test_case.expected_candidate_names
    )
    assert tuple(skipped.reason for skipped in plan.skipped_relations) == (
        test_case.expected_skipped_relation_reasons
    )
    assert tuple(candidate.table_name for candidate in plan.direct_state_prune_candidates) == (
        test_case.expected_direct_state_table_names
    )
    skipped_schema_sources: list[str] = []
    for skipped_schema in plan.skipped_schemas:
        for source_name in skipped_schema.source_names:
            skipped_schema_sources.append(source_name)
    assert tuple(skipped_schema_sources) == test_case.expected_skipped_schema_sources
    blocked_schema_sources: list[str] = []
    suppressed_candidate_names: list[str] = []
    for blocked_schema in plan.blocked_schemas:
        blocked_schema_sources.extend(blocked_schema.source_names)
        suppressed_candidate_names.extend(
            candidate.key.name for candidate in blocked_schema.suppressed_candidates
        )
    assert tuple(blocked_schema_sources) == test_case.expected_blocked_schema_sources
    assert tuple(suppressed_candidate_names) == test_case.expected_suppressed_candidate_names


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorTargetSchemaTestCase(
            description="a real target schema is inspected for artifacts",
            target_database=None,
            target_schema="dev",
            location=(None, "dev"),
            expected_inspected=True,
        ),
        JanitorTargetSchemaTestCase(
            description="a preserve target schema names no schema to inspect",
            target_database=None,
            target_schema="preserve",
            location=(None, "preserve"),
            expected_inspected=False,
        ),
        JanitorTargetSchemaTestCase(
            description="a preserve target database names no database for the target schema",
            target_database="preserve",
            target_schema="dev",
            location=(None, "dev"),
            expected_inspected=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_target_schema_when_building_janitor_plan_then_only_a_real_schema_is_inspected(
    test_case: JanitorTargetSchemaTestCase,
) -> None:
    adapter: FakeJanitorAdapter = FakeJanitorAdapter(relation_infos=())

    _ = build_janitor_plan(
        project=build_project(
            target_schema=test_case.target_schema, target_database=test_case.target_database
        ),
        adapter=adapter,
        connection=object(),
        retention_days=7,
        direct_mode=True,
    )

    assert (test_case.location in adapter.listed_locations) == test_case.expected_inspected
    assert all(database != "preserve" for database, _ in adapter.listed_locations)


@pytest.mark.parametrize(
    "test_case",
    [
        JanitorExecuteTestCase(
            description="direct mode archives eligible candidates instead of dropping them",
            relation_infos=(relation_info("old_orders", created_at=OLD_TIME),),
            expected_dropped_targets=(),
            direct_mode=True,
        ),
        JanitorExecuteTestCase(
            description="prunes direct state history tables",
            relation_infos=(relation_info(FINGERPRINT_TABLE_NAME, created_at=OLD_TIME),),
            expected_dropped_targets=(),
            expected_pruned_table_names=(FINGERPRINT_TABLE_NAME,),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_janitor_plan_when_executing_then_drops_expected_relations(
    test_case: JanitorExecuteTestCase,
) -> None:
    adapter: FakeJanitorAdapter = FakeJanitorAdapter(relation_infos=test_case.relation_infos)
    plan: JanitorPlan = build_janitor_plan(
        project=build_project(),
        adapter=adapter,
        connection=object(),
        retention_days=7,
        delete_tracked_only=False,
        direct_mode=test_case.direct_mode,
    )

    result: JanitorExecutionResult = execute_janitor_plan(
        plan=plan,
        adapter=adapter,
        connection=object(),
    )

    assert tuple(adapter.dropped_targets) == test_case.expected_dropped_targets
    assert tuple(candidate.key.name for candidate in result.deleted) == tuple(
        target.rsplit(".", maxsplit=1)[-1] for target in test_case.expected_dropped_targets
    )
    assert tuple(candidate.table_name for candidate in result.pruned_direct_state) == (
        test_case.expected_pruned_table_names
    )
