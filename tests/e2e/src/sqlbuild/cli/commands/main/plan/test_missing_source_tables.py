"""E2E coverage for planning against source tables missing from the warehouse."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import (
    MissingSourceTableE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.plan.helpers import (
    prepare_missing_source_tables_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    query_duckdb,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        MissingSourceTableE2ETestCase(
            description="one missing source among several fails plan and build",
            expected_output_fragments=(
                "error[S405]",
                "1 source table read by selected resources does not exist in the warehouse",
                "source 'raw_payments' (raw.payments), read by payments",
                "'raw_payments' at sql/sources/raw.yml:18",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_source_table_when_planning_then_fails_naming_it(
    test_case: MissingSourceTableE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_missing_source_tables_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    unaffected_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--select", "orders"), project_dir=project_dir
    )

    assert result.returncode == 1, result.stdout + result.stderr
    output: str = result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output
    assert build_result.returncode == 1, build_result.stdout + build_result.stderr
    assert test_case.expected_output_fragments[0] in build_result.stdout + build_result.stderr
    assert unaffected_result.returncode == 0, unaffected_result.stdout + unaffected_result.stderr
    assert not query_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="SELECT table_name FROM information_schema.tables WHERE table_schema = 'main'",
    )


@pytest.mark.parametrize(
    "test_case",
    [
        MissingSourceTableE2ETestCase(
            description="unread missing source leaves other freshness recorded and used",
            expected_output_fragments=(
                "Freshness query  FAIL (source freshness unknown)",
                "source freshness unknown (error) for raw_payments",
            ),
            expected_recorded_source_names=("raw_customers", "raw_orders"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_source_reference_removed_when_building_then_other_sources_keep_freshness(
    test_case: MissingSourceTableE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_missing_source_tables_project(tmp_path=tmp_path)
    (project_dir / "sql" / "models" / "payments.sql").unlink()

    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("plan", "--json"), project_dir=project_dir
    )
    text_plan_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )

    assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    recorded: list[tuple[Any, ...]] = query_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="SELECT DISTINCT source_name FROM main._sqlbuild_source_freshness ORDER BY 1",
    )
    assert tuple(row[0] for row in recorded) == test_case.expected_recorded_source_names
    assert plan_result.returncode == 0, plan_result.stdout + plan_result.stderr
    payload: dict[str, Any] = json.loads(plan_result.stdout)
    freshness: dict[str, Any] = payload["metadata"]["direct_source_freshness"]
    assert freshness["unchanged_source_names"] == list(test_case.expected_recorded_source_names)
    assert freshness["unknown_source_names"] == ["raw_payments"]
    assert [
        (detail["source_name"], detail["reason"]) for detail in freshness["unknown_source_details"]
    ] == [("raw_payments", "error")]
    assert [warning["message"] for warning in payload["warnings"]] == [
        "source freshness unknown (error) for raw_payments: the freshness observation failed; "
        "run sqb freshness for each source's details"
    ]
    assert text_plan_result.returncode == 0, text_plan_result.stdout + text_plan_result.stderr
    text_output: str = text_plan_result.stdout + text_plan_result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in text_output, text_output


@pytest.mark.parametrize(
    "test_case",
    [
        MissingSourceTableE2ETestCase(
            description="creating the missing table lets the plan succeed",
            expected_output_fragments=("Plan ready  3 selected",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_source_table_when_created_then_plan_succeeds(
    test_case: MissingSourceTableE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_missing_source_tables_project(tmp_path=tmp_path)
    execute_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="CREATE TABLE raw.payments AS SELECT 1 AS payment_id, "
        "TIMESTAMP '2026-01-03 00:00:00' AS updated_at",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    output: str = result.stdout + result.stderr
    assert "error[S405]" not in output
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        MissingSourceTableE2ETestCase(
            description="source declared with different identifier case plans",
            expected_output_fragments=("Plan ready  3 selected",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_case_variant_source_declaration_when_planning_then_table_is_found(
    test_case: MissingSourceTableE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_missing_source_tables_project(tmp_path=tmp_path)
    execute_duckdb(
        db_path=project_dir / "warehouse.duckdb",
        sql="CREATE TABLE raw.payments AS SELECT 1 AS payment_id, "
        "TIMESTAMP '2026-01-03 00:00:00' AS updated_at",
    )
    sources_path: Path = project_dir / "sql" / "sources" / "raw.yml"
    sources_path.write_text(
        sources_path.read_text(encoding="utf-8").replace(
            "schema: raw\n    table: orders", "schema: Raw\n    table: Orders"
        ),
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    output: str = result.stdout + result.stderr
    assert "error[S405]" not in output
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        MissingSourceTableE2ETestCase(
            description="freshness command reports each source on its own",
            expected_output_fragments=(
                "raw_customers  value 2026-01-02T00:00:00+00:00",
                "raw_orders     value 2026-01-01T00:00:00+00:00",
                "raw_payments",
                "OBSERVED=2",
                "ERROR=1",
            ),
            expected_statuses={
                "raw_customers": "observed",
                "raw_orders": "observed",
                "raw_payments": "error",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_source_table_when_observing_freshness_then_reports_each_source(
    test_case: MissingSourceTableE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_missing_source_tables_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "freshness"), project_dir=project_dir
    )
    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("freshness", "--json"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_output_fragments:
        assert fragment in result.stdout, result.stdout
    payload: dict[str, Any] = json.loads(json_result.stdout)
    assert {
        source["name"]: source["status"] for source in payload["sources"]
    } == test_case.expected_statuses


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
