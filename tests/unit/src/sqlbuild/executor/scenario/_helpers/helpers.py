from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import cast

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    ScenarioArtifactKind,
    ScenarioAssertionExpectationPlan,
    ScenarioExecutionPlan,
    ScenarioExpectedExpectationPlan,
    ScenarioFixturePlan,
    ScenarioGraphPlan,
    ScenarioRelationMap,
    ScenarioRelationPlan,
    SeedPlanEntry,
)
from sqlbuild.executor.scenario.models import (
    ScenarioSnapshotCaptureRunResult,
    ScenarioSnapshotColumn,
    ScenarioSnapshotManifest,
    ScenarioSnapshotRelation,
    ScenarioSnapshotStateResult,
)
from sqlbuild.observability import LifecycleEvent
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS
from tests.unit.src.sqlbuild.executor.scenario._helpers._test_types import (
    ExecuteScenarioSnapshotCaptureStepsTestCase,
    ScenarioSnapshotStateTestCase,
)


def resource_attempt_events(events: list[LifecycleEvent]) -> tuple[LifecycleEvent, ...]:
    return tuple(filter(lambda event: event.event_type.startswith("resource_attempt_"), events))


def build_snapshot_input_specs_test_plan(
    *, fixture_sql_suffix: str = "", expected_sql_suffix: str = ""
) -> ScenarioExecutionPlan:
    scenario_name: str = "revenue__customer_refund"
    scenario_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SQL_SCENARIO,
        name=scenario_name,
    )
    model_target: CompiledRelationLocation = _target("model", "daily_revenue")

    return ScenarioExecutionPlan(
        key=scenario_key,
        name=scenario_name,
        graph_plan=ScenarioGraphPlan(
            key=scenario_key,
            name=scenario_name,
            target_model_names=("daily_revenue",),
            model_names=("daily_revenue",),
            source_fixture_names=("raw__orders",),
            ref_fixture_names=("stg_customers",),
            seed_names=("country_codes", "currency_codes"),
            seed_fixture_names=("country_codes",),
        ),
        relation_plan=ScenarioRelationPlan(
            scenario_name=scenario_name,
            relation_map=ScenarioRelationMap(
                scenario_name=scenario_name,
                hash_prefix="51b385aebe20",
            ),
        ),
        fixture_plans=(
            ScenarioFixturePlan(
                kind=ScenarioArtifactKind.SOURCE,
                logical_name="raw__orders",
                destination=_target("source", "raw__orders"),
                sql=f"SELECT 1 AS order_id{fixture_sql_suffix}",
            ),
            ScenarioFixturePlan(
                kind=ScenarioArtifactKind.REF,
                logical_name="stg_customers",
                destination=_target("ref", "stg_customers"),
                sql="SELECT 10 AS customer_id",
            ),
            ScenarioFixturePlan(
                kind=ScenarioArtifactKind.SEED,
                logical_name="country_codes",
                destination=_target("seed", "country_codes"),
                sql="SELECT 'US' AS country_code",
            ),
        ),
        seed_entries=(
            SeedPlanEntry(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.SEED,
                    name="currency_codes",
                ),
                name="currency_codes",
                destination=_target("seed", "currency_codes"),
                file_path=Path("sql/seeds/currency_codes.csv"),
                columns=(),
                csv_settings=DEFAULT_SEED_CSV_SETTINGS,
            ),
        ),
        model_entries=(
            ModelPlanEntry(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.MODEL,
                    name="daily_revenue",
                ),
                name="daily_revenue",
                relative_path=Path("sql/models/daily_revenue.sql"),
                materialization_type=MaterializationType.TABLE,
                action=PlanAction.CREATE_TABLE,
                reason=PlanReason.FIRST_RUN,
                destination=model_target,
                fingerprint_query_sql="SELECT 75 AS revenue",
                resolved_sql="SELECT 75 AS revenue",
                logical_ddl="CREATE TABLE daily_revenue AS SELECT 75 AS revenue",
            ),
        ),
        expected_expectations=(
            ScenarioExpectedExpectationPlan(
                model_name="daily_revenue",
                actual_destination=model_target,
                expected_sql=f"SELECT 75 AS revenue{expected_sql_suffix}",
            ),
        ),
        assertion_expectations=(
            ScenarioAssertionExpectationPlan(
                name="no_negative_revenue",
                sql=f"SELECT * FROM daily_revenue WHERE revenue < 0{expected_sql_suffix}",
            ),
        ),
    )


def build_snapshot_manifest(*, input_fingerprint: str = "fresh123") -> ScenarioSnapshotManifest:
    return ScenarioSnapshotManifest(
        version=1,
        scenario_name="revenue__customer_refund",
        captured_at="2026-05-09T00:00:00Z",
        capture_adapter="snowflake",
        capture_dialect="snowflake",
        sqlbuild_version="0.1.0",
        input_fingerprint=input_fingerprint,
        total_rows=2,
        total_bytes=100,
        relations=(
            ScenarioSnapshotRelation(
                kind=ScenarioArtifactKind.SOURCE,
                logical_name="raw__orders",
                file_path=Path("sql/sources/raw__orders.jsonl"),
                row_count=2,
                byte_count=100,
                columns=(
                    ScenarioSnapshotColumn(
                        name="order_id",
                        warehouse_type="NUMBER",
                        local_type="BIGINT",
                    ),
                    ScenarioSnapshotColumn(
                        name="amount",
                        warehouse_type="NUMBER(10,2)",
                        local_type="DECIMAL(10,2)",
                    ),
                ),
            ),
        ),
    )


