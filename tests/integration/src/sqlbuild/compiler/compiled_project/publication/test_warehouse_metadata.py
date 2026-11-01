"""Real warehouse writers, populated predecessors and the isolated read-only history protocol."""

import hashlib
import json
import sqlite3
from contextlib import closing
from datetime import UTC, datetime, timedelta, timezone
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    Fingerprint,
    SourceFreshnessRecord,
    SourceFreshnessRenderers,
    read_latest_fingerprints,
    read_latest_source_freshness,
    write_fingerprints,
    write_source_freshness_records,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CatalogConnectionTestCase,
    FingerprintHistoryTestCase,
    WarehouseHistoryTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    append_contended_fingerprints,
    append_retried_fingerprints,
    open_legacy_warehouse,
    warehouse_history_page,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CatalogConnectionTestCase("DuckDB independent reader", "duckdb", "database", "legacy-run"),
        CatalogConnectionTestCase("SQLite independent reader", "sqlite", "path", "legacy-run"),
    ],
    ids=lambda case: case.description,
)
def test_given_independent_catalog_connection_when_reading_history_then_writer_secrets_and_files_are_untouched(
    test_case: CatalogConnectionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    connection_key: str = test_case.connection_key
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(project, test_case.adapter_name, connection_key)
    adapter.close(connection)
    config: Path = project / "sqlbuild_project.toml"
    config.write_text(
        Path("benchmarks/duckshop/edge_cases/orchestration/catalog_connection.toml")
        .read_text()
        .replace('adapter = "sqlite"', f'adapter = "{test_case.adapter_name}"')
        .replace("\npath =", f"\n{connection_key} =")
    )
    monkeypatch.delenv("SQLBUILD_MISSING_WRITER_PASSWORD", raising=False)
    monkeypatch.setenv("SQLBUILD_CATALOG_DATABASE", "warehouse.db")
    first: Any = warehouse_history_page(project, "source_freshness")
    assert first is not None and len(first["records"]) == 1
    assert first["records"][0]["run_id"] == test_case.expected_run_id
    # Local target selection replaces only the catalog name, not the writer connection.
    (project / "warehouse.db").rename(project / "retained-warehouse.db")
    (project / "sqlbuild_local.toml").write_text(
        f'[connections.mirror]\n{connection_key} = "retained-warehouse.db"\n'
        '[targets.development]\ncatalog_connection = "mirror"\n'
    )
    assert warehouse_history_page(project, "source_freshness") == first
    assert not (project / "must-not-open.db").exists()
    assert not (project / "must-not-open-writer.db").exists()
    assert not (project / ".sqlbuild/control.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseHistoryTestCase(
            "DuckDB typed timestamps",
            "duckdb",
            (100, 6),
            "database",
            (),
            append_retried_fingerprints,
        ),
        WarehouseHistoryTestCase(
            "SQLite timestamp text",
            "sqlite",
            (100, 6),
            "path",
            ("delete",),
            append_contended_fingerprints,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_populated_predecessor_tables_when_writers_add_ids_then_history_preserves_new_legacy_and_retried_observations(
    test_case: WarehouseHistoryTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    connection_key: str = test_case.connection_key
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    adapter.close(connection)
    for journal_mode in test_case.journal_modes:
        with closing(sqlite3.connect(project / "warehouse.db")) as predecessor:
            assert predecessor.execute("PRAGMA journal_mode=DELETE").fetchone() == (journal_mode,)

    def page(after: list[str | None] | None = None, observed_since: str | None = None) -> Any:
        return warehouse_history_page(project, "source_freshness", after, observed_since)

    old: dict[str, Any] = page()
    assert len(old["records"]) == 1 and old["records"][0]["event_id"] is None
    assert page(observed_since="2026-01-01T09:00:00Z") == old
    assert page(observed_since="2026-01-01T10:00:00.000001Z") == {
        "records": [],
        "next_after": None,
    }
    for journal_mode in test_case.journal_modes:
        with closing(sqlite3.connect(project / "warehouse.db")) as predecessor:
            assert predecessor.execute("PRAGMA journal_mode").fetchone() == (journal_mode,)
    connection = adapter.connect({connection_key: str(project / "warehouse.db")})
    # Reading history never upgrades the warehouse or imports project declarations.
    assert "event_id" not in {
        column.name
        for column in adapter.get_columns(
            connection=connection, database=None, schema="main", name="_sqlbuild_source_freshness"
        )
    }
    latest: Any = read_latest_fingerprints(
        connection=connection,
        execute=adapter.execute,
        table_exists=True,
        database=None,
        schema="main",
        render_qualified_name=adapter.render_qualified_name,
        render_read_latest_sql=adapter.render_read_latest_fingerprints_sql,
    )
    assert latest.fingerprints["orders"].run_id == "legacy-run"
    fingerprint: Fingerprint = Fingerprint(
        node_type="model",
        node_name="orders",
        target_database=None,
        target_schema="main",
        target_name="orders",
        run_id="external-run",
        definition_hash="definition",
        version_hash="version",
        schema_fingerprint="schema",
        definition="SELECT 2 AS order_id",
        ts=datetime(2026, 2, 1, tzinfo=UTC),
    )
    test_case.append_fingerprints(
        adapter=adapter,
        connection=connection,
        database_path=project / "warehouse.db",
        fingerprint=fingerprint,
    )
    expected_fingerprint_id: str = hashlib.sha256(
        json.dumps(
            [
                "sqlbuild.fingerprint.event.v1",
                "model",
                "orders",
                None,
                "main",
                "orders",
                "external-run",
                "version",
            ],
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode()
    ).hexdigest()
    assert adapter.query(
        connection=connection,
        sql="SELECT event_id FROM main._sqlbuild_fingerprints ORDER BY ts",
        limit=10,
    ).rows == (
        (None,),
        (expected_fingerprint_id,),
        (expected_fingerprint_id,),
    )
    records: tuple[SourceFreshnessRecord, ...] = tuple(
        SourceFreshnessRecord(
            source_name=("Orders", "orders", "orders ", "ordérs")[index % 4],
            target_database=("", None)[index % 2],
            target_schema="main",
            target_name="orders",
            run_id=f"external-{index}",
            strategy="max_column",
            value_kind="string",
            data_version=("o'rders 🧾", None, None)[index % 3],
            data_version_hash=f"value-{index}",
            observed_at=datetime(2026, 2, 1, microsecond=index, tzinfo=UTC),
        )
        for index in range(104)
    )
    renderers: SourceFreshnessRenderers = SourceFreshnessRenderers(
        render_qualified_name=adapter.render_qualified_name,
        render_framework_type=adapter.render_framework_type,
        render_insert_records_sql=adapter.render_insert_source_freshness_records_sql,
        render_create_table_sql=adapter.render_create_source_freshness_table_sql,
        read_columns=adapter.get_columns,
        render_add_columns=adapter.render_add_columns,
    )
    for batch in (records, records[:1]):
        write_source_freshness_records(
            connection=connection,
            execute=adapter.execute,
            database=None,
            schema="main",
            records=batch,
            renderers=renderers,
        )
    expected_freshness_id: str = hashlib.sha256(
        json.dumps(
            [
                "sqlbuild.source-freshness.event.v1",
                "Orders",
                "",
                "main",
                "orders",
                "external-0",
                "2026-02-01T00:00:00.000000+00:00",
                "max_column",
                "string",
                "value-0",
            ],
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode()
    ).hexdigest()
    assert adapter.query(
        connection=connection,
        sql="SELECT event_id FROM main._sqlbuild_source_freshness WHERE run_id='external-0'",
        limit=10,
    ).rows == (
        (expected_freshness_id,),
        (expected_freshness_id,),
    )
    # An equivalent offset denotes the same observation, not another event.
    equivalent: SourceFreshnessRecord = SourceFreshnessRecord(
        source_name="Orders",
        target_database="",
        target_schema="main",
        target_name="orders",
        run_id="external-0",
        strategy="max_column",
        value_kind="string",
        data_version="o'rders 🧾",
        data_version_hash="value-0",
        observed_at=datetime(2026, 2, 1, 1, tzinfo=timezone(timedelta(hours=1))),
    )
    assert expected_freshness_id in adapter.render_insert_source_freshness_records_sql(
        database=None, schema="main", records=(equivalent,)
    )
    current: Any = read_latest_source_freshness(
        connection=connection,
        execute=adapter.execute,
        table_exists=True,
        database=None,
        schema="main",
        render_qualified_name=adapter.render_qualified_name,
        render_read_latest_sql=adapter.render_read_latest_source_freshness_sql,
    )
    assert {row.run_id for row in current.records.values()} == {
        "legacy-run",
        "external-100",
        "external-101",
        "external-102",
        "external-103",
    }
    adapter.close(connection)
    pages: list[dict[str, Any]] = [page()]
    while pages[-1]["next_after"] is not None:
        pages.append(page(pages[-1]["next_after"]))
        assert len(pages) < 4
    assert tuple(len(item["records"]) for item in pages) == test_case.expected_page_lengths
    imported: list[dict[str, Any]] = []
    for item in pages:
        imported.extend(item["records"])
    event_ids: set[str | None] = {row["event_id"] for row in imported}
    assert len(event_ids - {None}) == 104
    assert sum(row["event_id"] is None for row in imported) == 1
    assert {row["run_id"] for row in imported} == {"legacy-run"} | {
        f"external-{index}" for index in range(104)
    }
    assert {row["target_database"] for row in imported} == {None, ""}
    # The time bound is inclusive and independent of source-name/keyset ordering.
    # SQL may read a superset within the boundary day, so a short decoded page
    # must retain its continuation if the raw query filled the page.
    observed_since: str = "2026-02-01T01:00:00.000050+01:00"
    recent: list[dict[str, Any]] = [page(observed_since=observed_since)]
    while recent[-1]["next_after"] is not None:
        recent.append(page(recent[-1]["next_after"], observed_since))
        assert len(recent) < 4
    recent_records: list[dict[str, Any]] = []
    for item in recent:
        recent_records.extend(item["records"])
    assert len(recent_records) == 54
    assert {row["run_id"] for row in recent_records} == {
        f"external-{index}" for index in range(50, 104)
    }
    zero: dict[str, Any] = page(observed_since="2026-02-01T00:00:00Z")
    assert (
        len(zero["records"]) + len(page(zero["next_after"], "2026-02-01T00:00:00Z")["records"])
        == 105
    )
    assert sum(row["run_id"] == "external-0" for row in zero["records"]) == 2
    # An empty decoded page can still have another bounded SQL page to inspect.
    empty_since: str = "2026-02-01T00:00:00.000104Z"
    empty: dict[str, Any] = page(observed_since=empty_since)
    assert empty["records"] == [] and empty["next_after"] is not None
    assert page(empty["next_after"], empty_since) == {"records": [], "next_after": None}
    assert page(observed_since="2030-01-01T00:00:00Z") == {
        "records": [],
        "next_after": None,
    }
    assert not (project / ".sqlbuild/control.sqlite3").exists()
    (project / "warehouse.db").rename(project / "retained-warehouse.db")
    assert page() is None
    assert not (project / "warehouse.db").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        FingerprintHistoryTestCase(
            "DuckDB typed fingerprint timestamps",
            "duckdb",
            "database",
            datetime(2026, 2, 1, tzinfo=UTC),
            (),
            (),
            ("legacy-run",),
        ),
        FingerprintHistoryTestCase(
            "SQLite offset fingerprint timestamps",
            "sqlite",
            "path",
            datetime(2026, 1, 31, 23, tzinfo=timezone(timedelta(hours=-1))),
            ("delete",),
            (
                Path(
                    "benchmarks/duckshop/edge_cases/orchestration/legacy_warehouse_fingerprint_offsets.sql"
                ),
            ),
            ("legacy-run", "offset-run"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_history_when_read_in_isolated_pages_then_legacy_retries_definitions_and_time_bounds_survive(
    test_case: FingerprintHistoryTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    for fixture in test_case.fixture_paths:
        _ = adapter.execute(connection=connection, sql=fixture.read_text())
    adapter.close(connection)

    def page(after: list[str | None] | None = None, observed_since: str | None = None) -> Any:
        return warehouse_history_page(project, "fingerprints", after, observed_since)

    legacy: dict[str, Any] = page()
    assert legacy["next_after"] is None
    assert {row["run_id"] for row in legacy["records"]} == set(test_case.expected_legacy_runs)
    assert legacy["records"][0]["event_id"] is None
    assert legacy["records"][0]["definition"] == "SELECT 1 AS order_id"
    assert legacy["records"][0]["metadata_json"] == "{}"
    assert page(observed_since="2026-01-01T09:00:00Z") == legacy
    assert page(observed_since="2026-01-01T10:00:00.000001Z") == {
        "records": [],
        "next_after": None,
    }
    connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
    assert "event_id" not in {
        column.name
        for column in adapter.get_columns(
            connection=connection, database=None, schema="main", name="_sqlbuild_fingerprints"
        )
    }
    fingerprints: tuple[Fingerprint, ...] = tuple(
        Fingerprint(
            node_type=("model", "seed")[index % 2],
            node_name=("Orders", "orders", "orders ", "ordérs", "o'rders 🧾")[index % 5],
            target_database=(None, "")[index % 2],
            target_schema="main",
            target_name="orders",
            run_id=f"external-{index}",
            definition_hash="definition",
            version_hash=f"version-{index}",
            schema_fingerprint="schema",
            definition="SELECT 'o''rders 🧾' AS order_note",
            metadata_json='{"note":"orders 🧾"}',
            ts=test_case.observed_at + timedelta(microseconds=index),
        )
        for index in range(104)
    )
    for batch in (fingerprints, fingerprints[:1]):
        write_fingerprints(
            connection=connection,
            execute=adapter.execute,
            database=None,
            schema="main",
            fingerprints=batch,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
            render_create_table_sql=adapter.render_create_fingerprint_table_sql,
            read_columns=adapter.get_columns,
            render_add_columns=adapter.render_add_columns,
        )
    adapter.close(connection)
    for journal_mode in test_case.journal_modes:
        with closing(sqlite3.connect(project / "warehouse.db")) as predecessor:
            assert predecessor.execute("PRAGMA journal_mode=DELETE").fetchone() == (journal_mode,)
    pages: list[dict[str, Any]] = [page()]
    while pages[-1]["next_after"] is not None:
        assert len(pages[-1]["next_after"]) == 13
        pages.append(page(pages[-1]["next_after"]))
        assert len(pages) < 4
    assert tuple(len(item["records"]) for item in pages) == (
        100,
        5 + len(test_case.expected_legacy_runs),
    )
    imported: list[dict[str, Any]] = []
    for item in pages:
        imported.extend(item["records"])
    assert {row["run_id"] for row in imported} == set(test_case.expected_legacy_runs) | {
        f"external-{index}" for index in range(104)
    }
    assert len({row["event_id"] for row in imported} - {None}) == 104
    assert sum(row["event_id"] is None for row in imported) == len(test_case.expected_legacy_runs)
    assert {row["target_database"] for row in imported} == {None, ""}
    first: list[dict[str, Any]] = list(filter(lambda row: row["run_id"] == "external-0", imported))
    assert len(first) == 2 and first[0] == first[1]
    assert first[0]["definition"] == "SELECT 'o''rders 🧾' AS order_note"
    assert first[0]["metadata_json"] == '{"note":"orders 🧾"}'
    assert (
        first[0]["event_id"]
        == hashlib.sha256(
            b'["sqlbuild.fingerprint.event.v1","model","Orders",null,"main","orders","external-0","version-0"]'
        ).hexdigest()
    )
    assert datetime.fromisoformat(first[0]["ts"]) == datetime(2026, 2, 1, tzinfo=UTC)
    observed_since: str = "2026-02-01T01:00:00.000050+01:00"
    recent: list[dict[str, Any]] = [page(observed_since=observed_since)]
    while recent[-1]["next_after"] is not None:
        recent.append(page(recent[-1]["next_after"], observed_since))
        assert len(recent) < 4
    recent_records: list[dict[str, Any]] = []
    for item in recent:
        recent_records.extend(item["records"])
    assert len(recent_records) == 54
    assert {row["run_id"] for row in recent_records} == {
        f"external-{index}" for index in range(50, 104)
    }
    empty_since: str = "2026-02-01T00:00:00.000104Z"
    empty: dict[str, Any] = page(observed_since=empty_since)
    assert empty["records"] == [] and empty["next_after"] is not None
    assert page(empty["next_after"], empty_since) == {"records": [], "next_after": None}
    for journal_mode in test_case.journal_modes:
        with closing(sqlite3.connect(project / "warehouse.db")) as predecessor:
            assert predecessor.execute("PRAGMA journal_mode").fetchone() == (journal_mode,)
    assert not (project / ".sqlbuild/control.sqlite3").exists()
    (project / "warehouse.db").rename(project / "retained-warehouse.db")
    assert page() is None
    assert not (project / "warehouse.db").exists()
