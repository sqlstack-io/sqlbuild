"""Helpers for janitor e2e tests."""

from __future__ import annotations

import json
import re
import subprocess
from collections.abc import Mapping
from datetime import UTC, datetime
from itertools import product
from pathlib import Path
from textwrap import dedent
from types import MappingProxyType
from typing import Any

from sqlbuild._native import (
    Fingerprint,
    SourceFreshnessRecord,
    SourceFreshnessRenderers,
    write_fingerprint,
    write_source_freshness_records,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.audit_results.main._write import write_audit_result_records
from sqlbuild.executor.audit_results.models import AuditResultRecord
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


def prepare_janitor_project(
    *,
    tmp_path: Path,
    project_name: str,
    janitor_config: str = "",
    settings_config: str = "",
) -> Path:
    """Create a minimal DuckDB project for janitor e2e tests."""

    normalized_settings_config: str = dedent(settings_config).strip()
    normalized_janitor_config: str = dedent(janitor_config).strip()
    settings_block: str = {False: "", True: f"\n[settings]\n{normalized_settings_config}\n"}[
        bool(normalized_settings_config)
    ]
    janitor_block: str = {False: "", True: f"\n[janitor]\n{normalized_janitor_config}\n"}[
        bool(normalized_janitor_config)
    ]
    project_config: str = (
        f'name = "{project_name}"\n'
        'adapter = "duckdb"\n\n'
        "[connection]\n"
        'database = "janitor.duckdb"\n'
        f"{settings_block}"
        f"{janitor_block}"
        "\n[defaults]\n"
        'materialized = "table"\n'
    )
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": project_config,
            "models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.");

                SELECT 1 AS order_id
                """
            ).strip()
            + "\n",
        },
    )


def create_janitor_demo_relations(*, db_path: Path) -> None:
    """Create tracked, untracked, and excluded stale relations."""

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    try:
        connection.execute("CREATE TABLE janitor_tracked_extra AS SELECT 1 AS id")
        connection.execute("CREATE TABLE janitor_untracked_extra AS SELECT 1 AS id")
        connection.execute("CREATE TABLE partition_state AS SELECT 1 AS id")
        connection.execute("CREATE TABLE _sqlbuild_microbatches AS SELECT 1 AS id")
        _ = write_fingerprint(
            connection=connection,
            execute=lambda *, connection, sql: connection.execute(sql),
            database=None,
            schema="main",
            fingerprint=Fingerprint(
                node_type="model",
                node_name="janitor_tracked_extra",
                target_database=None,
                target_schema="main",
                target_name="janitor_tracked_extra",
                run_id="run_janitor_e2e",
                definition_hash="definition_hash",
                schema_fingerprint="schema_hash",
                definition="SELECT 1 AS id",
                ts=datetime(2026, 1, 15, 12, 0, 0),
            ),
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            render_framework_type=DuckDbAdapter().render_framework_type,
        )
        _ = write_fingerprint(
            connection=connection,
            execute=lambda *, connection, sql: connection.execute(sql),
            database=None,
            schema="main",
            fingerprint=Fingerprint(
                node_type="model",
                node_name="microbatch_state_probe",
                target_database=None,
                target_schema="main",
                target_name="_sqlbuild_microbatches",
                run_id="run_janitor_microbatch_e2e",
                definition_hash="microbatch_definition_hash",
                schema_fingerprint="microbatch_schema_hash",
                definition="SELECT 1 AS id",
                ts=datetime(2026, 1, 15, 12, 0, 0),
            ),
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            render_framework_type=DuckDbAdapter().render_framework_type,
        )
    finally:
        connection.close()


def create_janitor_scenario_relations(*, db_path: Path) -> None:
    """Create strict scenario artifacts and a similarly named non-artifact relation."""

    import duckdb

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    try:
        connection.execute("CREATE TABLE __sqb_a13f09c2e7b8__source__raw_orders AS SELECT 1 AS id")
        connection.execute(
            "CREATE TABLE __sqb_a13f09c2e7b8__model__daily_revenue AS SELECT 1 AS id"
        )
        connection.execute("CREATE TABLE __sqb_a13f09c2e7b__model__daily_revenue AS SELECT 1 AS id")
    finally:
        connection.close()


DIRECT_STATE_PRUNING: dict[str, Any] = json.loads(
    Path("benchmarks/duckshop/edge_cases/orchestration/direct_state_pruning.json").read_text()
)


def audit_record(*, binding_key: str, run_id: str, occurred_at: datetime) -> AuditResultRecord:
    """One passing final audit result for `binding_key` in `run_id`."""

    audit_name: str = binding_key.split(".")[-1]
    return AuditResultRecord(
        result_id=f"{binding_key}:{run_id}",
        schema_version=1,
        occurred_at=occurred_at,
        invocation_id=f"invocation_{run_id}",
        run_id=run_id,
        audit_name=audit_name,
        audit_definition_name=audit_name,
        binding_key=binding_key,
        definition_fingerprint="definition-fingerprint",
        execution_fingerprint=f"execution_{run_id}",
        evaluation_mode="violations",
        run_scope_phase="final",
        attachment_kind="model",
        attached_target_kind="model",
        attached_target_name="orders",
        attached_column_name=None,
        target_database=None,
        target_schema="main",
        target_name="orders",
        severity="error",
        outcome="pass",
        execution_error=None,
        violation_count=0,
        measured_value=None,
        sample_count=None,
        sample_unit=None,
        minimum_samples=None,
        thresholds_json=None,
        evidence_json=None,
        evidence_count=None,
        evidence_truncated=None,
        evidence_error=None,
        measurement_sql=None,
        evidence_sql=None,
        executed_sql="SELECT 1 WHERE FALSE",
        sql_digest="digest",
        metadata_json=None,
        reused=False,
    )


def create_direct_state_history(*, db_path: Path, schema: str = "main") -> None:
    import duckdb

    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(db_path))
    try:
        run_id: str
        observed_hour: int
        for run_id, observed_hour in DIRECT_STATE_PRUNING["runs"]:
            _ = write_fingerprint(
                connection=connection,
                execute=lambda *, connection, sql: connection.execute(sql),
                database=None,
                schema=schema,
                fingerprint=Fingerprint(
                    node_type="model",
                    node_name="janitor_state_probe",
                    target_database=None,
                    target_schema=schema,
                    target_name="janitor_state_probe",
                    run_id=run_id,
                    definition_hash=f"definition_{run_id}",
                    version_hash=f"version_{run_id}",
                    schema_fingerprint=f"schema_{run_id}",
                    definition=f"SELECT '{run_id}'",
                    ts=datetime(2026, 1, 15, observed_hour, 0, 0),
                ),
                render_qualified_name=adapter.render_qualified_name,
                render_framework_type=adapter.render_framework_type,
            )
            write_audit_result_records(
                connection=connection,
                execute=lambda *, connection, sql: connection.execute(sql),
                database=None,
                schema=schema,
                records=tuple(
                    audit_record(
                        binding_key=binding_key,
                        run_id=run_id,
                        occurred_at=datetime(2026, 1, 15, observed_hour, 10, 0, tzinfo=UTC),
                    )
                    for binding_key in DIRECT_STATE_PRUNING["audit_binding_keys"]
                ),
                render_qualified_name=adapter.render_qualified_name,
                render_framework_type=adapter.render_framework_type,
            )
            write_source_freshness_records(
                connection=connection,
                execute=lambda *, connection, sql: connection.execute(sql),
                database=None,
                schema=schema,
                records=(
                    SourceFreshnessRecord(
                        source_name="raw.janitor_state_probe",
                        target_database=None,
                        target_schema=None,
                        target_name=None,
                        run_id=run_id,
                        strategy="adapter_metadata",
                        value_kind="timestamp",
                        data_version=f"2026-01-15T{observed_hour:02d}:00:00",
                        data_version_hash=f"source_{run_id}",
                        observed_at=datetime(2026, 1, 15, observed_hour, 5, 0, tzinfo=UTC),
                    ),
                ),
                renderers=SourceFreshnessRenderers(
                    render_qualified_name=adapter.render_qualified_name,
                    render_framework_type=adapter.render_framework_type,
                    render_insert_records_sql=adapter.render_insert_source_freshness_records_sql,
                ),
            )
    finally:
        connection.close()


AGED_JANITOR_ADAPTER_NAME: str = "aged_janitor_duckdb"
JANITOR_TEST_SETTINGS_FILENAME: str = "janitor_test_settings.json"
AGED_JANITOR_ADAPTER_SOURCE: str = """
import json
from dataclasses import replace
from datetime import datetime
from pathlib import Path
from typing import Any