def build_local_snapshot_load_manifest(
    *,
    input_fingerprint: str,
    columns: tuple[tuple[str, str, str], ...],
) -> ScenarioSnapshotManifest:
    return ScenarioSnapshotManifest(
        version=1,
        scenario_name="revenue__customer_refund",
        captured_at="2026-05-09T00:00:00Z",
        capture_adapter="snowflake",
        capture_dialect="snowflake",
        sqlbuild_version="0.1.0",
        input_fingerprint=input_fingerprint,
        total_rows=1,
        total_bytes=1,
        relations=(
            ScenarioSnapshotRelation(
                kind=ScenarioArtifactKind.SOURCE,
                logical_name="raw__orders",
                file_path=Path("sql/sources/raw__orders.jsonl"),
                row_count=1,
                byte_count=1,
                columns=tuple(
                    ScenarioSnapshotColumn(
                        name=name,
                        warehouse_type=warehouse_type,
                        local_type=local_type,
                    )
                    for name, warehouse_type, local_type in columns
                ),
            ),
        ),
    )


def write_local_snapshot_load_test_files(
    *,
    project_dir: Path,
    manifest: ScenarioSnapshotManifest,
    rows: tuple[dict[str, object], ...],
) -> None:
    from sqlbuild.executor.scenario._helpers.snapshots.core import (
        scenario_snapshot_root,
        write_scenario_snapshot_jsonl,
        write_scenario_snapshot_manifest,
    )

    snapshot_root: Path = scenario_snapshot_root(
        project_dir=project_dir,
        scenario_name=manifest.scenario_name,
    )
    write_scenario_snapshot_manifest(
        manifest_path=snapshot_root / "scenario.json",
        manifest=manifest,
    )
    write_scenario_snapshot_jsonl(
        file_path=snapshot_root / "sql" / "sources" / "raw__orders.jsonl",
        rows=rows,
    )


def write_local_snapshot_load_test_file_contents(
    *, project_dir: Path, manifest: ScenarioSnapshotManifest, file_contents: str
) -> None:
    from sqlbuild.executor.scenario._helpers.snapshots.core import (
        scenario_snapshot_root,
        write_scenario_snapshot_manifest,
    )

    snapshot_root: Path = scenario_snapshot_root(
        project_dir=project_dir,
        scenario_name=manifest.scenario_name,
    )
    write_scenario_snapshot_manifest(
        manifest_path=snapshot_root / "scenario.json",
        manifest=manifest,
    )
    file_path: Path = snapshot_root / "sql" / "sources" / "raw__orders.jsonl"
    file_path.parent.mkdir(parents=True, exist_ok=True)
    file_path.write_text(file_contents, encoding="utf-8")


def write_snapshot_state_test_manifest(
    *, manifest_path: Path, test_case: ScenarioSnapshotStateTestCase
) -> None:
    _SNAPSHOT_MANIFEST_WRITERS[test_case.manifest is None](
        manifest_path=manifest_path,
        value=test_case.manifest,
    )
    _SNAPSHOT_CONTENT_WRITERS[test_case.manifest_contents is None](
        manifest_path=manifest_path,
        value=test_case.manifest_contents,
    )


def assert_snapshot_state_error(
    *, state_result: ScenarioSnapshotStateResult, test_case: ScenarioSnapshotStateTestCase
) -> None:
    normalized_error_code: str | None = {None: None}.get(
        test_case.expected_error_code, state_result.error_code
    )
    assert normalized_error_code == test_case.expected_error_code
    assert (test_case.expected_error_fragment or "") in (state_result.error_message or "")


def assert_capture_steps_error(
    *,
    result: ScenarioSnapshotCaptureRunResult,
    test_case: ExecuteScenarioSnapshotCaptureStepsTestCase,
) -> None:
    assert (test_case.expected_error_fragment or "") in (result.error_message or "")


def _target(kind: str, logical_name: str) -> CompiledRelationLocation:
    name: str = f"__sqb_51b385aebe20__{kind}__{logical_name}"
    return CompiledRelationLocation(
        database=None,
        schema="scenario_schema",
        name=name,
        qualified_name=f"scenario_schema.{name}",
    )


def _ignore_optional_snapshot_value(*, manifest_path: Path, value: object) -> None:
    del manifest_path, value


def _write_snapshot_manifest(*, manifest_path: Path, value: object) -> None:
    from sqlbuild.executor.scenario._helpers.snapshots.core import write_scenario_snapshot_manifest

    write_scenario_snapshot_manifest(
        manifest_path=manifest_path,
        manifest=cast(ScenarioSnapshotManifest, value),
    )


def _write_snapshot_contents(*, manifest_path: Path, value: object) -> None:
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(cast(str, value), encoding="utf-8")


_SNAPSHOT_MANIFEST_WRITERS: dict[bool, Callable[..., None]] = {
    True: _ignore_optional_snapshot_value,
    False: _write_snapshot_manifest,
}
_SNAPSHOT_CONTENT_WRITERS: dict[bool, Callable[..., None]] = {
    True: _ignore_optional_snapshot_value,
    False: _write_snapshot_contents,
}
