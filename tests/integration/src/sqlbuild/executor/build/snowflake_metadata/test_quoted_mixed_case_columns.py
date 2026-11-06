"""A Snowflake source whose columns are stored in quoted mixed case plans and builds offline."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.executor.build.snowflake_metadata._test_types import (
    QuotedColumnBuildTestCase,
    QuotedConfiguredColumnTestCase,
    QuotedContractBuildTestCase,
)
from tests.integration.src.sqlbuild.executor.build.snowflake_metadata.helpers import (
    SimulatedSnowflakeWarehouse,
    build_with_cli,
    quoted_configured_column_files,
)
from tests.unit.src.sqlbuild.adapters.snowflake.inspection.helpers import (
    FakeColumn,
    FakeRelation,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "partner_feed"\nadapter = "snowflake"\n\n'
        '[connection]\naccount = "example"\nuser = "example"\n'
        'database = "analytics"\nschema = "marts"\n\n'
        '[defaults]\ndatabase = "analytics"\nschema = "marts"\nmaterialized = "table"\n'
    ),
    "sources/raw.yml": (
        "sources:\n"
        "  - name: raw_listings\n    description: Partner listings in quoted camelCase.\n"
        "    database: analytics\n    schema: raw\n    table: listings\n"
        "    columns:\n"
        "      - name: '\"listingId\"'\n        type: VARCHAR\n"
        "      - name: '\"unitPriceMinor\"'\n        type: NUMBER(38,0)\n"
    ),
    "models/stg_listings.sql": (
        "MODEL (description 'Listings renamed to snake case.');\n\n"
        'SELECT s."listingId" AS listing_id, s."unitPriceMinor" AS unit_price_minor\n'
        'FROM __source("raw_listings") AS s\n'
    ),
    "models/listing_export.sql": (
        "MODEL (\n  description 'Listings in the partner feed format.',\n"
        '  columns ("listingId" (nullable false, audits [not_null]),),\n);\n\n'
        'WITH final AS (\n  SELECT l.listing_id AS "listingId"\n'
        '  FROM __ref("stg_listings") AS l\n)\n\nSELECT *\nFROM final\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedColumnBuildTestCase(
            description="quoted camelCase source columns bind in planning on a first build and rebuild",
            stored_columns=("listingId", "unitPriceMinor"),
            expected_fragment='"listingId" IS NULL',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_mixed_case_source_columns_when_building_then_planning_binds_them(
    test_case: QuotedColumnBuildTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    for relative, content in _PROJECT_FILES.items():
        path: Path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")
    columns: tuple[FakeColumn, ...] = tuple(
        FakeColumn(name=name, data_type="TEXT") for name in test_case.stored_columns
    )
    warehouse: SimulatedSnowflakeWarehouse = SimulatedSnowflakeWarehouse(
        relations=(
            FakeRelation(database="ANALYTICS", schema="RAW", name="LISTINGS", columns=columns),
        ),
    )

    builds: tuple[tuple[int, tuple[str, ...]], ...] = tuple(
        build_with_cli(project_dir=tmp_path, warehouse=warehouse, monkeypatch=monkeypatch)
        for _ in range(2)
    )

    output: str = capsys.readouterr().out
    assert tuple(build[0] for build in builds) == (0, 0), output
    assert "B002" not in output, output
    assert any(test_case.expected_fragment in statement for statement in builds[1][1]), builds[1][1]


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedContractBuildTestCase(
            description="an unquoted declaration matches the quoted camelCase alias",
            declared_name="listingId",
            produced_columns=("listingId",),
        ),
        QuotedContractBuildTestCase(
            description="a quoted declaration matches the quoted camelCase alias",
            declared_name='"listingId"',
            produced_columns=("listingId",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_enforced_contract_on_quoted_alias_when_building_then_runtime_contract_passes(
    test_case: QuotedContractBuildTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    files: dict[str, str] = {
        **_PROJECT_FILES,
        "models/listing_export.sql": (
            "MODEL (\n  description 'Listings in the partner feed format.',\n"
            "  contract enforced,\n"
            f"  columns ({test_case.declared_name} (type VARCHAR),),\n);\n\n"
            'SELECT l.listing_id::VARCHAR AS "listingId"\nFROM __ref("stg_listings") AS l\n'
        ),
    }
    for relative, content in files.items():
        path: Path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")
    warehouse: SimulatedSnowflakeWarehouse = SimulatedSnowflakeWarehouse(
        relations=(
            FakeRelation(
                database="ANALYTICS",
                schema="RAW",
                name="LISTINGS",
                columns=(
                    FakeColumn(name="listingId", data_type="TEXT"),
                    FakeColumn(name="unitPriceMinor", data_type="NUMBER"),
                ),
            ),
        ),
    )
    warehouse.set_model_columns(
        model="listing_export",
        columns=tuple(
            FakeColumn(name=name, data_type="TEXT") for name in test_case.produced_columns
        ),
    )

    exit_code, _statements = build_with_cli(
        project_dir=tmp_path, warehouse=warehouse, monkeypatch=monkeypatch
    )

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code, output
    assert "runtime contract" not in output, output


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedConfiguredColumnTestCase(
            description="quoted names keep their quotes in snapshot and microbatch SQL",
            unique_key="'\"listingId\"'",
            updated_at="'\"updatedAt\"'",
            cursor="'\"eventAt\"'",
            expected_exit_code=0,
            expected_fragments=(
                'GROUP BY "listingId"',
                '__sqb_rebuild__listing_activity WHERE "eventAt" >=',
            ),
        ),
        QuotedConfiguredColumnTestCase(
            description="unquoted names for case-sensitive columns are rejected clearly",
            unique_key="listingId",
            updated_at="updatedAt",
            cursor="eventAt",
            expected_exit_code=1,
            expected_errors=(
                "unique_key 'listingId' names the case-sensitive column \"listingId\"; "
                "write it with its quotes, as '\"listingId\"'",
                "cursor 'eventAt' names the case-sensitive column \"eventAt\"",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_configured_quoted_columns_when_building_then_sql_keeps_quotes_or_rejects(
    test_case: QuotedConfiguredColumnTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    for relative, content in quoted_configured_column_files(
        unique_key=test_case.unique_key, updated_at=test_case.updated_at, cursor=test_case.cursor
    ).items():
        path: Path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(content, encoding="utf-8")
    listing_columns: tuple[FakeColumn, ...] = (
        FakeColumn(name="listingId"),
        FakeColumn(name="unitPrice", data_type="NUMBER"),
        FakeColumn(name="updatedAt", data_type="TIMESTAMP_NTZ"),
    )
    event_columns: tuple[FakeColumn, ...] = (
        FakeColumn(name="listingId"),
        FakeColumn(name="eventAt", data_type="TIMESTAMP_NTZ"),
    )
    warehouse: SimulatedSnowflakeWarehouse = SimulatedSnowflakeWarehouse(
        relations=(
            FakeRelation(
                database="ANALYTICS", schema="RAW", name="LISTINGS", columns=listing_columns
            ),
            FakeRelation(
                database="ANALYTICS", schema="RAW", name="LISTING_EVENTS", columns=event_columns
            ),
        ),
    )
    warehouse.set_model_columns(model="listing_history", columns=listing_columns)
    warehouse.set_model_columns(model="listing_activity", columns=event_columns)

    builds: tuple[tuple[int, tuple[str, ...]], ...] = tuple(
        build_with_cli(project_dir=tmp_path, warehouse=warehouse, monkeypatch=monkeypatch)
        for _ in range(2)
    )

    output: str = capsys.readouterr().out
    assert tuple(build[0] for build in builds) == (test_case.expected_exit_code,) * 2, output
    statements: str = "\n".join(("\n".join(builds[0][1]), "\n".join(builds[1][1])))
    fragment: str
    for fragment in test_case.expected_fragments:
        assert fragment in statements, statements
    error: str
    for error in test_case.expected_errors:
        assert error in output, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
