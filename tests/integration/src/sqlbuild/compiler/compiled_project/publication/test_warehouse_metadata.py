"""Real warehouse writers, populated predecessors and the isolated read-only history protocol."""

import base64
import gzip
import hashlib
import json
import os
import sqlite3
import sys
import time
from contextlib import closing
from datetime import UTC, datetime, timedelta, timezone
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen
from uuid import uuid4

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    Fingerprint,
    SourceFreshnessRecord,
    SourceFreshnessRenderers,
    build_compiled_project,
    build_orchestration_snapshot,
    discover_project_inputs,
    read_latest_fingerprints,
    read_latest_source_freshness,
    write_fingerprints,
    write_source_freshness_records,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    METADATA,
    TYPE_PROOF_FILES,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    AutomaticHistoryTestCase,
    CatalogConnectionTestCase,
    FingerprintHistoryTestCase,
    KeyPageTestCase,
    StateEventsTestCase,
    WarehouseHistoryTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    record_finished_run,
    request_warehouse_sync,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    append_audit_history,
    append_column_migration_history,
    append_consumption_history,
    append_contended_fingerprints,
    append_janitor_history,
    append_legacy_history,
    append_microbatch_history,
    append_migration_history,
    append_node_result_history,
    append_observation_history,
    append_old_name_history,
    append_retried_fingerprints,
    hold_write_lock,
    location_process,
    open_legacy_warehouse,
    snapshot_api,
    sync_warehouse_history,
    wait_for_generation,
    wait_for_location_work,
    wait_for_warehouse_sync_health,
    warehouse_history_page,
    warehouse_history_pages,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        StateEventsTestCase(
            f"{adapter_name} {fixture_name}",
            adapter_name,
            connection_key,
            expected_run_id,
            fixture_name,
            append_history,
            legacy_columns,
        )
        for adapter_name, connection_key, fixture_name, append_history, expected_run_id, legacy_columns in [
            (
                "duckdb",
                "database",
                "audit_result_history",
                append_audit_history,
                "",
                ("audit_description", "audit_meta_json"),
            ),
            (
                "duckdb",
                "database",
                "janitor_event_history",
                append_janitor_history,
                "cleanup-run",
                (),
            ),
            (
                "sqlite",
                "path",
                "audit_result_history",
                append_audit_history,
                "",
                ("audit_description", "audit_meta_json"),
            ),
            ("sqlite", "path", "janitor_event_history", append_janitor_history, "cleanup-run", ()),
            # SQLite has no supported concurrent-microbatch coordination writer.
            ("duckdb", "database", "microbatch_history", append_microbatch_history, "", ()),
            (
                "duckdb",
                "database",
                "migration_history",
                append_migration_history,
                "external-move",
                (),
            ),
            (
                "duckdb",
                "database",
                "old_name_history",
                append_old_name_history,
                "external-move",
                (),
            ),
            ("sqlite", "path", "migration_history", append_migration_history, "external-move", ()),
            ("sqlite", "path", "old_name_history", append_old_name_history, "external-move", ()),
            (
                "duckdb",
                "database",
                "column_migration_history",
                append_column_migration_history,
                "external-rename",
                (),
            ),
            (
                "sqlite",
                "path",
                "column_migration_history",
                append_column_migration_history,
                "external-rename",
                (),
            ),
            (
                "duckdb",
                "database",
                "node_result_history",
                append_node_result_history,
                "external-run",
                (),
            ),
            (
                "duckdb",
                "database",
                "consumption_history",
                append_consumption_history,
                "external-run",
                (),
            ),
            (
                "duckdb",
                "database",
                "observation_history",
                append_observation_history,
                "external-run",
                (),
            ),
            (
                "sqlite",
                "path",
                "observation_history",
                append_observation_history,
                "external-run",
                (),
            ),
            (
                "duckdb",
                "database",
                "old_name_history",
                append_old_name_history,
                "external-move",
                ("column_aliases",),
            ),
            (
                "sqlite",
                "path",
                "old_name_history",
                append_old_name_history,
                "external-move",
                ("column_aliases",),
            ),
        ]
    ],
    ids=lambda case: case.description,
)
def test_given_external_state_events_when_reading_pages_then_identities_text_and_timestamps_survive(
    test_case: StateEventsTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    fixture: dict[str, Any] = json.loads(
        Path(
            f"benchmarks/duckshop/edge_cases/orchestration/{test_case.fixture_name}.json"
        ).read_text()
    )
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    test_case.append_history(adapter, connection, fixture)
    table: str = f"_sqlbuild_{fixture['kind']}"
    source_key: str = fixture["source_key"]
    legacy_columns: tuple[str, ...] = test_case.legacy_columns + tuple(
        fixture.get("legacy_columns", ())
    )
    for column in legacy_columns:
        _ = adapter.execute(connection=connection, sql=f"ALTER TABLE {table} DROP COLUMN {column}")
    adapter.close(connection)
    pages: list[dict[str, Any]] = warehouse_history_pages(project, fixture["kind"])
    rows: list[dict[str, Any]] = []
    for page in pages:
        rows.extend(page["records"])
    assert [len(page["records"]) for page in pages] == fixture["page_lengths"]
    assert {row[source_key] for row in rows} == {
        f"{fixture['id_prefix']}-{index:03}" for index in range(fixture["source_rows"])
    }
    expected: dict[str, Any] = fixture["record"] | dict.fromkeys(legacy_columns)
    assert rows[0] == expected
    assert rows[0][fixture.get("run_key", "run_id")] == test_case.expected_run_id
    assert rows[fixture["later_index"]] == (
        expected | fixture["later_record"] | dict.fromkeys(legacy_columns)
    )
    assert rows[-1] == expected | fixture["last_record"] | dict.fromkeys(legacy_columns)
    recent_pages: list[dict[str, Any]] = warehouse_history_pages(
        project, fixture["kind"], fixture["later_time"]
    )
    recent_rows: list[dict[str, Any]] = []
    for page in recent_pages:
        recent_rows.extend(page["records"])
    assert recent_rows == [expected | fixture["later_record"] | dict.fromkeys(legacy_columns)]
    connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
    columns: set[str] = {
        column.name
        for column in adapter.get_columns(
            connection=connection, database=None, schema="main", name=table
        )
    }
    assert not columns.intersection(legacy_columns), "the reader never upgrades source storage"
    assert adapter.query(
        connection=connection, sql=f"SELECT COUNT(*) FROM {table}", limit=1
    ).rows == ((fixture["source_count"],),)
    adapter.close(connection)
    assert not (project / ".sqlbuild/control.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        StateEventsTestCase(
            f"{adapter_name} {fixture_name}",
            adapter_name,
            connection_key,
            expected_run_id,
            fixture_name,
            append_history,
            legacy_columns,
        )
        for adapter_name, connection_key, fixture_name, append_history, expected_run_id, legacy_columns in [
            (
                "duckdb",
                "database",
                "audit_result_history",
                append_audit_history,
                "",
                ("audit_description", "audit_meta_json"),
            ),
            (
                "duckdb",
                "database",
                "janitor_event_history",
                append_janitor_history,
                "cleanup-run",
                (),
            ),
            (
                "sqlite",
                "path",
                "audit_result_history",
                append_audit_history,
                "",
                ("audit_description", "audit_meta_json"),
            ),
            ("sqlite", "path", "janitor_event_history", append_janitor_history, "cleanup-run", ()),
            ("duckdb", "database", "microbatch_history", append_microbatch_history, "", ()),
            (
                "duckdb",
                "database",
                "migration_history",
                append_migration_history,
                "external-move",
                (),
            ),
            (
                "duckdb",
                "database",
                "old_name_history",
                append_old_name_history,
                "external-move",
                (),
            ),
            ("sqlite", "path", "migration_history", append_migration_history, "external-move", ()),
            ("sqlite", "path", "old_name_history", append_old_name_history, "external-move", ()),
            (
                "duckdb",
                "database",
                "column_migration_history",
                append_column_migration_history,
                "external-rename",
                (),
            ),
            (
                "sqlite",
                "path",
                "column_migration_history",
                append_column_migration_history,
                "external-rename",
                (),
            ),
            (
                "duckdb",
                "database",
                "node_result_history",
                append_node_result_history,
                "external-run",
                (),
            ),
            (
                "duckdb",
                "database",
                "consumption_history",
                append_consumption_history,
                "external-run",
                (),
            ),
            (
                "duckdb",
                "database",
                "observation_history",
                append_observation_history,
                "external-run",
                (),
            ),
            (
                "sqlite",
                "path",
                "observation_history",
                append_observation_history,
                "external-run",
                (),
            ),
        ]
    ],
    ids=lambda case: case.description,
)
def test_given_a_served_location_when_state_event_history_is_rebuilt_and_retained_then_only_complete_scans_delete(
    test_case: StateEventsTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    fixture: dict[str, Any] = json.loads(
        Path(
            f"benchmarks/duckshop/edge_cases/orchestration/{test_case.fixture_name}.json"
        ).read_text()
    )
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    test_case.append_history(adapter, connection, fixture)
    adapter.close(connection)
    (project / "orchestration/orders.py").write_text("")
    write_files(project_dir=project, files={"models/orders.sql": fixture["model"]})
    with snapshot_api(tmp_path / "control") as api:
        log: Path = tmp_path / "location.log"
        control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ
            | {"SQLBUILD_LOCATION_TOKEN": api.bearer, "SQLBUILD_METADATA_RECONCILE_SECONDS": "30"},
        ) as location:
            _ = wait_for_generation(api, "development", 1, location, log)
            identities: dict[str, str] = {}
            source_key: str = fixture["source_key"]
            mirror_table: str = f"sqb_warehouse_{fixture['kind']}"
            all_ids: set[str] = {
                f"{fixture['id_prefix']}-{index:03}" for index in range(fixture["source_rows"])
            }
            for expected_ids, mirror_sql, warehouse_sql in (
                (all_ids, f"DELETE FROM {mirror_table}", "SELECT 1"),
                (
                    all_ids,
                    "SELECT 1",
                    f"DELETE FROM _sqlbuild_{fixture['kind']} "
                    f"WHERE {source_key} <> '{fixture['retained_result']}'",
                ),
                ({fixture["retained_result"]}, "SELECT 1", "SELECT 1"),
            ):
                deadline: float = time.monotonic() + 45
                mirrored: list[tuple[str, str]] = []
                while len(mirrored) != len(expected_ids):
                    assert location.poll() is None, log.read_text()
                    assert time.monotonic() < deadline, log.read_text()
                    with closing(sqlite3.connect(control)) as database:
                        mirrored = database.execute(
                            f"SELECT event_id,record_json FROM {mirror_table} ORDER BY event_id"
                        ).fetchall()
                    record_finished_run(control)
                    time.sleep(0.5)
                assert {json.loads(row)[source_key] for _, row in mirrored} == expected_ids
                for event_id, text in mirrored:
                    record: dict[str, Any] = json.loads(text)
                    assert record == (
                        fixture["record"]
                        | {source_key: record[source_key]}
                        | fixture["record_overrides"].get(record[source_key], {})
                    )
                    assert record[fixture.get("run_key", "run_id")] == test_case.expected_run_id
                    assert identities.setdefault(record[source_key], event_id) == event_id
                with closing(sqlite3.connect(control)) as database:
                    _ = database.execute(mirror_sql)
                    database.commit()
                connection = adapter.connect(
                    {test_case.connection_key: str(project / "warehouse.db")}
                )
                _ = adapter.execute(connection=connection, sql=warehouse_sql)
                adapter.close(connection)
                record_finished_run(control)


