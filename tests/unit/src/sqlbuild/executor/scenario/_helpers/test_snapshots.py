from __future__ import annotations

from datetime import date, datetime, time, timedelta
from decimal import Decimal
from pathlib import Path

import pytest

from sqlbuild._native import ScenarioArtifactKind, ScenarioExecutionPlan
from sqlbuild.executor.scenario._helpers.snapshots.core import (
    build_scenario_snapshot_capture_plan,
    build_scenario_snapshot_input_fingerprint,
    build_scenario_snapshot_input_specs,
    build_scenario_snapshot_manifest_shell,
    classify_scenario_snapshot_state,
    is_scenario_snapshot_fresh,
    read_scenario_snapshot_jsonl,
    read_scenario_snapshot_manifest,
    scenario_snapshot_manifest_path,
    scenario_snapshot_relation_file_path,
    scenario_snapshot_root,
    write_scenario_snapshot_jsonl,
    write_scenario_snapshot_manifest,
)
from sqlbuild.executor.scenario.constants import (
    SCENARIO_LOCAL_MANIFEST_INVALID,
    SCENARIO_LOCAL_SNAPSHOT_MISSING,
    SCENARIO_LOCAL_SNAPSHOT_STALE,
)
from sqlbuild.executor.scenario.models import (
    ScenarioSnapshotCapturePlan,
    ScenarioSnapshotCaptureRelationPlan,
    ScenarioSnapshotFileStats,
    ScenarioSnapshotInputSpec,
    ScenarioSnapshotManifest,
    ScenarioSnapshotRelation,
    ScenarioSnapshotStateResult,
)
from sqlbuild.executor.scenario.types import ScenarioSnapshotState
from tests.unit.src.sqlbuild.executor.scenario._helpers._test_types import (
    ScenarioSnapshotCapturePlanTestCase,
    ScenarioSnapshotDialectFingerprintTestCase,
    ScenarioSnapshotFingerprintTestCase,
    ScenarioSnapshotFreshnessTestCase,
    ScenarioSnapshotInputSpecsFromPlanTestCase,
    ScenarioSnapshotJsonlErrorTestCase,
    ScenarioSnapshotJsonlRoundTripTestCase,
    ScenarioSnapshotManifestIoTestCase,
    ScenarioSnapshotPathTestCase,
    ScenarioSnapshotRelationPathErrorTestCase,
    ScenarioSnapshotStateTestCase,
)
from tests.unit.src.sqlbuild.executor.scenario._helpers.helpers import (
    assert_snapshot_state_error,
    build_snapshot_input_specs_test_plan,
    build_snapshot_manifest,
    write_snapshot_state_test_manifest,
)