from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter

_SETTINGS_PATH = Path(__file__).resolve().parent.parent / "janitor_test_settings.json"


def _settings() -> dict[str, Any]:
    if not _SETTINGS_PATH.exists():
        return {}
    return json.loads(_SETTINGS_PATH.read_text(encoding="utf-8"))


class AgedJanitorDuckDbAdapter(DuckDbAdapter):
    adapter_name = "aged_janitor_duckdb"

    def supports_relation_age_metadata(self) -> bool:
        return True

    def maximum_identifier_length(self) -> int:
        return int(_settings().get("identifier_limit") or super().maximum_identifier_length())

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        created_at: dict[str, str] = _settings().get("created_at", {})
        return tuple(
            replace(relation, created_at=datetime.fromisoformat(created_at[relation.name]))
            if relation.name in created_at
            else relation
            for relation in super().list_relations(
                connection=connection, database=database, schemas=schemas, names=names
            )
        )

    def _execute(self, *, connection: Any, sql: str) -> Any:
        if (
            _settings().get("fail_janitor_event_insert")
            and sql.startswith("INSERT INTO")
            and "_sqlbuild_janitor_events" in sql
        ):
            raise AdapterUserError("simulated janitor event write failure")
        return super()._execute(connection=connection, sql=sql)
"""


def prepare_archive_janitor_project(
    *,
    tmp_path: Path,
    project_name: str,
    janitor_config: str,
    model_names: tuple[str, ...] = ("orders",),
    use_aged_adapter: bool = False,
) -> Path:
    """Create a DuckDB project whose janitor archives stale relations."""

    repo_files: dict[str, str] = {
        "sqlbuild_project.toml": (
            f'name = "{project_name}"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            f'database = "{(tmp_path / project_name / "janitor.duckdb").as_posix()}"\n\n'
            f"[janitor]\n{dedent(janitor_config).strip()}\n\n"
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        **{
            f"models/{model_name}.sql": f"MODEL (description 'Test model.');\n\n"
            f"SELECT 1 AS {model_name}_id\n"
            for model_name in model_names
        },
        f"adapters/{AGED_JANITOR_ADAPTER_NAME}.py": AGED_JANITOR_ADAPTER_SOURCE,
        **{
            False: {},
            True: {"sqlbuild_local.toml": f'adapter = "{AGED_JANITOR_ADAPTER_NAME}"\n'},
        }[use_aged_adapter],
    }
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files=repo_files,
    )


def write_janitor_test_settings(
    *,
    project_dir: Path,
    created_at: Mapping[str, datetime] = MappingProxyType({}),
    identifier_limit: int = 255,
    fail_janitor_event_insert: bool = False,
) -> None:
    """Write relation ages and fault switches consumed by the aged janitor adapter."""

    import json

    payload: dict[str, object] = {
        "created_at": {name: value.isoformat() for name, value in created_at.items()},
        "identifier_limit": identifier_limit,
        "fail_janitor_event_insert": fail_janitor_event_insert,
    }
    (project_dir / JANITOR_TEST_SETTINGS_FILENAME).write_text(json.dumps(payload), encoding="utf-8")


def archive_timestamp_text(value: datetime) -> str:
    """Render the UTC timestamp component embedded in janitor archive names."""

    return value.astimezone(UTC).strftime("%Y%m%dT%H%M%SZ").lower()


def list_archive_names(*, db_path: Path, schema: str = "main") -> tuple[str, ...]:
    """Return archive-prefixed relation names in one schema."""

    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT table_name FROM information_schema.tables "
            f"WHERE table_schema = '{schema}' AND starts_with(table_name, '_sqb_archive__') "
            "ORDER BY table_name"
        ),
    )
    return tuple(str(row[0]) for row in rows)


def read_janitor_events(*, db_path: Path) -> list[tuple[object, ...]]:
    """Return janitor audit rows in a stable order."""

    return query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT event_type, original_name, original_qualified_name, archive_name, run_id "
            "FROM main._sqlbuild_janitor_events ORDER BY occurred_at, event_type, archive_name"
        ),
    )


def read_current_janitor_events(*, db_path: Path, run_id: str) -> list[tuple[object, ...]]:
    """Return janitor audit rows excluding one seeded historical run."""

    return query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT event_type, original_name, original_qualified_name, archive_name, run_id "
            f"FROM main._sqlbuild_janitor_events WHERE run_id <> '{run_id}' "
            "ORDER BY archive_name"
        ),
    )


def prepare_two_target_janitor_project(
    *, tmp_path: Path, project_name: str, model_names: tuple[str, ...]
) -> Path:
    """Create a DuckDB janitor project with `dev` (default) and `prod` targets in two schemas."""

    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": (
                f'name = "{project_name}"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[connection]\n"
                f'database = "{(tmp_path / project_name / "janitor.duckdb").as_posix()}"\n\n'
                "[targets.dev]\n"
                'schema = "dev"\n\n'
                "[targets.prod]\n"
                'schema = "prod"\n\n'
                "[janitor]\n"
                "enabled = true\n"
                "retention_days = 0\n"
                "archive_retention_days = 14\n"
                "direct_state_history_versions = 2\n\n"
                "[defaults]\n"
                'materialized = "table"\n'
            ),
            **{
                f"models/{model_name}.sql": f"MODEL (description 'Test model.');\n\n"
                f"SELECT 1 AS {model_name}_id\n"
                for model_name in model_names
            },
        },
    )


def snapshot_schema_contents(
    *, db_path: Path, schemas: tuple[str, ...]
) -> tuple[tuple[str, str, str, tuple[tuple[object, ...], ...]], ...]:
    """Return every relation in the schemas with its type and complete sorted contents."""

    schema_list: str = ", ".join(f"'{schema}'" for schema in schemas)
    relations: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT table_schema, table_name, table_type FROM information_schema.tables "
            f"WHERE table_schema IN ({schema_list}) ORDER BY 1, 2"
        ),
    )
    snapshot: list[tuple[str, str, str, tuple[tuple[object, ...], ...]]] = []
    for schema, name, table_type in relations:
        rows: list[tuple[object, ...]] = query_duckdb(
            db_path=db_path, sql=f'SELECT * FROM "{schema}"."{name}" ORDER BY ALL'
        )
        snapshot.append((str(schema), str(name), str(table_type), tuple(rows)))
    return tuple(snapshot)


EXPIRED_ARCHIVE_NAME: str = "_sqb_archive__20200101t000000z__old_products"


def prepare_cruft_in_both_targets(*, tmp_path: Path, project_name: str) -> Path:
    """Build both targets, retire one model, and seed an expired archive and state history."""

    project_dir: Path = prepare_two_target_janitor_project(
        tmp_path=tmp_path, project_name=project_name, model_names=("orders", "customers")
    )
    for build_command in (("--no-color", "build"), ("--no-color", "build", "--target", "prod")):
        build_result: subprocess.CompletedProcess[str] = run_sqb(
            command=build_command, project_dir=project_dir
        )
        assert build_result.returncode == 0, build_result.stdout + build_result.stderr
    (project_dir / "models" / "customers.sql").unlink()
    db_path: Path = project_dir / "janitor.duckdb"
    for schema in ("dev", "prod"):
        execute_duckdb(
            db_path=db_path,
            sql=f'CREATE TABLE {schema}."{EXPIRED_ARCHIVE_NAME}" AS SELECT 1 AS product_id',
        )
        create_direct_state_history(db_path=db_path, schema=schema)
    return project_dir


def probe_fingerprint_count(*, db_path: Path, schema: str) -> int:
    """Return how many seeded probe fingerprint rows remain in one schema."""

    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            f"SELECT COUNT(*) FROM {schema}._sqlbuild_fingerprints "
            "WHERE node_name = 'janitor_state_probe'"
        ),
    )
    return int(str(rows[0][0]))


PROBE_JANITOR_ADAPTER_NAME: str = "probe_janitor_duckdb"
PROBE_JANITOR_SETTINGS_FILENAME: str = "janitor_probe_settings.json"
PROBE_STATEMENT_LOG_FILENAME: str = "janitor_statements.log"
PROBE_JANITOR_ADAPTER_SOURCE: str = """
import json
import threading
from pathlib import Path
from typing import Any