@pytest.mark.parametrize(
    "test_case",
    [
        KeyPageTestCase(
            "duckdb audit results",
            "duckdb",
            "database",
            "",
            "audit_result_history",
            append_audit_history,
            key_width=3,
        ),
        KeyPageTestCase(
            "duckdb node results",
            "duckdb",
            "database",
            "external-run",
            "node_result_history",
            append_node_result_history,
            key_width=9,
        ),
        KeyPageTestCase(
            "duckdb consumption",
            "duckdb",
            "database",
            "external-run",
            "consumption_history",
            append_consumption_history,
            key_width=7,
        ),
        KeyPageTestCase(
            "sqlite observations",
            "sqlite",
            "path",
            "external-run",
            "observation_history",
            append_observation_history,
            key_width=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_mirrored_location_when_a_reconcile_scan_runs_then_only_keys_and_missing_rows_are_read(
    test_case: KeyPageTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    fixture: dict[str, Any] = json.loads(
        Path(
            f"benchmarks/duckshop/edge_cases/orchestration/{test_case.fixture_name}.json"
        ).read_text()
    )
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    test_case.append_history(adapter, connection, fixture)
    adapter.close(connection)
    (project / "orchestration/orders.py").write_text("")
    write_files(project_dir=project, files={"models/orders.sql": fixture["model"]})
    table: str = f"_sqlbuild_{fixture['kind']}"
    hook: Path = tmp_path / "read-hook"
    hook.mkdir()
    reads: Path = hook / "reads.jsonl"
    reads.write_text("")
    (hook / "sitecustomize.py").write_text(
        "import json, re\n"
        "from pathlib import Path\n"
        "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n"
        "from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter\n"
        "def logged(original):\n"
        "    def execute(self, *, connection, sql):\n"
        f"        if re.search(r'FROM\\s+\\S*{table}\\b', sql):\n"
        f"            with Path({str(reads)!r}).open('a') as log:\n"
        "                log.write(json.dumps(sql) + '\\n')\n"
        "        return original(self, connection=connection, sql=sql)\n"
        "    return execute\n"
        "for adapter in (DuckDbAdapter, SqliteAdapter):\n"
        "    adapter.execute = logged(adapter.execute)\n"
    )
    mirror_table: str = f"sqb_warehouse_{fixture['kind']}"
    with snapshot_api(tmp_path / "control") as api:
        log: Path = tmp_path / "location.log"
        control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ
            | {
                "SQLBUILD_LOCATION_TOKEN": api.bearer,
                "SQLBUILD_METADATA_RECONCILE_SECONDS": "1",
                "PYTHONPATH": str(hook),
            },
        ) as location:
            _ = wait_for_generation(api, "development", 1, location, log)

            def mirrored() -> list[tuple[str, str]]:
                with closing(sqlite3.connect(control)) as database:
                    rows: list[tuple[str, str]] = database.execute(
                        f"SELECT event_id,record_json FROM {mirror_table} ORDER BY event_id"
                    ).fetchall()
                return rows

            def reconcile() -> list[tuple[bool, int]]:
                """One reconcile scan after a run; each read is (keyset-ordered, projected width)."""
                time.sleep(1.2)
                completed: int = log.read_text().count("synchronization completed")
                reads.write_text("")
                record_finished_run(control)
                deadline: float = time.monotonic() + 45
                while log.read_text().count("synchronization completed") == completed:
                    assert location.poll() is None, log.read_text()
                    assert time.monotonic() < deadline, log.read_text()
                    time.sleep(0.2)
                statements: list[str] = [
                    json.loads(line) for line in reads.read_text().splitlines()
                ]
                return [
                    (
                        " ORDER BY " in sql,
                        len(sql.split(" FROM ")[0].removeprefix("SELECT ").split(", ")),
                    )
                    for sql in statements
                ]

            deadline: float = time.monotonic() + 45
            while log.read_text().count("synchronization completed") == 0 or not mirrored():
                assert location.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.2)
            initial: list[tuple[str, str]] = mirrored()
            assert {json.loads(row)[fixture["source_key"]] for _, row in initial} == {
                f"{fixture['id_prefix']}-{index:03}" for index in range(fixture["source_rows"])
            }

            unchanged: list[tuple[bool, int]] = reconcile()
            assert unchanged, "a reconcile scan reads the warehouse"
            assert set(unchanged) == {(True, test_case.key_width)}, (
                "a mirror that holds every row is reconciled from key pages alone"
            )
            assert mirrored() == initial

            lost: int = 3
            with closing(sqlite3.connect(control)) as database:
                _ = database.execute(
                    f"DELETE FROM {mirror_table} WHERE event_id IN "
                    f"(SELECT event_id FROM {mirror_table} ORDER BY event_id LIMIT {lost})"
                )
                database.commit()
            repaired: list[tuple[bool, int]] = reconcile()
            assert {read for read in repaired if read[0]} == {(True, test_case.key_width)}, (
                "no page is read in full"
            )
            assert 1 <= sum(not ordered for ordered, _ in repaired) <= lost, (
                "only the rows the mirror lost are fetched, by key"
            )
            assert mirrored() == initial, "refetched rows keep their identities and payloads"


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
        CatalogConnectionTestCase("DuckDB retained key set", "duckdb", "database", "retained-run"),
        CatalogConnectionTestCase("SQLite retained key set", "sqlite", "path", "retained-run"),
    ],
    ids=lambda case: case.description,
)
def test_given_external_warehouse_retention_when_a_complete_scan_arrives_then_mirror_deletions_and_empty_resync_match_source(
    test_case: CatalogConnectionTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "warehouse-project"
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    for table in ("_sqlbuild_source_freshness", "_sqlbuild_fingerprints"):
        _ = adapter.execute(connection=connection, sql=f"INSERT INTO {table} SELECT * FROM {table}")
        _ = adapter.execute(
            connection=connection,
            sql=f"UPDATE {table} SET run_id='retained-run' WHERE rowid=(SELECT MAX(rowid) FROM {table})",
        )
    adapter.close(connection)
    declarations: Path = tmp_path / "declarations"
    write_files(project_dir=declarations, files=TYPE_PROOF_FILES)
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=declarations)
    compiled: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    with snapshot_api(tmp_path / "control") as api:
        _ = build_orchestration_snapshot(
            compiled,
            json.dumps(METADATA | {"location_id": api.location_id, "target": "development"}),
            declarations,
            discovered_inputs=discovered,
        ).publish(api.url, api.bearer)
        session_id: str = str(uuid4())
        heartbeat: Request = Request(
            f"{api.url}/api/locations/{api.location_id}/health",
            data=json.dumps(
                {
                    "session_id": session_id,
                    "event_id": str(uuid4()),
                    "target": "development",
                    "health": {"kind": "heartbeat"},
                }
            ).encode(),
            headers={"Authorization": f"Bearer {api.bearer}", "Content-Type": "application/json"},
        )
        with urlopen(heartbeat, timeout=5) as response:
            assert json.load(response) == {"applied": True}
        for kind, mirror in [
            ("source_freshness", "sqb_warehouse_freshness"),
            ("fingerprints", "sqb_warehouse_fingerprints"),
        ]:
            initial: dict[str, Any] = warehouse_history_page(project, kind)
            assert len(initial["records"]) == 2 and initial["next_after"] is None
            batch: dict[str, Any] = {
                "kind": kind,
                "session_id": session_id,
                "generation": 1,
                "records": initial["records"],
            }
            assert sync_warehouse_history(api, batch) == {"applied": True}
            control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
            with closing(sqlite3.connect(control)) as database:
                before: list[tuple[str, str]] = database.execute(
                    f"SELECT event_id,record_json FROM {mirror} ORDER BY event_id"
                ).fetchall()
            assert len(before) == 2
            connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
            _ = adapter.execute(
                connection=connection, sql=f"DELETE FROM _sqlbuild_{kind} WHERE run_id='legacy-run'"
            )
            adapter.close(connection)
            remaining: dict[str, Any] = warehouse_history_page(project, kind)
            assert [row["run_id"] for row in remaining["records"]] == [test_case.expected_run_id]
            batch["records"] = remaining["records"]
            batch["scan"] = {
                "scan_id": str(uuid4()),
                "location": {"database": None, "schema": "main"},
                "complete": False,
            }
            assert sync_warehouse_history(api, batch) == {"applied": False}
            with closing(sqlite3.connect(control)) as database:
                assert database.execute(f"SELECT COUNT(*) FROM {mirror}").fetchone() == (2,)
            batch["records"] = []
            batch["scan"]["complete"] = True
            assert sync_warehouse_history(api, batch) == {"applied": True}
            originals_by_run: dict[str, tuple[str, str]] = {
                json.loads(row[1])["run_id"]: row for row in before
            }
            expected: list[tuple[str, str]] = [originals_by_run[test_case.expected_run_id]]
            with closing(sqlite3.connect(control)) as database:
                assert (
                    database.execute(f"SELECT event_id,record_json FROM {mirror}").fetchall()
                    == expected
                )
                _ = database.execute(f"DELETE FROM {mirror}")
                database.commit()
            assert sync_warehouse_history(
                api,
                {
                    "kind": kind,
                    "session_id": session_id,
                    "generation": 1,
                    "records": remaining["records"],
                },
            ) == {"applied": True}
            with closing(sqlite3.connect(control)) as database:
                assert (
                    database.execute(f"SELECT event_id,record_json FROM {mirror}").fetchall()
                    == expected
                )
            assert warehouse_history_page(project, kind) == remaining, (
                "sync does not mutate warehouse state"
            )
    assert not (project / ".sqlbuild/control.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticHistoryTestCase("DuckDB automatic history", "duckdb", "database", "legacy-run"),
        AutomaticHistoryTestCase("SQLite automatic history", "sqlite", "path", "legacy-run"),
        AutomaticHistoryTestCase(
            "SQLite wide history pages",
            "sqlite",
            "path",
            "legacy-run",
            wide=True,
            initial_seconds=30,
            expected_average_bytes=640_000,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_served_location_when_warehouse_history_changes_then_periodic_read_only_sync_reconciles_without_manual_requests(
    test_case: AutomaticHistoryTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    fixture: dict[str, Any] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/automatic_warehouse_history.json"
        ).read_text()
    )
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    expected_runs: set[str] = {test_case.expected_run_id}
    definition: str = base64.b64encode(
        ("SELECT 17 /*" + "x" * fixture["wide_definition_bytes"] + "*/").encode()
    ).decode()
    for index in range(fixture["wide_fingerprint_count"] * test_case.wide):
        expected_runs.add(f"automatic-{index}")
        _ = adapter.execute(
            connection=connection,
            sql=(
                "INSERT INTO _sqlbuild_fingerprints SELECT node_type,"
                f"'orders-{index}',target_database,target_schema,target_name,'automatic-{index}',"
                "definition_hash,version_hash,schema_fingerprint,"
                f"'{definition}',metadata_json_b64,ts FROM _sqlbuild_fingerprints WHERE run_id='legacy-run'"
            ),
        )
    assert adapter.query(
        connection=connection,
        sql="SELECT SUM(LENGTH(definition_b64))*3/4 FROM _sqlbuild_fingerprints",
        limit=1,
    ).rows[0][0] >= test_case.expected_average_bytes * len(expected_runs), (
        "wide fixture exceeds one API request before any synchronization"
    )
    adapter.close(connection)
    (project / "orchestration/orders.py").write_text("")
    write_files(
        project_dir=project,
        files={"models/orders.sql": fixture["model"]},
    )
    with snapshot_api(tmp_path / "control") as api:
        log: Path = tmp_path / "location.log"
        control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ
            | {"SQLBUILD_LOCATION_TOKEN": api.bearer, "SQLBUILD_METADATA_RECONCILE_SECONDS": "30"},
        ) as location:
            _ = wait_for_generation(api, "development", 1, location, log)
            initial_tables: dict[str, set[str]] = {
                "sqb_warehouse_fingerprints": expected_runs,
                "sqb_warehouse_freshness": {test_case.expected_run_id},
            }
            for expected_tables, seconds, mirror_sql, warehouse_sql in [
                (
                    initial_tables,
                    test_case.initial_seconds,
                    "DELETE FROM {table}",
                    "SELECT 1",
                ),
                (initial_tables, 45, "SELECT 1", "DELETE FROM {table}"),
                ({table: set() for table in initial_tables}, 45, "SELECT 1", "SELECT 1"),
            ]:
                deadline: float = time.monotonic() + seconds
                observed: tuple[int, int] = (-1, -1)
                expected: tuple[int, int] = (
                    len(expected_tables["sqb_warehouse_fingerprints"]),
                    len(expected_tables["sqb_warehouse_freshness"]),
                )
                while observed != expected:
                    assert location.poll() is None, log.read_text()
                    assert time.monotonic() < deadline, log.read_text()
                    with closing(sqlite3.connect(control)) as database:
                        observed = database.execute(
                            "SELECT (SELECT COUNT(*) FROM sqb_warehouse_fingerprints),"
                            "(SELECT COUNT(*) FROM sqb_warehouse_freshness)"
                        ).fetchone()
                    record_finished_run(control)
                    time.sleep(0.5)
                with closing(sqlite3.connect(control)) as database:
                    bodies: list[tuple[bytes]] = database.execute(
                        "SELECT t.body FROM sqb_warehouse_fingerprints f "
                        "JOIN sqb_source_texts t ON t.text_hash=f.definition_text_hash"
                    ).fetchall()
                    assert sum(len(gzip.decompress(body)) for (body,) in bodies) >= (
                        test_case.expected_average_bytes
                        * len(expected_tables["sqb_warehouse_fingerprints"])
                    )
                    for table, expected_runs in expected_tables.items():
                        rows: list[tuple[str]] = database.execute(
                            f"SELECT record_json FROM {table}"
                        ).fetchall()
                        runs: set[str] = {json.loads(row[0])["run_id"] for row in rows}
                        assert runs == expected_runs
                        _ = database.execute(mirror_sql.format(table=table))
                    database.commit()
                connection = adapter.connect(
                    {test_case.connection_key: str(project / "warehouse.db")}
                )
                for table in ("_sqlbuild_fingerprints", "_sqlbuild_source_freshness"):
                    _ = adapter.execute(
                        connection=connection, sql=warehouse_sql.format(table=table)
                    )
                adapter.close(connection)
                record_finished_run(control)