SNAPSHOT_STATE_PLAN: ScenarioExecutionPlan = build_snapshot_input_specs_test_plan()
SNAPSHOT_STATE_FINGERPRINT: str = build_scenario_snapshot_input_fingerprint(
    scenario_name=SNAPSHOT_STATE_PLAN.name,
    input_specs=build_scenario_snapshot_input_specs(scenario_plan=SNAPSHOT_STATE_PLAN),
)
SNAPSHOT_CAPTURE_PLAN: ScenarioExecutionPlan = build_snapshot_input_specs_test_plan()
SNAPSHOT_CAPTURE_FINGERPRINT: str = build_scenario_snapshot_input_fingerprint(
    scenario_name=SNAPSHOT_CAPTURE_PLAN.name,
    input_specs=build_scenario_snapshot_input_specs(scenario_plan=SNAPSHOT_CAPTURE_PLAN),
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotCapturePlanTestCase(
            description="scenario execution plan becomes snapshot capture plan",
            project_dir=Path("/repo"),
            scenario_plan=SNAPSHOT_CAPTURE_PLAN,
            expected_capture_plan=ScenarioSnapshotCapturePlan(
                scenario_name="revenue__customer_refund",
                snapshot_root=Path("/repo/tests/_scenario_snapshots/revenue__customer_refund"),
                manifest_path=Path(
                    "/repo/tests/_scenario_snapshots/revenue__customer_refund/scenario.json"
                ),
                input_fingerprint=SNAPSHOT_CAPTURE_FINGERPRINT,
                relations=(
                    ScenarioSnapshotCaptureRelationPlan(
                        kind=ScenarioArtifactKind.REF,
                        logical_name="stg_customers",
                        source_target=SNAPSHOT_CAPTURE_PLAN.fixture_plans[1].destination,
                        file_path=Path("refs/stg_customers.jsonl"),
                        capture_sql="SELECT 10 AS customer_id",
                    ),
                    ScenarioSnapshotCaptureRelationPlan(
                        kind=ScenarioArtifactKind.SEED,
                        logical_name="country_codes",
                        source_target=SNAPSHOT_CAPTURE_PLAN.fixture_plans[2].destination,
                        file_path=Path("seeds/country_codes.jsonl"),
                        capture_sql="SELECT 'US' AS country_code",
                    ),
                    ScenarioSnapshotCaptureRelationPlan(
                        kind=ScenarioArtifactKind.SEED,
                        logical_name="currency_codes",
                        source_target=SNAPSHOT_CAPTURE_PLAN.seed_entries[0].destination,
                        file_path=Path("seeds/currency_codes.jsonl"),
                        capture_sql="seed_file:sql/seeds/currency_codes.csv",
                    ),
                    ScenarioSnapshotCaptureRelationPlan(
                        kind=ScenarioArtifactKind.SOURCE,
                        logical_name="raw__orders",
                        source_target=SNAPSHOT_CAPTURE_PLAN.fixture_plans[0].destination,
                        file_path=Path("sources/raw__orders.jsonl"),
                        capture_sql="SELECT 1 AS order_id",
                    ),
                ),
            ),
            expected_manifest=ScenarioSnapshotManifest(
                version=1,
                scenario_name="revenue__customer_refund",
                captured_at="2026-05-09T00:00:00Z",
                capture_adapter="snowflake",
                capture_dialect="snowflake",
                sqlbuild_version="0.1.0",
                input_fingerprint=SNAPSHOT_CAPTURE_FINGERPRINT,
                total_rows=0,
                total_bytes=0,
                relations=(
                    ScenarioSnapshotRelation(
                        kind=ScenarioArtifactKind.REF,
                        logical_name="stg_customers",
                        file_path=Path("refs/stg_customers.jsonl"),
                        row_count=0,
                        byte_count=0,
                    ),
                    ScenarioSnapshotRelation(
                        kind=ScenarioArtifactKind.SEED,
                        logical_name="country_codes",
                        file_path=Path("seeds/country_codes.jsonl"),
                        row_count=0,
                        byte_count=0,
                    ),
                    ScenarioSnapshotRelation(
                        kind=ScenarioArtifactKind.SEED,
                        logical_name="currency_codes",
                        file_path=Path("seeds/currency_codes.jsonl"),
                        row_count=0,
                        byte_count=0,
                    ),
                    ScenarioSnapshotRelation(
                        kind=ScenarioArtifactKind.SOURCE,
                        logical_name="raw__orders",
                        file_path=Path("sources/raw__orders.jsonl"),
                        row_count=0,
                        byte_count=0,
                    ),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_execution_plan_when_building_capture_plan_then_returns_snapshot_work(
    test_case: ScenarioSnapshotCapturePlanTestCase,
) -> None:
    capture_plan: ScenarioSnapshotCapturePlan = build_scenario_snapshot_capture_plan(
        project_dir=test_case.project_dir,
        scenario_plan=test_case.scenario_plan,
    )
    manifest: ScenarioSnapshotManifest = build_scenario_snapshot_manifest_shell(
        capture_plan=capture_plan,
        captured_at="2026-05-09T00:00:00Z",
        capture_adapter="snowflake",
        capture_dialect="snowflake",
        sqlbuild_version="0.1.0",
    )

    assert capture_plan == test_case.expected_capture_plan
    assert manifest == test_case.expected_manifest


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotJsonlRoundTripTestCase(
            description="jsonl rows round trip with file stats",
            relative_file_path=Path("sources/raw__orders.jsonl"),
            rows=(
                {"order_id": 1, "amount": 10.5, "is_paid": True, "note": None},
                {"order_id": 2, "amount": 20, "is_paid": False, "note": "refund"},
            ),
            expected_stats=ScenarioSnapshotFileStats(row_count=2, byte_count=115),
            expected_file_contents=(
                '{"amount":10.5,"is_paid":true,"note":null,"order_id":1}\n'
                '{"amount":20,"is_paid":false,"note":"refund","order_id":2}\n'
            ),
            expected_rows=(
                {"amount": 10.5, "is_paid": True, "note": None, "order_id": 1},
                {"amount": 20, "is_paid": False, "note": "refund", "order_id": 2},
            ),
        ),
        ScenarioSnapshotJsonlRoundTripTestCase(
            description="jsonl rows serialize captured warehouse scalar objects",
            relative_file_path=Path("sources/raw__orders.jsonl"),
            rows=(
                {
                    "amount": Decimal("10.50"),
                    "order_date": date(2026, 1, 2),
                    "loaded_at": datetime(2026, 1, 2, 3, 4, 5),
                    "loaded_time": time(3, 4, 5),
                    "interval": timedelta(days=2),
                    "payload": b"abc",
                },
            ),
            expected_stats=ScenarioSnapshotFileStats(row_count=1, byte_count=151),
            expected_file_contents=(
                '{"amount":"10.50","interval":"172800 seconds",'
                '"loaded_at":"2026-01-02T03:04:05","loaded_time":"03:04:05",'
                '"order_date":"2026-01-02","payload":"616263"}\n'
            ),
            expected_rows=(
                {
                    "amount": "10.50",
                    "interval": "172800 seconds",
                    "loaded_at": "2026-01-02T03:04:05",
                    "loaded_time": "03:04:05",
                    "order_date": "2026-01-02",
                    "payload": "616263",
                },
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_rows_when_writing_and_reading_jsonl_then_round_trips_with_stats(
    tmp_path: Path,
    test_case: ScenarioSnapshotJsonlRoundTripTestCase,
) -> None:
    file_path: Path = tmp_path / test_case.relative_file_path

    stats: ScenarioSnapshotFileStats = write_scenario_snapshot_jsonl(
        file_path=file_path,
        rows=test_case.rows,
    )
    file_contents: str = file_path.read_text(encoding="utf-8")
    rows: tuple[dict[str, object], ...] = read_scenario_snapshot_jsonl(file_path=file_path)

    assert stats == test_case.expected_stats
    assert file_contents == test_case.expected_file_contents
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotJsonlErrorTestCase(
            description="malformed json line fails clearly",
            file_contents='{"order_id":1}\n{not json}\n',
            expected_error_fragment="line 2",
        ),
        ScenarioSnapshotJsonlErrorTestCase(
            description="non object json line fails clearly",
            file_contents='{"order_id":1}\n[1, 2, 3]\n',
            expected_error_fragment="row must be a JSON object",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_snapshot_jsonl_when_reading_then_raises_clear_error(
    tmp_path: Path,
    test_case: ScenarioSnapshotJsonlErrorTestCase,
) -> None:
    file_path: Path = tmp_path / "sql" / "sources" / "raw__orders.jsonl"
    file_path.parent.mkdir(parents=True, exist_ok=True)
    file_path.write_text(test_case.file_contents, encoding="utf-8")

    with pytest.raises(ValueError) as exc_info:
        read_scenario_snapshot_jsonl(file_path=file_path)

    assert test_case.expected_error_fragment in str(exc_info.value)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotManifestIoTestCase(
            description="manifest round trips through scenario json shape",
            manifest=build_snapshot_manifest(),
            expected_json_fragments=(
                '"file": "sources/raw__orders.jsonl"',
                '"bytes": 100',
                '"warehouse_type": "NUMBER(10,2)"',
                '"local_type": "DECIMAL(10,2)"',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_manifest_when_writing_and_reading_then_round_trips_json_shape(
    tmp_path: Path,
    test_case: ScenarioSnapshotManifestIoTestCase,
) -> None:
    manifest_path: Path = scenario_snapshot_manifest_path(
        project_dir=tmp_path,
        scenario_name=test_case.manifest.scenario_name,
    )

    write_scenario_snapshot_manifest(
        manifest_path=manifest_path,
        manifest=test_case.manifest,
    )
    manifest_contents: str = manifest_path.read_text(encoding="utf-8")
    loaded_manifest: ScenarioSnapshotManifest = read_scenario_snapshot_manifest(
        manifest_path=manifest_path,
    )

    assert loaded_manifest == test_case.manifest
    for expected_fragment in test_case.expected_json_fragments:
        assert expected_fragment in manifest_contents


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotStateTestCase(
            description="missing manifest is classified as missing",
            manifest=None,
            manifest_contents=None,
            expected_state=ScenarioSnapshotState.MISSING,
            expected_has_manifest=False,
            expected_error_code=SCENARIO_LOCAL_SNAPSHOT_MISSING,
        ),
        ScenarioSnapshotStateTestCase(
            description="matching fingerprint manifest is classified as fresh",
            manifest=build_snapshot_manifest(input_fingerprint=SNAPSHOT_STATE_FINGERPRINT),
            manifest_contents=None,
            expected_state=ScenarioSnapshotState.FRESH,
            expected_has_manifest=True,
        ),
        ScenarioSnapshotStateTestCase(
            description="different fingerprint manifest is classified as stale",
            manifest=build_snapshot_manifest(input_fingerprint="stale123"),
            manifest_contents=None,
            expected_state=ScenarioSnapshotState.STALE,
            expected_has_manifest=True,
            expected_error_code=SCENARIO_LOCAL_SNAPSHOT_STALE,
        ),
        ScenarioSnapshotStateTestCase(
            description="malformed manifest is classified as invalid",
            manifest=None,
            manifest_contents="{not json",
            expected_state=ScenarioSnapshotState.INVALID,
            expected_has_manifest=False,
            expected_error_code=SCENARIO_LOCAL_MANIFEST_INVALID,
            expected_error_fragment="Invalid scenario snapshot manifest JSON",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_manifest_state_when_classifying_then_returns_expected_state(
    tmp_path: Path,
    test_case: ScenarioSnapshotStateTestCase,
) -> None:
    manifest_path: Path = scenario_snapshot_manifest_path(
        project_dir=tmp_path,
        scenario_name=SNAPSHOT_STATE_PLAN.name,
    )
    write_snapshot_state_test_manifest(manifest_path=manifest_path, test_case=test_case)

    state_result: ScenarioSnapshotStateResult = classify_scenario_snapshot_state(
        project_dir=tmp_path,
        scenario_plan=SNAPSHOT_STATE_PLAN,
    )

    assert state_result.state == test_case.expected_state
    assert (state_result.manifest is not None) is test_case.expected_has_manifest
    assert_snapshot_state_error(state_result=state_result, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotInputSpecsFromPlanTestCase(
            description="scenario plan inputs become durable snapshot input specs",
            scenario_plan=build_snapshot_input_specs_test_plan(),
            expected_input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.REF,
                    logical_name="stg_customers",
                    file_path=Path("refs/stg_customers.jsonl"),
                    capture_sql="SELECT 10 AS customer_id",
                ),
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SEED,
                    logical_name="country_codes",
                    file_path=Path("seeds/country_codes.jsonl"),
                    capture_sql="SELECT 'US' AS country_code",
                ),
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SEED,
                    logical_name="currency_codes",
                    file_path=Path("seeds/currency_codes.jsonl"),
                    capture_sql="seed_file:sql/seeds/currency_codes.csv",
                ),
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw__orders",
                    file_path=Path("sources/raw__orders.jsonl"),
                    capture_sql="SELECT 1 AS order_id",
                ),
            ),
            changed_check_plan=build_snapshot_input_specs_test_plan(
                expected_sql_suffix=" ORDER BY revenue"
            ),
            changed_fixture_plan=build_snapshot_input_specs_test_plan(
                fixture_sql_suffix=" WHERE order_id = 1"
            ),
            expected_check_fingerprint_matches=True,
            expected_fixture_fingerprint_differs=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_execution_plan_when_building_snapshot_specs_then_returns_input_specs(
    test_case: ScenarioSnapshotInputSpecsFromPlanTestCase,
) -> None:
    input_specs: tuple[ScenarioSnapshotInputSpec, ...] = build_scenario_snapshot_input_specs(
        scenario_plan=test_case.scenario_plan,
    )
    fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.scenario_plan.name,
        input_specs=input_specs,
    )
    changed_check_fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.changed_check_plan.name,
        input_specs=build_scenario_snapshot_input_specs(
            scenario_plan=test_case.changed_check_plan,
        ),
    )
    changed_fixture_fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.changed_fixture_plan.name,
        input_specs=build_scenario_snapshot_input_specs(
            scenario_plan=test_case.changed_fixture_plan,
        ),
    )

    assert input_specs == test_case.expected_input_specs
    assert (fingerprint == changed_check_fingerprint) is (
        test_case.expected_check_fingerprint_matches
    )
    assert (fingerprint != changed_fixture_fingerprint) is (
        test_case.expected_fixture_fingerprint_differs
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotPathTestCase(
            description="source snapshot paths use scenario snapshot root",
            project_dir=Path("/repo"),
            scenario_name="order_refund",
            kind=ScenarioArtifactKind.SOURCE,
            logical_name="raw__orders",
            expected_root=Path("/repo/tests/_scenario_snapshots/order_refund"),
            expected_manifest_path=Path(
                "/repo/tests/_scenario_snapshots/order_refund/scenario.json"
            ),
            expected_relation_path=Path("sources/raw__orders.jsonl"),
        ),
        ScenarioSnapshotPathTestCase(
            description="ref snapshot paths use refs directory",
            project_dir=Path("/repo"),
            scenario_name="order_refund",
            kind=ScenarioArtifactKind.REF,
            logical_name="stg_customers",
            expected_root=Path("/repo/tests/_scenario_snapshots/order_refund"),
            expected_manifest_path=Path(
                "/repo/tests/_scenario_snapshots/order_refund/scenario.json"
            ),
            expected_relation_path=Path("refs/stg_customers.jsonl"),
        ),
        ScenarioSnapshotPathTestCase(
            description="seed snapshot paths use seeds directory",
            project_dir=Path("/repo"),
            scenario_name="order_refund",
            kind=ScenarioArtifactKind.SEED,
            logical_name="waffle_types",
            expected_root=Path("/repo/tests/_scenario_snapshots/order_refund"),
            expected_manifest_path=Path(
                "/repo/tests/_scenario_snapshots/order_refund/scenario.json"
            ),
            expected_relation_path=Path("seeds/waffle_types.jsonl"),
        ),
        ScenarioSnapshotPathTestCase(
            description="dbt ref snapshot paths use dbt_refs directory",
            project_dir=Path("/repo"),
            scenario_name="order_refund",
            kind=ScenarioArtifactKind.DBT_REF,
            logical_name="analytics__fact_orders",
            expected_root=Path("/repo/tests/_scenario_snapshots/order_refund"),
            expected_manifest_path=Path(
                "/repo/tests/_scenario_snapshots/order_refund/scenario.json"
            ),
            expected_relation_path=Path("dbt_refs/analytics__fact_orders.jsonl"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_and_relation_when_resolving_snapshot_paths_then_returns_expected_paths(
    test_case: ScenarioSnapshotPathTestCase,
) -> None:
    root: Path = scenario_snapshot_root(
        project_dir=test_case.project_dir,
        scenario_name=test_case.scenario_name,
    )
    manifest_path: Path = scenario_snapshot_manifest_path(
        project_dir=test_case.project_dir,
        scenario_name=test_case.scenario_name,
    )
    relation_path: Path = scenario_snapshot_relation_file_path(
        kind=test_case.kind,
        logical_name=test_case.logical_name,
    )

    assert root == test_case.expected_root
    assert manifest_path == test_case.expected_manifest_path
    assert relation_path == test_case.expected_relation_path


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotFingerprintTestCase(
            description="fingerprint is stable across input ordering and SQL whitespace",
            scenario_name="order_refund",
            input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw__orders",
                    file_path=Path("sources/raw__orders.jsonl"),
                    capture_sql="SELECT *\nFROM raw.orders",
                ),
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.REF,
                    logical_name="stg_customers",
                    file_path=Path("refs/stg_customers.jsonl"),
                    capture_sql="SELECT * FROM analytics.stg_customers",
                ),
            ),
            equivalent_input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.REF,
                    logical_name="stg_customers",
                    file_path=Path("refs/stg_customers.jsonl"),
                    capture_sql="SELECT   *   FROM analytics.stg_customers",
                ),
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw__orders",
                    file_path=Path("sources/raw__orders.jsonl"),
                    capture_sql="SELECT * FROM raw.orders",
                ),
            ),
            changed_input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw__orders",
                    file_path=Path("sources/raw__orders.jsonl"),
                    capture_sql="SELECT id FROM raw.orders",
                ),
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.REF,
                    logical_name="stg_customers",
                    file_path=Path("refs/stg_customers.jsonl"),
                    capture_sql="SELECT * FROM analytics.stg_customers",
                ),
            ),
            expected_matches_equivalent=True,
            expected_differs_from_changed=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_input_specs_when_fingerprinting_then_is_stable_and_input_sensitive(
    test_case: ScenarioSnapshotFingerprintTestCase,
) -> None:
    fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.scenario_name,
        input_specs=test_case.input_specs,
    )
    equivalent_fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.scenario_name,
        input_specs=test_case.equivalent_input_specs,
    )
    changed_fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.scenario_name,
        input_specs=test_case.changed_input_specs,
    )

    assert (fingerprint == equivalent_fingerprint) is test_case.expected_matches_equivalent
    assert (fingerprint != changed_fingerprint) is test_case.expected_differs_from_changed


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotDialectFingerprintTestCase(
            description="fingerprint changes when capture dialect changes",
            scenario_name="order_refund",
            input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw__orders",
                    file_path=Path("sources/raw__orders.jsonl"),
                    capture_sql="SELECT * FROM raw.orders",
                ),
            ),
            capture_adapter="snowflake",
            capture_dialect="snowflake",
            changed_capture_adapter="snowflake",
            changed_capture_dialect="bigquery",
            expected_differs_from_changed=True,
        ),
        ScenarioSnapshotDialectFingerprintTestCase(
            description="fingerprint changes when capture adapter changes",
            scenario_name="order_refund",
            input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw__orders",
                    file_path=Path("sources/raw__orders.jsonl"),
                    capture_sql="SELECT * FROM raw.orders",
                ),
            ),
            capture_adapter="snowflake",
            capture_dialect="snowflake",
            changed_capture_adapter="custom_snowflake",
            changed_capture_dialect="snowflake",
            expected_differs_from_changed=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_capture_dialect_when_fingerprinting_then_dialect_changes_are_stale(
    test_case: ScenarioSnapshotDialectFingerprintTestCase,
) -> None:
    fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.scenario_name,
        input_specs=test_case.input_specs,
        capture_adapter=test_case.capture_adapter,
        capture_dialect=test_case.capture_dialect,
    )
    changed_fingerprint: str = build_scenario_snapshot_input_fingerprint(
        scenario_name=test_case.scenario_name,
        input_specs=test_case.input_specs,
        capture_adapter=test_case.changed_capture_adapter,
        capture_dialect=test_case.changed_capture_dialect,
    )

    assert (fingerprint != changed_fingerprint) is test_case.expected_differs_from_changed


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotFreshnessTestCase(
            description="matching input fingerprint is fresh",
            manifest_fingerprint="abc123",
            current_fingerprint="abc123",
            expected_is_fresh=True,
        ),
        ScenarioSnapshotFreshnessTestCase(
            description="different input fingerprint is stale",
            manifest_fingerprint="abc123",
            current_fingerprint="def456",
            expected_is_fresh=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_manifest_and_current_fingerprint_when_checking_freshness_then_returns_expected_state(
    test_case: ScenarioSnapshotFreshnessTestCase,
) -> None:
    manifest: ScenarioSnapshotManifest = ScenarioSnapshotManifest(
        version=1,
        scenario_name="order_refund",
        captured_at="2026-05-09T00:00:00Z",
        capture_adapter="snowflake",
        capture_dialect="snowflake",
        sqlbuild_version="0.1.0",
        input_fingerprint=test_case.manifest_fingerprint,
        total_rows=0,
        total_bytes=0,
    )

    is_fresh: bool = is_scenario_snapshot_fresh(
        manifest=manifest,
        current_input_fingerprint=test_case.current_fingerprint,
    )

    assert is_fresh == test_case.expected_is_fresh


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioSnapshotRelationPathErrorTestCase(
            description="model artifacts are not captured as local snapshot relations",
            kind=ScenarioArtifactKind.MODEL,
            logical_name="fact_orders",
            expected_error_type=ValueError,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unsupported_artifact_when_resolving_snapshot_path_then_raises_expected_error(
    test_case: ScenarioSnapshotRelationPathErrorTestCase,
) -> None:
    with pytest.raises(test_case.expected_error_type):
        scenario_snapshot_relation_file_path(
            kind=test_case.kind,
            logical_name=test_case.logical_name,
        )