from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter

_ROOT = Path(__file__).resolve().parent.parent
_LOG_LOCK = threading.Lock()


def _settings() -> dict[str, Any]:
    path = _ROOT / "janitor_probe_settings.json"
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


class ProbeJanitorDuckDbAdapter(DuckDbAdapter):
    adapter_name = "probe_janitor_duckdb"

    @property
    def concurrent_ddl_limit(self) -> int:
        return int(_settings().get("concurrent_ddl_limit", 1))

    def maximum_identifier_length(self) -> int:
        return int(_settings().get("identifier_limit") or super().maximum_identifier_length())

    def _execute(self, *, connection: Any, sql: str) -> Any:
        settings = _settings()
        failing = settings.get("fail_rename")
        if failing and sql.startswith("ALTER TABLE") and f".{failing} RENAME" in sql:
            raise AdapterUserError(f"simulated rename failure for {failing}")
        if settings.get("log"):
            with _LOG_LOCK, (_ROOT / "janitor_statements.log").open("a") as handle:
                handle.write(" ".join(sql.split()) + "\\n")
        return super()._execute(connection=connection, sql=sql)
"""


ARCHIVE_ITEM_LINE: re.Pattern[str] = re.compile(
    r"^\s+(?P<index>\d+)/(?P<total>\d+)  archived  ", re.MULTILINE
)


def _probe_model_sql(*, schema: str, migrate_from: str) -> str:
    migration: str = {"": ""}.get(migrate_from, f', migrate_from "{migrate_from}"')
    return f"MODEL (description 'Test model.', schema {schema}{migration});\n\nSELECT 1 AS id\n"


def prepare_probe_janitor_project(
    *,
    tmp_path: Path,
    project_name: str,
    stale_tables: Mapping[str, int],
    renamed_models: tuple[str, ...] = (),
    concurrency: int = 1,
) -> Path:
    """Build a DuckDB project with stale tables and an expired archive in each schema."""

    models: dict[str, str] = {
        f"models/{schema}_{name}.sql": _probe_model_sql(schema=schema, migrate_from="")
        for schema, name in product(stale_tables, ("orders", *renamed_models))
    }
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": (
                f'name = "{project_name}"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                f'database = "{(tmp_path / project_name / "janitor.duckdb").as_posix()}"\n\n'
                f"[settings]\nconcurrency = {concurrency}\n\n"
                "[janitor]\n"
                "enabled = true\n"
                "retention_days = 0\n"
                "archive_retention_days = 14\n"
                "delete_tracked_only = false\n"
                "direct_state_history_versions = 2\n\n"
                "[defaults]\n"
                'materialized = "table"\n'
            ),
            f"adapters/{PROBE_JANITOR_ADAPTER_NAME}.py": PROBE_JANITOR_ADAPTER_SOURCE,
            **models,
        },
    )
    _build(project_dir)
    db_path: Path = project_dir / "janitor.duckdb"
    schema: str
    count: int
    for schema, count in stale_tables.items():
        tables: str = "; ".join(
            f"CREATE TABLE {schema}.old_orders_{index:03d} AS SELECT {index} AS id"
            for index in range(count)
        )
        execute_duckdb(
            db_path=db_path,
            sql=f'{tables}; CREATE TABLE {schema}."{EXPIRED_ARCHIVE_NAME}" AS SELECT 1 AS id',
        )
    return project_dir


def rename_probe_models(
    *, project_dir: Path, schemas: tuple[str, ...], renamed_models: tuple[str, ...]
) -> None:
    """Rename each model with `migrate_from` and rebuild, leaving compatibility views behind."""

    schema: str
    name: str
    for schema, name in product(schemas, renamed_models):
        (project_dir / "models" / f"{schema}_{name}.sql").unlink()
        (project_dir / "models" / f"{schema}_{name}_v2.sql").write_text(
            _probe_model_sql(schema=schema, migrate_from=f"{schema}.{schema}_{name}"),
            encoding="utf-8",
        )
    _build(project_dir)


def _build(project_dir: Path) -> None:
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert result.returncode == 0, result.stdout + result.stderr


def write_probe_settings(
    *,
    project_dir: Path,
    concurrent_ddl_limit: int = 1,
    fail_rename: str | None = None,
    identifier_limit: int | None = None,
) -> None:
    """Run janitor on the probe adapter with statement logging and these fault switches."""

    (project_dir / "sqlbuild_local.toml").write_text(
        f'adapter = "{PROBE_JANITOR_ADAPTER_NAME}"\n', encoding="utf-8"
    )
    (project_dir / PROBE_STATEMENT_LOG_FILENAME).write_text("", encoding="utf-8")
    (project_dir / PROBE_JANITOR_SETTINGS_FILENAME).write_text(
        json.dumps(
            {
                "log": True,
                "concurrent_ddl_limit": concurrent_ddl_limit,
                "fail_rename": fail_rename,
                "identifier_limit": identifier_limit,
            }
        ),
        encoding="utf-8",
    )


def read_probe_statements(*, project_dir: Path) -> tuple[str, ...]:
    """Return every statement the probe adapter logged, in order."""

    return tuple(
        (project_dir / PROBE_STATEMENT_LOG_FILENAME).read_text(encoding="utf-8").splitlines()
    )


def count_statements(
    *, statements: tuple[str, ...], fragments: tuple[str, ...], excluded: tuple[str, ...] = ()
) -> int:
    """Count statements containing every fragment and none of the excluded fragments."""

    return len(
        tuple(
            filter(
                lambda statement: (
                    all(fragment in statement for fragment in fragments)
                    and not any(fragment in statement for fragment in excluded)
                ),
                statements,
            )
        )
    )


def statement_index(*, statements: tuple[str, ...], prefix: str) -> int:
    """The position of the first statement starting with `prefix`."""

    return [statement.startswith(prefix) for statement in statements].index(True)


def archive_line_positions(output: str) -> tuple[tuple[int, int], ...]:
    """The `n/N` counter of every archived action line, in printed order."""

    return tuple(
        (int(match["index"]), int(match["total"])) for match in ARCHIVE_ITEM_LINE.finditer(output)
    )


def assert_in_order(*, output: str, lines: tuple[str, ...]) -> None:
    """Assert each line appears in `output` after the previous one."""

    position: int = 0
    line: str
    for line in lines:
        found: int = output.find(line, position)
        assert found >= 0, f"{line!r} missing after position {position} in:\n{output}"
        position = found + len(line)


def janitor_end_state(*, db_path: Path, schemas: tuple[str, ...]) -> tuple[tuple[object, ...], ...]:
    """Every relation and every audit event in `schemas`, with archive timestamps masked."""

    relations: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT table_schema, regexp_replace(table_name, '[0-9]{8}t[0-9]{6}z', 'ts') "
            "FROM information_schema.tables WHERE table_schema NOT IN ('information_schema') "
            "ORDER BY 1, 2"
        ),
    )
    events: list[tuple[object, ...]] = []
    schema: str
    for schema in schemas:
        events.extend(
            query_duckdb(
                db_path=db_path,
                sql=(
                    "SELECT relation_schema, event_type, original_name, "
                    "regexp_replace(archive_name, '[0-9]{8}t[0-9]{6}z', 'ts') "
                    f"FROM {schema}._sqlbuild_janitor_events ORDER BY 1, 2, 3, 4"
                ),
            )
        )
    return (*relations, *events)


USAGE_JANITOR_ADAPTER_NAME: str = "usage_janitor_duckdb"
USAGE_JANITOR_SETTINGS_FILENAME: str = "janitor_usage_settings.json"
USAGE_JANITOR_CALLS_FILENAME: str = "janitor_usage_calls.json"
USAGE_JANITOR_ADAPTER_SOURCE: str = """
import json
from dataclasses import replace
from datetime import datetime
from pathlib import Path
from typing import Any