@pytest.mark.parametrize(
    "test_case",
    [
        AutomaticHistoryTestCase("DuckDB incremental overlap", "duckdb", "database", "legacy-run"),
        AutomaticHistoryTestCase("SQLite incremental overlap", "sqlite", "path", "legacy-run"),
    ],
    ids=lambda case: case.description,
)
def test_given_durable_mirror_when_late_rows_arrive_then_incremental_overlap_preserves_history_and_empty_resyncs(
    test_case: AutomaticHistoryTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    fixture: dict[str, Any] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/incremental_warehouse_history.json"
        ).read_text()
    )
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    append_legacy_history(adapter, connection, fixture["initial"], fixture["insert_sql"])
    adapter.close(connection)
    (project / "orchestration/orders.py").write_text("")
    write_files(
        project_dir=project,
        files={"models/orders.sql": "MODEL (description 'Orders.'); SELECT 17 AS amount"},
    )
    with snapshot_api(tmp_path / "control") as api:
        control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
        log: Path = tmp_path / "location.log"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ
            | {"SQLBUILD_LOCATION_TOKEN": api.bearer, "SQLBUILD_METADATA_RECONCILE_SECONDS": "300"},
        ) as location:
            _ = wait_for_generation(api, "development", 1, location, log)
            observed_health_at: datetime = datetime(1970, 1, 1, tzinfo=UTC)
            for stage in fixture["stages"]:
                connection = adapter.connect(
                    {test_case.connection_key: str(project / "warehouse.db")}
                )
                arrivals: list[dict[str, str]] = stage["arrivals"] + [
                    {"run_id": f"middle-{index:03}", "observed_at": "2026-02-01 11:58:00"}
                    for index in range(stage["overlap_count"])
                ]
                append_legacy_history(adapter, connection, arrivals, fixture["insert_sql"])
                adapter.close(connection)
                with closing(sqlite3.connect(control)) as database:
                    for table in ("sqb_warehouse_fingerprints", "sqb_warehouse_freshness"):
                        _ = database.execute(stage["mirror_sql"].format(table=table))
                    database.commit()
                record_finished_run(control)
                expected: set[str] = set(stage["expected_runs"]) | {
                    f"middle-{index:03}"
                    for index in range(stage.get("expected_overlap_count", stage["overlap_count"]))
                }
                deadline: float = time.monotonic() + 45
                remaining: bool = True
                while remaining:
                    assert location.poll() is None, log.read_text()
                    assert time.monotonic() < deadline, log.read_text()
                    remaining = False
                    with closing(sqlite3.connect(control)) as database:
                        for table in ("sqb_warehouse_fingerprints", "sqb_warehouse_freshness"):
                            observed: set[str] = {
                                json.loads(record)["run_id"]
                                for (record,) in database.execute(
                                    f"SELECT record_json FROM {table}"
                                )
                            }
                            assert observed <= expected, (table, stage, observed)
                            remaining |= observed != expected
                    remaining |= (
                        log.read_text().count("Warehouse history synchronization completed.")
                        < stage["completed_cycles"]
                    )
                    time.sleep(0.05)
                assert test_case.expected_run_id in observed
                reports: list[dict[str, Any]] = wait_for_warehouse_sync_health(
                    api, fixture["health"], location, log, after=observed_health_at
                )
                sent_counts: dict[str, int] = dict.fromkeys(fixture["health"]["tables"], 0)
                for report in reports:
                    sent_counts[report["report"]["health"]["table"]] += report["report"]["health"][
                        "records_sent"
                    ]
                assert sent_counts == dict.fromkeys(
                    fixture["health"]["tables"], stage["health_records_per_table"]
                )
                observed_health_at = max(
                    datetime.fromisoformat(report["received_at"]) for report in reports
                )
            time.sleep(6)
            completed: int = log.read_text().count("Warehouse history synchronization completed.")
            time.sleep(12)
            assert (
                log.read_text().count("Warehouse history synchronization completed.") == completed
            ), "an idle location read the warehouse without a run ending"
            work: dict[str, Any] = wait_for_location_work(
                api, request_warehouse_sync(control), location, log
            )
            assert work["result_json"]["outcome"] == {"kind": "sync_started"}
            deadline = time.monotonic() + 45
            while (
                log.read_text().count("Warehouse history synchronization completed.") == completed
            ):
                assert location.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.05)
            lock: Any = None
            for availability in fixture["availability"]:
                if test_case.adapter_name not in availability.get(
                    "adapters", [test_case.adapter_name]
                ):
                    continue
                if lock is not None:
                    lock.close()
                    lock = None
                if "source" in availability:
                    _ = (project / availability["source"]).rename(
                        project / availability["destination"]
                    )
                if availability.get("hold_write_lock"):
                    lock = hold_write_lock(project / "warehouse.db")
                record_finished_run(control)
                reports = wait_for_warehouse_sync_health(
                    api, availability, location, log, after=observed_health_at
                )
                observed_health_at = max(
                    datetime.fromisoformat(report["received_at"]) for report in reports
                )
                assert (project / "warehouse.db").exists() == availability["available"]
            if test_case.adapter_name == "duckdb":
                assert log.read_text().count("Warehouse file is in use by another process") == 1


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


