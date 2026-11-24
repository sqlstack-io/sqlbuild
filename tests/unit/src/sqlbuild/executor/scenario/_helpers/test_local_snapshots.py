from __future__ import annotations

from pathlib import Path

import duckdb
import pytest

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scenario._helpers.local.snapshots import (
    load_scenario_snapshot_into_duckdb,
)
from sqlbuild.executor.scenario.constants import (
    SCENARIO_LOCAL_JSONL_INVALID,
    SCENARIO_LOCAL_LOAD_FAILED,
    SCENARIO_LOCAL_TYPE_INVALID,
)
from sqlbuild.executor.scenario.models import (
    ScenarioLocalSnapshotLoadResult,
    ScenarioSnapshotManifest,
)
from tests.unit.src.sqlbuild.executor.scenario._helpers._test_types import (
    ScenarioLocalSnapshotLoadErrorTestCase,
    ScenarioLocalSnapshotLoadTestCase,
    ScenarioLocalVariantLoadTestCase,
)
from tests.unit.src.sqlbuild.executor.scenario._helpers.helpers import (
    build_local_snapshot_load_manifest,
    write_local_snapshot_load_test_file_contents,
    write_local_snapshot_load_test_files,
)

SCENARIO_NAME: str = "revenue__customer_refund"
INPUT_FINGERPRINT: str = "fresh123"


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioLocalSnapshotLoadTestCase(
            description="loads typed jsonl rows into duckdb tables",
            columns=(
                ("order_id", "NUMBER", "BIGINT"),
                ("amount", "NUMBER(10,2)", "DECIMAL(10,2)"),
                ("order_date", "DATE", "DATE"),
            ),
            rows=(
                {"order_id": 1, "amount": "10.50", "order_date": "2026-01-02"},
                {"order_id": 2, "amount": "20.25", "order_date": "2026-01-03"},
            ),
            expected_table_name="__sqb_local__source__raw__orders",
            expected_summary_row=(2, 2, "30.75"),
        ),
        ScenarioLocalSnapshotLoadTestCase(
            description="loads warehouse-folded jsonl keys case-insensitively",
            columns=(
                ("order_id", "NUMBER", "BIGINT"),
                ("amount", "NUMBER(10,2)", "DECIMAL(10,2)"),
                ("order_date", "DATE", "DATE"),
            ),
            rows=(
                {"ORDER_ID": 1, "AMOUNT": "10.50", "ORDER_DATE": "2026-01-02"},
                {"ORDER_ID": 2, "AMOUNT": "20.25", "ORDER_DATE": "2026-01-03"},
            ),
            expected_table_name="__sqb_local__source__raw__orders",
            expected_summary_row=(2, 2, "30.75"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fresh_snapshot_when_loading_into_duckdb_then_creates_typed_tables(
    tmp_path: Path,
    test_case: ScenarioLocalSnapshotLoadTestCase,
) -> None:
    manifest: ScenarioSnapshotManifest = build_local_snapshot_load_manifest(
        input_fingerprint=INPUT_FINGERPRINT,
        columns=test_case.columns,
    )
    write_local_snapshot_load_test_files(
        project_dir=tmp_path,
        manifest=manifest,
        rows=test_case.rows,
    )
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")

    result: ScenarioLocalSnapshotLoadResult = load_scenario_snapshot_into_duckdb(
        project_dir=tmp_path,
        scenario_name=SCENARIO_NAME,
        current_input_fingerprint=INPUT_FINGERPRINT,
        connection=connection,
    )
    summary_row: tuple[object, ...] | None = connection.execute(
        f"SELECT COUNT(*), MAX(order_id), CAST(SUM(amount) AS VARCHAR) "
        f'FROM "{test_case.expected_table_name}"'
    ).fetchone()

    assert result.scenario_name == SCENARIO_NAME
    assert result.relations[0].table_name == test_case.expected_table_name
    assert summary_row is not None
    assert summary_row == test_case.expected_summary_row


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioLocalSnapshotLoadErrorTestCase(
            description="invalid local type reports column context",
            columns=(
                ("payload", "OBJECT", "NOT_A_DUCKDB_TYPE"),
                ("metadata", "VARIANT", "ALSO_NOT_A_DUCKDB_TYPE"),
            ),
            file_contents='{"payload":"abc"}\n',
            expected_error_code=SCENARIO_LOCAL_TYPE_INVALID,
            expected_error_fragments=(
                "DuckDB rejected 2 local_type errors",
                "local_type 'NOT_A_DUCKDB_TYPE'",
                "local_type 'ALSO_NOT_A_DUCKDB_TYPE'",
                "scenario 'revenue__customer_refund'",
                "relation 'source raw__orders'",
                "column 'payload'",
                "column 'metadata'",
                "scenario.json",
            ),
        ),
        ScenarioLocalSnapshotLoadErrorTestCase(
            description="malformed jsonl reports snapshot file context",
            columns=(("order_id", "NUMBER", "BIGINT"),),
            file_contents='{"order_id":1}\n{not json}\n',
            expected_error_code=SCENARIO_LOCAL_JSONL_INVALID,
            expected_error_fragments=(
                "invalid local snapshot JSONL",
                "sources/raw__orders.jsonl",
                "relation 'source raw__orders'",
                "line 2",
            ),
        ),
        ScenarioLocalSnapshotLoadErrorTestCase(
            description="uncastable jsonl value reports column context",
            columns=(("order_date", "DATE", "DATE"),),
            file_contents='{"order_date":"not-a-date"}\n',
            expected_error_code=SCENARIO_LOCAL_LOAD_FAILED,
            expected_error_fragments=(
                "could not load local snapshot JSONL",
                "sources/raw__orders.jsonl",
                "relation 'source raw__orders'",
                "column 'order_date'",
                "local_type 'DATE'",
                "row 1",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_local_snapshot_when_loading_then_raises_coded_error(
    tmp_path: Path,
    test_case: ScenarioLocalSnapshotLoadErrorTestCase,
) -> None:
    manifest: ScenarioSnapshotManifest = build_local_snapshot_load_manifest(
        input_fingerprint=INPUT_FINGERPRINT,
        columns=test_case.columns,
    )
    write_local_snapshot_load_test_file_contents(
        project_dir=tmp_path,
        manifest=manifest,
        file_contents=test_case.file_contents,
    )
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")

    with pytest.raises(ExecutorInputError) as exc_info:
        load_scenario_snapshot_into_duckdb(
            project_dir=tmp_path,
            scenario_name=SCENARIO_NAME,
            current_input_fingerprint=INPUT_FINGERPRINT,
            connection=connection,
        )

    assert exc_info.value.code == test_case.expected_error_code
    assert exc_info.value.help is not None
    error_text: str = str(exc_info.value)
    expected_fragment: str
    for expected_fragment in test_case.expected_error_fragments:
        assert expected_fragment in error_text


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioLocalVariantLoadTestCase(
            description="driver JSON text of an object loads as that object",
            value='{\n  "qty": 2\n}',
            expected_variant_type="OBJECT(qty)",
            expected_matches_parsed_json=True,
        ),
        ScenarioLocalVariantLoadTestCase(
            description="driver JSON text of a string loads as the string",
            value='"placed"',
            expected_variant_type="VARCHAR",
            expected_matches_parsed_json=False,
        ),
        ScenarioLocalVariantLoadTestCase(
            description="text that is not JSON loads as a string",
            value="placed",
            expected_variant_type="VARCHAR",
            expected_matches_parsed_json=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_variant_text_when_loading_into_duckdb_then_it_loads_as_parsed_json(
    tmp_path: Path,
    test_case: ScenarioLocalVariantLoadTestCase,
) -> None:
    manifest: ScenarioSnapshotManifest = build_local_snapshot_load_manifest(
        input_fingerprint=INPUT_FINGERPRINT,
        columns=(("payload", "VARIANT", "VARIANT"),),
    )
    write_local_snapshot_load_test_files(
        project_dir=tmp_path, manifest=manifest, rows=({"PAYLOAD": test_case.value},)
    )
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")

    result: ScenarioLocalSnapshotLoadResult = load_scenario_snapshot_into_duckdb(
        project_dir=tmp_path,
        scenario_name=SCENARIO_NAME,
        current_input_fingerprint=INPUT_FINGERPRINT,
        connection=connection,
    )
    loaded: tuple[object, ...] | None = connection.execute(
        "SELECT variant_typeof(payload), "
        "payload IN (SELECT CAST(CAST('{\"qty\": 2}' AS JSON) AS VARIANT)) "
        f'FROM "{result.relations[0].table_name}"'
    ).fetchone()

    assert loaded == (
        test_case.expected_variant_type,
        test_case.expected_matches_parsed_json,
    ), test_case.description