from sqlbuild.adapter.contract.models import RelationInfo, RelationUsage, RelationUsageReport
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter

_ROOT = Path(__file__).resolve().parent.parent


def _settings() -> dict[str, Any]:
    return json.loads((_ROOT / "janitor_usage_settings.json").read_text(encoding="utf-8"))


class UsageJanitorDuckDbAdapter(DuckDbAdapter):
    adapter_name = "usage_janitor_duckdb"

    def supports_relation_age_metadata(self) -> bool:
        return True

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        changed_at: datetime = datetime.fromisoformat(_settings()["changed_at"])
        return tuple(
            replace(relation, created_at=changed_at)
            if relation.name.startswith("old_")
            else relation
            for relation in super().list_relations(
                connection=connection, database=database, schemas=schemas, names=names
            )
        )

    def relation_usage(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
        window_seconds: int,
    ) -> RelationUsageReport:
        (_ROOT / "janitor_usage_calls.json").write_text(
            json.dumps(
                {
                    "relations": sorted(relation.name for relation in relations),
                    "window_seconds": window_seconds,
                }
            ),
            encoding="utf-8",
        )
        settings: dict[str, Any] = _settings()
        if settings["failure"] is not None:
            raise RuntimeError(settings["failure"])
        if settings["unavailable"] is not None:
            return RelationUsageReport(checked=False, reason=settings["unavailable"])
        reads: dict[str, dict[str, Any]] = settings["reads"]
        return RelationUsageReport(
            checked=True,
            usages=tuple(
                RelationUsage(
                    database=relation.database,
                    schema=relation.schema,
                    name=relation.name,
                    last_read_at=datetime.fromisoformat(reads[relation.name]["last_read_at"]),
                    readers=tuple(reads[relation.name]["readers"]),
                    query_count=int(reads[relation.name]["query_count"]),
                    direct=reads[relation.name]["via"] is None,
                    via=tuple(reads[relation.name]["via"] or ()),
                )
                for relation in relations
                if relation.name in reads
            ),
            notices=tuple(settings["notices"]),
        )