@pytest.mark.parametrize(
    "test_case",
    [AutomaticHistoryTestCase("DuckDB change tokens", "duckdb", "database", "legacy-run")],
    ids=lambda case: case.description,
)
def test_given_change_tokens_when_a_served_location_resyncs_then_only_changed_or_lost_tables_are_read(
    test_case: AutomaticHistoryTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    adapter.close(connection)
    (project / "orchestration/orders.py").write_text("")
    fixture: dict[str, Any] = json.loads(
        Path(
            "benchmarks/duckshop/edge_cases/orchestration/automatic_warehouse_history.json"
        ).read_text()
    )
    write_files(project_dir=project, files={"models/orders.sql": fixture["model"]})
    hook: Path = tmp_path / "token-hook"
    hook.mkdir()
    tokens: Path = hook / "tokens.json"
    reads: Path = hook / "reads.log"
    sessions: Path = hook / "sessions.log"
    tokens.write_text(
        json.dumps({"_sqlbuild_fingerprints": "f1", "_sqlbuild_source_freshness": "s1"})
    )
    (hook / "sitecustomize.py").write_text(
        "import json, os, re\n"
        "from pathlib import Path\n"
        "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n"
        "def log_session(event):\n"
        f"    with Path({str(sessions)!r}).open('a') as log:\n"
        "        log.write(f'{event} {os.getpid()}\\n')\n"
        "original_connect, original_close = DuckDbAdapter.connect, DuckDbAdapter.close\n"
        "def connect(self, config):\n"
        "    log_session('connect')\n"
        "    return original_connect(self, config)\n"
        "def close(self, connection):\n"
        "    log_session('close')\n"
        "    return original_close(self, connection)\n"
        "DuckDbAdapter.connect, DuckDbAdapter.close = connect, close\n"
        "def read_relation_change_token(self, *, connection, database, schema, name):\n"
        f"    return json.loads(Path({str(tokens)!r}).read_text()).get(name)\n"
        "DuckDbAdapter.read_relation_change_token = read_relation_change_token\n"
        "original_execute = DuckDbAdapter.execute\n"
        "def execute(self, *, connection, sql):\n"
        "    read = re.search(r'FROM\\s+\\S*(_sqlbuild_\\w+)', sql)\n"
        "    if read:\n"
        f"        with Path({str(reads)!r}).open('a') as log:\n"
        "            log.write(read.group(1) + '\\n')\n"
        "    return original_execute(self, connection=connection, sql=sql)\n"
        "DuckDbAdapter.execute = execute\n"
    )
    mirrors: tuple[str, str] = ("sqb_warehouse_fingerprints", "sqb_warehouse_freshness")
    with snapshot_api(tmp_path / "control") as api:
        log: Path = tmp_path / "location.log"
        control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
        with location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ
            | {
                "SQLBUILD_LOCATION_TOKEN": api.bearer,
                "SQLBUILD_METADATA_RECONCILE_SECONDS": "3600",
                "PYTHONPATH": str(hook),
            },
        ) as location:
            _ = wait_for_generation(api, "development", 1, location, log)

            def mirrored() -> tuple[int, int]:
                with closing(sqlite3.connect(control)) as database:
                    counts: tuple[int, int] = database.execute(
                        f"SELECT (SELECT COUNT(*) FROM {mirrors[0]}),"
                        f"(SELECT COUNT(*) FROM {mirrors[1]})"
                    ).fetchone()
                return counts

            def resync() -> list[str]:
                """Start one synchronization after a finished run and return the tables it read."""
                completed: int = log.read_text().count("synchronization completed")
                reads.write_text("")
                sessions.write_text("")
                record_finished_run(control)
                deadline: float = time.monotonic() + 45
                while log.read_text().count("synchronization completed") == completed:
                    assert location.poll() is None, log.read_text()
                    assert time.monotonic() < deadline, log.read_text()
                    time.sleep(0.2)
                deadline = time.monotonic() + 10
                while sessions.read_text().count("close") == 0:
                    assert time.monotonic() < deadline, sessions.read_text()
                    time.sleep(0.1)
                events: list[list[str]] = [
                    line.split() for line in sessions.read_text().splitlines()
                ]
                assert len({pid for _, pid in events}) == 1, (
                    "one warm interpreter reads every page of a synchronization"
                )
                assert [event for event, _ in events] == ["connect", "close"] * (
                    len(events) // 2
                ), "a local database file is never held open between pages, so writers proceed"
                return sorted(set(reads.read_text().split()))

            deadline: float = time.monotonic() + 45
            while log.read_text().count("synchronization completed") == 0 or 0 in mirrored():
                assert location.poll() is None, log.read_text()
                assert time.monotonic() < deadline, log.read_text()
                time.sleep(0.2)
            initial: tuple[int, int] = mirrored()

            assert resync() == [], "unchanged tokens read nothing"
            assert mirrored() == initial

            changed_rows: int = 250
            connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
            _ = adapter.execute(
                connection=connection,
                sql=(
                    "INSERT INTO _sqlbuild_fingerprints SELECT node_type,"
                    "'orders-changed-' || page.i,target_database,target_schema,target_name,"
                    "'changed-run',definition_hash,version_hash,schema_fingerprint,"
                    "definition_b64,metadata_json_b64,ts FROM _sqlbuild_fingerprints "
                    f"CROSS JOIN range({changed_rows}) AS page(i) "
                    f"WHERE run_id='{test_case.expected_run_id}'"
                ),
            )
            adapter.close(connection)
            tokens.write_text(
                json.dumps({"_sqlbuild_fingerprints": "f2", "_sqlbuild_source_freshness": "s1"})
            )
            assert resync() == ["_sqlbuild_fingerprints"], (
                "only the changed table is read, across several pages"
            )
            assert mirrored() == (initial[0] + changed_rows, initial[1])

            with closing(sqlite3.connect(control)) as database:
                _ = database.execute(f"DELETE FROM {mirrors[1]}")
                database.commit()
            assert resync() == ["_sqlbuild_source_freshness"], "a lost mirror is read again"
            assert mirrored() == (initial[0] + changed_rows, initial[1])
