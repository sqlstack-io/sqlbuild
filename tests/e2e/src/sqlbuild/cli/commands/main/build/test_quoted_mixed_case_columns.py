"""E2E coverage that quoted mixed-case column names work in headers, fixtures, plans and builds."""

from __future__ import annotations

import subprocess
from datetime import datetime
from decimal import Decimal
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    QuotedConfiguredColumnsE2ETestCase,
    QuotedContractE2ETestCase,
    QuotedMixedCaseColumnsBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "partner_feed"\nadapter = "duckdb"\n\n[connection]\ndatabase = "partner_feed.duckdb"\n'
    ),
    "sources/raw.yml": (
        "sources:\n"
        "  - name: raw_listings\n    description: Partner listings in quoted camelCase.\n"
        "    schema: main\n    table: raw_listings\n    columns:\n"
        "      - name: '\"listingId\"'\n        type: VARCHAR\n"
        "      - name: '\"unitPriceMinor\"'\n        type: BIGINT\n"
    ),
    "models/stg_listings.sql": (
        "MODEL (description 'Listings renamed to snake case.', materialized table);\n\n"
        'SELECT s."listingId" AS listing_id, s."unitPriceMinor" AS unit_price_minor\n'
        'FROM __source("raw_listings") AS s\n'
    ),
    "models/listing_export.sql": (
        "MODEL (\n  description 'Listings in the partner feed format.',\n  materialized table,\n"
        "  columns (\n"
        '    "listingId" (type VARCHAR, nullable false, audits [not_null, unique]),\n'
        '    "unitPrice" (type DECIMAL(18, 2)),\n  ),\n);\n\n'
        "WITH final AS (\n"
        '  SELECT l.listing_id::VARCHAR AS "listingId", (l.unit_price_minor / 100)::DECIMAL(18, 2) AS "unitPrice"\n'
        '  FROM __ref("stg_listings") AS l\n)\n\nSELECT *\nFROM final\n'
    ),
    "tests/unit/test_stg_listings.sql": (
        'TEST (name "stg_listings__renames_quoted_columns");\n\n'
        'WITH __source__raw_listings AS (SELECT \'L1\' AS "listingId", 1299 AS "unitPriceMinor"),\n\n'
        "__expected__stg_listings AS (SELECT 'L1' AS listing_id, 1299 AS unit_price_minor)\n"
    ),
    "tests/scenarios/listings__export.sql": (
        "SCENARIO (description 'Listings reach the export in quoted camelCase.');\n"
        "WITH\n"
        '__source__raw_listings AS (SELECT \'L1\' AS "listingId", 1299 AS "unitPriceMinor"),\n'
        '__expected__listing_export AS (SELECT \'L1\' AS "listingId", 12.99::DECIMAL(18, 2) AS "unitPrice")\n'
        "SELECT 1\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedMixedCaseColumnsBuildE2ETestCase(
            description="quoted camelCase columns compile, build twice, test and replay",
            expected_export_rows=[("L1", Decimal("12.99"))],
            expected_test_fragment="PASS=1  FAIL=0",
            expected_scenario_fragment="PASS=1  FAIL=0",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_mixed_case_columns_when_running_commands_then_each_succeeds(
    test_case: QuotedMixedCaseColumnsBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="partner_feed", repo_files=_PROJECT_FILES
    )
    db_path: Path = project_dir / "partner_feed.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql='CREATE TABLE main.raw_listings AS SELECT \'L1\' AS "listingId", 1299::BIGINT AS "unitPriceMinor"',
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    builds: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_sqb(command=("--no-color", "build"), project_dir=project_dir) for _ in range(2)
    )
    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )
    replayed: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"), project_dir=project_dir
    )

    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert "0 errors, 0 warnings" in compiled.stdout, compiled.stdout
    for build in builds:
        assert build.returncode == 0, build.stdout + build.stderr
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path, sql='SELECT "listingId", "unitPrice" FROM main.listing_export'
    )
    assert rows == test_case.expected_export_rows
    assert tested.returncode == 0, tested.stdout + tested.stderr
    assert test_case.expected_test_fragment in tested.stdout, tested.stdout
    assert replayed.returncode == 0, replayed.stdout + replayed.stderr
    assert test_case.expected_scenario_fragment in replayed.stdout, replayed.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedContractE2ETestCase(
            description="an unquoted declaration enforces the quoted camelCase alias",
            declared_name="listingId",
            expected_rows=[("L1",)],
        ),
        QuotedContractE2ETestCase(
            description="a quoted declaration enforces the quoted camelCase alias",
            declared_name='"listingId"',
            expected_rows=[("L1",)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_enforced_contract_on_quoted_alias_when_building_then_contract_and_types_hold(
    test_case: QuotedContractE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="partner_feed",
        repo_files={
            **_PROJECT_FILES,
            "models/listing_export.sql": (
                "MODEL (\n  description 'Listings in the partner feed format.',\n"
                "  materialized table,\n  contract enforced,\n"
                f"  columns ({test_case.declared_name} (type VARCHAR),),\n);\n\n"
                'SELECT l.listing_id::VARCHAR AS "listingId"\nFROM __ref("stg_listings") AS l\n'
            ),
        },
    )
    db_path: Path = project_dir / "partner_feed.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql='CREATE TABLE main.raw_listings AS SELECT \'L1\' AS "listingId", 1299::BIGINT AS "unitPriceMinor"',
    )

    builds: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_sqb(command=("--no-color", "build"), project_dir=project_dir) for _ in range(2)
    )

    for build in builds:
        assert build.returncode == 0, build.stdout + build.stderr
        assert "runtime contract" not in build.stdout, build.stdout
    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path, sql='SELECT "listingId" FROM main.listing_export'
    )
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedConfiguredColumnsE2ETestCase(
            description="a quoted snapshot key and microbatch cursor build twice",
            unique_key="'\"listingId\"'",
            cursor="'\"eventAt\"'",
            expected_history_rows=[("L1", 1299, True), ("L1", 1499, False)],
            expected_activity_rows=[
                ("L1", datetime(2026, 1, 1, 0, 30)),
                ("L2", datetime(2026, 1, 1, 1, 30)),
                ("L3", datetime(2026, 1, 1, 2, 30)),
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_configured_columns_when_building_twice_then_snapshot_and_batches_apply(
    test_case: QuotedConfiguredColumnsE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="partner_feed",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"],
            "sources/raw.yml": (
                "sources:\n"
                "  - name: raw_listings\n    description: Partner listings.\n"
                "    schema: main\n    table: raw_listings\n"
                "  - name: raw_listing_events\n    description: Partner listing events.\n"
                "    schema: main\n    table: raw_listing_events\n"
            ),
            "models/listing_history.sql": (
                "MODEL (\n  description 'Listing price history.',\n  materialized snapshot,\n"
                f"  unique_key [{test_case.unique_key}],\n  snapshot_strategy timestamp,\n"
                "  updated_at '\"updatedAt\"',\n);\n\n"
                'SELECT "listingId", "unitPriceMinor", "updatedAt" FROM __source("raw_listings")\n'
            ),
            "models/listing_activity.sql": (
                "MODEL (\n  description 'Listing events by hour.',\n  materialized incremental,\n"
                "  incremental_strategy delete_insert,\n  incremental_mode microbatch,\n"
                "  microbatch_strategy watermark,\n  cursor_watermark_mode all,\n"
                f"  cursor {test_case.cursor},\n  cursor_type timestamp,\n  cursor_grain hour,\n"
                "  cursor_inputs (\n"
                '    raw_listing_events (column "eventAt", roles [filter, watermark]),\n  ),\n'
                "  batch_size 1h,\n);\n\n"
                'SELECT "listingId", "eventAt" FROM __source("raw_listing_events")\n'
            ),
        },
    )
    db_path: Path = project_dir / "partner_feed.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql=(
            "CREATE TABLE main.raw_listings AS SELECT 'L1' AS \"listingId\", "
            '1299::BIGINT AS "unitPriceMinor", TIMESTAMP \'2026-01-01 00:00:00\' AS "updatedAt";\n'
            "CREATE TABLE main.raw_listing_events AS SELECT * FROM (VALUES "
            "('L1', TIMESTAMP '2026-01-01 00:30:00'), ('L2', TIMESTAMP '2026-01-01 01:30:00')) "
            'AS v("listingId", "eventAt")'
        ),
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    execute_duckdb(
        db_path=db_path,
        sql=(
            'UPDATE main.raw_listings SET "unitPriceMinor" = 1499, '
            "\"updatedAt\" = TIMESTAMP '2026-01-02 00:00:00';\n"
            "INSERT INTO main.raw_listing_events VALUES ('L3', TIMESTAMP '2026-01-01 02:30:00')"
        ),
    )
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    for build in (first, second):
        assert build.returncode == 0, build.stdout + build.stderr
    history: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            'SELECT "listingId", "unitPriceMinor", valid_to IS NOT NULL '
            "FROM main.listing_history ORDER BY valid_from"
        ),
    )
    activity: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql='SELECT "listingId", "eventAt" FROM main.listing_activity ORDER BY "eventAt"',
    )
    assert history == test_case.expected_history_rows
    assert activity == test_case.expected_activity_rows


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