"""


def prepare_usage_janitor_project(
    *,
    tmp_path: Path,
    project_name: str,
    stale_tables: int,
    stale_views: tuple[str, ...] = (),
    janitor_config: str = "",
) -> Path:
    """A built DuckDB project with stale tables and views, an expired archive, and a usage adapter.

    Every stale `old_*` relation last changed on 2026-09-01, so it is older than the retention.
    """

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name=project_name,
        repo_files={
            "sqlbuild_project.toml": (
                f'name = "{project_name}"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                f'database = "{(tmp_path / project_name / "janitor.duckdb").as_posix()}"\n\n'
                "[janitor]\n"
                "enabled = true\n"
                "retention_days = 14\n"
                "archive_retention_days = 14\n"
                "delete_tracked_only = false\n"
                f"{dedent(janitor_config).strip()}\n\n"
                "[defaults]\n"
                'materialized = "table"\n'
            ),
            "models/orders.sql": "MODEL (description 'Test model.');\n\nSELECT 1 AS order_id\n",
            f"adapters/{USAGE_JANITOR_ADAPTER_NAME}.py": USAGE_JANITOR_ADAPTER_SOURCE,
        },
    )
    _build(project_dir)
    tables: str = "; ".join(
        f"CREATE TABLE main.old_orders_{index:03d} AS SELECT {index} AS id"
        for index in range(stale_tables)
    )
    views: str = "".join(f"; CREATE VIEW main.{name} AS SELECT 1 AS id" for name in stale_views)
    execute_duckdb(
        db_path=project_dir / "janitor.duckdb",
        sql=f'{tables}{views}; CREATE TABLE main."{EXPIRED_ARCHIVE_NAME}" AS SELECT 1 AS id',
    )
    write_usage_settings(project_dir=project_dir)
    return project_dir


def write_usage_settings(
    *,
    project_dir: Path,
    reads: Mapping[str, tuple[tuple[str, ...], int]] = MappingProxyType({}),
    unavailable: str | None = None,
    via: Mapping[str, tuple[str, ...]] = MappingProxyType({}),
    failure: str | None = None,
    notices: tuple[str, ...] = (),
) -> None:
    """Run janitor on the usage adapter: who read which relation, or why usage is unavailable.

    A relation in `via` was read only through the named objects; `failure` makes the read fail;
    `notices` say what a completed check left out.
    """

    (project_dir / "sqlbuild_local.toml").write_text(
        f'adapter = "{USAGE_JANITOR_ADAPTER_NAME}"\n', encoding="utf-8"
    )
    (project_dir / USAGE_JANITOR_SETTINGS_FILENAME).write_text(
        json.dumps(
            {
                "changed_at": "2026-09-01T08:00:00+00:00",
                "reads": {
                    name: {
                        "readers": list(readers),
                        "query_count": count,
                        "last_read_at": "2026-10-21T09:30:00+00:00",
                        "via": via.get(name),
                    }
                    for name, (readers, count) in reads.items()
                },
                "unavailable": unavailable,
                "failure": failure,
                "notices": list(notices),
            }
        ),
        encoding="utf-8",
    )


def read_usage_calls(*, project_dir: Path) -> dict[str, object]:
    """The relations and window of the usage adapter's last `relation_usage` call."""

    return json.loads((project_dir / USAGE_JANITOR_CALLS_FILENAME).read_text(encoding="utf-8"))


def count_stale_archives(*, db_path: Path) -> int:
    """How many archives in `main` hold a stale `old_orders` relation."""

    return sum("__old_orders" in name for name in list_archive_names(db_path=db_path))


def in_use_entries(candidates: list[dict[str, object]]) -> tuple[tuple[object, object], ...]:
    """Each `--json` candidate's name and `in_use`."""

    return tuple((candidate["name"], candidate["in_use"]) for candidate in candidates)
