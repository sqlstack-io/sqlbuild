from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.freshness._test_types import (
    FreshnessE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.freshness.helpers import (
    freshness_sources_yml,
    persist_direct_source_freshness,
    prepare_freshness_project,
    prepare_multi_schema_freshness_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    query_duckdb,
    run_sqb,
    table_exists,
)


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="observes configured source freshness without writing state",
            expected_fragments=(
                "Source freshness",
                "Observed (1)",
                "raw_orders  value 1  kind integer  via sql",
                "OBSERVED=1  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_configured_source_freshness_when_running_then_observes_read_only(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "raw_orders"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout

    assert not table_exists(
        db_path=project_dir / "warehouse.duckdb",
        table_name="_sqlbuild_source_freshness",
    )

    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("freshness", "--select", "raw_orders", "--json"),
        project_dir=project_dir,
    )
    payload: dict[str, Any] = json.loads(json_result.stdout)
    assert json_result.returncode == 0, json_result.stdout + json_result.stderr
    assert payload["summary"] == {
        "observed": 1,
        "changed": 0,
        "unchanged": 0,
        "tolerated": 0,
        "unknown": 0,
        "errors": 0,
        "age_pass": 0,
        "age_warn": 0,
        "age_error": 0,
        "age_unknown": 0,
    }
    assert payload["sources"][0]["name"] == "raw_orders"
    assert payload["sources"][0]["status"] == "observed"

    json_output_path: Path = project_dir / "target" / "freshness.json"
    json_file_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "freshness",
            "--select",
            "raw_orders",
            "--json-output",
            str(json_output_path),
        ),
        project_dir=project_dir,
    )
    file_payload: dict[str, Any] = json.loads(json_output_path.read_text(encoding="utf-8"))
    assert json_file_result.returncode == 0, json_file_result.stdout + json_file_result.stderr
    assert "Source freshness" in json_file_result.stdout
    assert file_payload["summary"] == {
        "observed": 1,
        "changed": 0,
        "unchanged": 0,
        "tolerated": 0,
        "unknown": 0,
        "errors": 0,
        "age_pass": 0,
        "age_warn": 0,
        "age_error": 0,
        "age_unknown": 0,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="fail on error returns nonzero for unknown source freshness",
            expected_fragments=(
                "Unknown (1)",
                "raw_unknown  no freshness config and adapter metadata unavailable",
                "OBSERVED=0  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=1  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_source_freshness_when_fail_on_error_then_returns_nonzero(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path, include_error_source=False)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "raw_unknown", "--fail-on-error"),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="explicit freshness error returns nonzero with fail on error",
            expected_fragments=(
                "Errors (1)",
                "raw_error  source 'raw_error' freshness query failed: "
                'Binder Error: Referenced column "missing_column"',
                "OBSERVED=0  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=1",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_error_when_fail_on_error_then_returns_nonzero(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "raw_error", "--fail-on-error"),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="model selector observes upstream source freshness",
            expected_fragments=(
                "Observed (1)",
                "raw_orders  value 1  kind integer  via sql",
                "OBSERVED=1  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_selector_when_running_freshness_then_observes_upstream_sources(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path, include_error_source=False)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "orders"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="excluded model removes upstream source freshness",
            expected_fragments=(
                "Source freshness",
                "OBSERVED=0  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_excluded_when_running_freshness_then_skips_upstream_sources(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path, include_error_source=False)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "orders", "--exclude", "orders"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout
    assert "raw_orders" not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="managed source with explicit freshness is observed read only",
            expected_fragments=(
                "Observed (1)",
                "raw_managed  value 5  kind integer  via sql",
                "OBSERVED=1  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_managed_source_with_freshness_when_running_then_observes_read_only(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(
        tmp_path=tmp_path,
        include_error_source=False,
        include_managed_source=True,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "raw_managed"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout
    assert not table_exists(
        db_path=project_dir / "warehouse.duckdb",
        table_name="_sqlbuild_source_freshness",
    )


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="existing source freshness state table is not modified",
            expected_fragments=(
                "Observed (1)",
                "OBSERVED=1  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_existing_source_freshness_state_when_running_then_does_not_write_state(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path, include_error_source=False)
    db_path: Path = project_dir / "warehouse.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE _sqlbuild_source_freshness ("
            "source_name VARCHAR, target_database VARCHAR, target_schema VARCHAR, "
            "target_name VARCHAR, run_id VARCHAR, strategy VARCHAR, value_kind VARCHAR, "
            "data_version VARCHAR, data_version_hash VARCHAR, observed_at TIMESTAMP)"
        ),
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            "INSERT INTO _sqlbuild_source_freshness VALUES "
            "('raw_orders', NULL, NULL, NULL, 'existing', 'sql', 'integer', "
            "'old', 'old-hash', TIMESTAMP '2026-01-01 00:00:00')"
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "raw_orders"),
        project_dir=project_dir,
    )

    rows: list[tuple[Any, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT run_id, data_version FROM _sqlbuild_source_freshness",
    )
    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout

    assert rows == [("existing", "old")]


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description=(
                "direct state comparison fails on timestamp movement beyond lag tolerance"
            ),
            expected_fragments=(
                "Changed (1)",
                "raw_orders  previous 2026-01-01T00:00:00+00:00  current 2026-01-01T00:11:00+00:00",
                "tolerance 10m",
                "OBSERVED=0  CHANGED=1  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_timestamp_freshness_beyond_tolerance_when_state_then_returns_nonzero(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(
        tmp_path=tmp_path,
        include_error_source=False,
        raw_orders_freshness=(
            "    freshness:\n"
            "      strategy: sql\n"
            "      type: timestamp\n"
            "      query: SELECT TIMESTAMP '2026-01-01 00:00:00' AS data_version\n"
            "      lag_tolerance: 10m\n"
        ),
    )
    persist_direct_source_freshness(project_dir=project_dir)
    (project_dir / "sql" / "sources" / "raw.yml").write_text(
        freshness_sources_yml(
            raw_orders_freshness=(
                "    freshness:\n"
                "      strategy: sql\n"
                "      type: timestamp\n"
                "      query: SELECT TIMESTAMP '2026-01-01 00:11:00' AS data_version\n"
                "      lag_tolerance: 10m\n"
            ),
            include_error_source=False,
        ),
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "freshness",
            "--select",
            "raw_orders",
            "--state",
            "--fail-on-stale",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="direct state comparison fails on backwards timestamp movement",
            expected_fragments=(
                "Changed (1)",
                "raw_orders  previous 2026-01-01T00:10:00+00:00  current 2026-01-01T00:05:00+00:00",
                "tolerance 10m",
                "OBSERVED=0  CHANGED=1  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_timestamp_freshness_moves_backwards_when_state_then_returns_nonzero(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(
        tmp_path=tmp_path,
        include_error_source=False,
        raw_orders_freshness=(
            "    freshness:\n"
            "      strategy: sql\n"
            "      type: timestamp\n"
            "      query: SELECT TIMESTAMP '2026-01-01 00:10:00' AS data_version\n"
            "      lag_tolerance: 10m\n"
        ),
    )
    persist_direct_source_freshness(project_dir=project_dir)
    (project_dir / "sql" / "sources" / "raw.yml").write_text(
        freshness_sources_yml(
            raw_orders_freshness=(
                "    freshness:\n"
                "      strategy: sql\n"
                "      type: timestamp\n"
                "      query: SELECT TIMESTAMP '2026-01-01 00:05:00' AS data_version\n"
                "      lag_tolerance: 10m\n"
            ),
            include_error_source=False,
        ),
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "freshness",
            "--select",
            "raw_orders",
            "--state",
            "--fail-on-stale",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="direct state comparison reads state from multiple target schemas",
            expected_fragments=(
                "Unchanged (1)",
                "raw_orders  previous 1  current 1",
                "OBSERVED=0  CHANGED=0  UNCHANGED=1  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_freshness_state_in_secondary_schema_when_running_state_then_reports_unchanged(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_multi_schema_freshness_project(tmp_path=tmp_path)
    persist_direct_source_freshness(project_dir=project_dir)
    execute_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="DELETE FROM dev._sqlbuild_source_freshness",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "raw_orders", "--state"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="direct state comparison reports unchanged source freshness",
            expected_fragments=(
                "Unchanged (1)",
                "raw_orders  previous 1  current 1",
                "OBSERVED=0  CHANGED=0  UNCHANGED=1  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_persisted_source_freshness_when_running_state_then_reports_unchanged(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path, include_error_source=False)
    persist_direct_source_freshness(project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness", "--select", "raw_orders", "--state"),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout

    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("freshness", "--select", "raw_orders", "--state", "--json"),
        project_dir=project_dir,
    )
    payload: dict[str, Any] = json.loads(json_result.stdout)
    assert json_result.returncode == 0, json_result.stdout + json_result.stderr
    assert payload["summary"]["unchanged"] == 1
    assert payload["sources"][0]["status"] == "unchanged"
    assert payload["sources"][0]["previous_data_version"] == "1"
    assert payload["sources"][0]["current_data_version"] == "1"


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="direct state comparison fails on changed source freshness",
            expected_fragments=(
                "Changed (1)",
                "raw_orders  previous 1  current 2",
                "OBSERVED=0  CHANGED=1  UNCHANGED=0  TOLERATED=0  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_changed_source_freshness_when_running_state_fail_on_stale_then_returns_nonzero(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path, include_error_source=False)
    persist_direct_source_freshness(project_dir=project_dir)
    (project_dir / "sql" / "sources" / "raw.yml").write_text(
        freshness_sources_yml(
            raw_orders_query="SELECT 2 AS data_version",
            include_error_source=False,
        ),
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "freshness",
            "--select",
            "raw_orders",
            "--state",
            "--fail-on-stale",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout

    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "freshness",
            "--select",
            "raw_orders",
            "--state",
            "--fail-on-stale",
            "--json",
        ),
        project_dir=project_dir,
    )
    payload: dict[str, Any] = json.loads(json_result.stdout)
    assert json_result.returncode == 1, json_result.stdout + json_result.stderr
    assert payload["summary"]["changed"] == 1
    assert payload["sources"][0]["status"] == "changed"
    assert payload["sources"][0]["previous_data_version"] == "1"
    assert payload["sources"][0]["current_data_version"] == "2"


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description=(
                "direct state comparison tolerates timestamp movement within lag tolerance"
            ),
            expected_fragments=(
                "Tolerated (1)",
                "raw_orders  previous 2026-01-01T00:00:00+00:00  current 2026-01-01T00:05:00+00:00",
                "tolerance 10m",
                "OBSERVED=0  CHANGED=0  UNCHANGED=0  TOLERATED=1  UNKNOWN=0  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_timestamp_freshness_within_tolerance_when_running_state_then_reports_tolerated(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(
        tmp_path=tmp_path,
        include_error_source=False,
        raw_orders_freshness=(
            "    freshness:\n"
            "      strategy: sql\n"
            "      type: timestamp\n"
            "      query: SELECT TIMESTAMP '2026-01-01 00:00:00' AS data_version\n"
            "      lag_tolerance: 10m\n"
        ),
    )
    persist_direct_source_freshness(project_dir=project_dir)
    (project_dir / "sql" / "sources" / "raw.yml").write_text(
        freshness_sources_yml(
            raw_orders_freshness=(
                "    freshness:\n"
                "      strategy: sql\n"
                "      type: timestamp\n"
                "      query: SELECT TIMESTAMP '2026-01-01 00:05:00' AS data_version\n"
                "      lag_tolerance: 10m\n"
            ),
            include_error_source=False,
        ),
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "freshness",
            "--select",
            "raw_orders",
            "--state",
            "--fail-on-stale",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout

    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "freshness",
            "--select",
            "raw_orders",
            "--state",
            "--fail-on-stale",
            "--json",
        ),
        project_dir=project_dir,
    )
    payload: dict[str, Any] = json.loads(json_result.stdout)
    assert json_result.returncode == 0, json_result.stdout + json_result.stderr
    assert payload["summary"]["tolerated"] == 1
    assert payload["sources"][0]["status"] == "tolerated"
    assert payload["sources"][0]["previous_data_version"] == "2026-01-01T00:00:00+00:00"
    assert payload["sources"][0]["current_data_version"] == "2026-01-01T00:05:00+00:00"


@pytest.mark.parametrize(
    "test_case",
    [
        FreshnessE2ETestCase(
            description="direct state comparison fails when previous state is missing",
            expected_fragments=(
                "Unknown (1)",
                "raw_orders  previous source freshness state missing",
                "OBSERVED=0  CHANGED=0  UNCHANGED=0  TOLERATED=0  UNKNOWN=1  ERROR=0",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_previous_freshness_when_running_state_fail_on_stale_then_returns_nonzero(
    test_case: FreshnessE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_freshness_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "freshness",
            "--select",
            "raw_orders",
            "--state",
            "--fail-on-stale",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout
