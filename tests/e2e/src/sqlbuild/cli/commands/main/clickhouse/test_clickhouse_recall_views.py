from __future__ import annotations

import json
import os
import signal
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse._test_types import (
    ClickHouseRecallPreviousVersionTestCase,
    ClickHouseRecallReaderSettingsTestCase,
    ClickHouseRecallReadOnlyUserTestCase,
    ClickHouseRecallRefusedSettingTestCase,
    ClickHouseRecallTableReadsTestCase,
    ClickHouseRecallViewEquivalenceTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.clickhouse.helpers import (
    RECALL_SCHEMA_VERSION,
    RECALL_VIEWS,
    column_names,
    create_previous_views,
    described,
    described_except,
    differing_rows,
    duplicate_recall_rows,
    execute_clickhouse_sql,
    fetch_clickhouse_rows,
    limited_user,
    primary_key_reads,
    readonly_user,
    representative_store,
    snapshot_views,
    table_reads,
    write_recall_project,
)
from tests.integration.src.sqlbuild.cli.recall.helpers import recall_rows, run


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRecallReadOnlyUserTestCase(
            description="a readonly = 1 user queries every view, the schema and the output",
            view_names=RECALL_VIEWS,
            expected_status=[["completed"]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_readonly_1_reader_when_recalling_then_no_setting_is_needed(
    test_case: ClickHouseRecallReadOnlyUserTestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
    recall_database: str,
) -> None:
    writer: Path = write_recall_project(
        parent=tmp_path / "writer", config=clickhouse_e2e_config, database=recall_database
    )
    built: subprocess.CompletedProcess[bytes] = run(writer, "build")

    with readonly_user(config=clickhouse_e2e_config, database=recall_database) as reader:
        project_dir: Path = write_recall_project(
            parent=tmp_path / "reader", config=reader, database=recall_database
        )
        counted: list[subprocess.CompletedProcess[bytes]] = [
            run(project_dir, "recall", "query", "--json", f"SELECT count() FROM {view}")
            for view in test_case.view_names
        ]
        limited: subprocess.CompletedProcess[bytes] = run(
            project_dir, "recall", "query", "--json", "--limit", "1", "SELECT * FROM events"
        )
        schema: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "schema")
        ddl: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "schema", "--ddl")
        output: subprocess.CompletedProcess[bytes] = run(
            project_dir, "recall", "output", "last", "--stream", "stdout"
        )
        status: list[list[object]] = recall_rows(project_dir, "SELECT status FROM invocations")
        refused: subprocess.CompletedProcess[bytes] = run(
            project_dir, "recall", "query", f"DROP TABLE {recall_database}.recall_meta"
        )

    assert built.returncode == 0, built.stderr
    assert [completed.returncode for completed in counted] == [0 for _ in counted], [
        completed.stderr for completed in counted
    ]
    assert [len(json.loads(completed.stdout)["rows"]) for completed in counted] == [
        1 for _ in counted
    ]
    document: dict[str, object] = json.loads(limited.stdout)
    assert (limited.returncode, len(document["rows"]), document["truncated"]) == (0, 1, True)  # type: ignore[arg-type]
    assert schema.returncode == 0, schema.stderr
    assert b"invocations" in schema.stdout
    assert (ddl.returncode, ddl.stdout) == (0, b""), ddl.stderr
    assert (output.returncode, output.stdout) == (0, built.stdout), output.stderr
    assert status == test_case.expected_status
    assert refused.returncode == 1
    assert fetch_clickhouse_rows(
        config=clickhouse_e2e_config,
        sql=f"SELECT count() FROM system.tables WHERE database = '{recall_database}' "
        "AND name = 'recall_meta'",
    ) == ((1,),)


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRecallRefusedSettingTestCase(
            description="a user that may not change readonly is told to set readonly = 2",
            user_settings="readonly = 0 CONST",
            expected_fragments=(
                "error[C974]",
                "may not change the setting `readonly`",
                "`readonly = 2`",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_reader_that_may_not_set_readonly_when_querying_then_the_error_names_the_setting(
    test_case: ClickHouseRecallRefusedSettingTestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
    recall_database: str,
) -> None:
    writer: Path = write_recall_project(
        parent=tmp_path / "writer", config=clickhouse_e2e_config, database=recall_database
    )
    assert run(writer, "build").returncode == 0

    with readonly_user(
        config=clickhouse_e2e_config, database=recall_database, settings=test_case.user_settings
    ) as reader:
        project_dir: Path = write_recall_project(
            parent=tmp_path / "reader", config=reader, database=recall_database
        )
        queried: subprocess.CompletedProcess[bytes] = run(
            project_dir, "recall", "query", "SELECT count() FROM invocations"
        )
        output: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "output", "last")

    stderr: str = queried.stderr.decode(errors="replace")
    assert queried.returncode == 1
    assert [fragment in stderr for fragment in test_case.expected_fragments] == [
        True for _ in test_case.expected_fragments
    ]
    assert output.returncode == 0, output.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRecallReaderSettingsTestCase(
            description="a readonly = 1 reader runs with the server's own settings",
            user_settings="readonly = 1",
            expected_rows=[[0, "", 1]],
        ),
        ClickHouseRecallReaderSettingsTestCase(
            description="a readonly = 2 reader gets the pinned settings",
            user_settings="readonly = 2",
            expected_rows=[[1, "DISTINCT", 1]],
        ),
        ClickHouseRecallReaderSettingsTestCase(
            description="a reader allowed to change settings gets the pinned settings",
            user_settings="readonly = 0",
            expected_rows=[[1, "DISTINCT", 1]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_reader_when_querying_then_pinned_settings_apply_unless_it_may_not_set_them(
    test_case: ClickHouseRecallReaderSettingsTestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
    recall_database: str,
) -> None:
    writer: Path = write_recall_project(
        parent=tmp_path / "writer", config=clickhouse_e2e_config, database=recall_database
    )
    assert run(writer, "build").returncode == 0

    with readonly_user(
        config=clickhouse_e2e_config, database=recall_database, settings=test_case.user_settings
    ) as reader:
        project_dir: Path = write_recall_project(
            parent=tmp_path / "reader", config=reader, database=recall_database
        )
        rows: list[list[object]] = recall_rows(
            project_dir,
            "SELECT toUInt8(getSetting('join_use_nulls')), "
            "toString(getSetting('union_default_mode')), count() FROM invocations",
        )

    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRecallViewEquivalenceTestCase(
            description="rewritten views match the previous ones with and without duplicates",
            replaced_views=("invocations", "statements", "output_invocations", "artifacts"),
            all_views=RECALL_VIEWS,
            expected_statuses=[
                ["abandoned"],
                ["abandoned"],
                ["completed"],
                ["failed"],
                ["running"],
            ],
            expected_added_columns=(
                (
                    ("ended_at_estimated", "Nullable(UInt8)"),
                    ("signal_number", "Nullable(Int64)"),
                    ("cost_status", "Nullable(String)"),
                    ("origin", "Nullable(String)"),
                    ("command_mode", "Nullable(String)"),
                    ("selection_count", "Nullable(Int64)"),
                    ("selection_truncated", "Nullable(UInt8)"),
                    ("selector_files", "Nullable(String)"),
                    ("integration", "Nullable(String)"),
                    ("integration_job", "Nullable(String)"),
                    ("integration_run_id", "Nullable(String)"),
                    ("integration_retry", "Nullable(Int64)"),
                ),
                (("sql_unavailable_reason", "Nullable(String)"),),
                (),
                (),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_representative_invocations_when_views_are_rewritten_then_rows_and_types_are_unchanged(
    test_case: ClickHouseRecallViewEquivalenceTestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
    recall_database: str,
) -> None:
    project_dir: Path = write_recall_project(
        parent=tmp_path, config=clickhouse_e2e_config, database=recall_database
    )
    running: subprocess.Popen[bytes] = representative_store(project_dir)
    try:
        create_previous_views(
            config=clickhouse_e2e_config,
            database=recall_database,
            names=test_case.replaced_views,
            prefix="previous_",
        )
        statuses: list[list[object]] = recall_rows(
            project_dir, "SELECT status FROM invocations ORDER BY status"
        )
        previous_columns: list[tuple[str, ...]] = [
            column_names(
                config=clickhouse_e2e_config, relation=f"{recall_database}.previous_{view}"
            )
            for view in test_case.replaced_views
        ]
        against_previous: list[int] = [
            differing_rows(
                config=clickhouse_e2e_config,
                left=f"{recall_database}.{view}",
                right=f"{recall_database}.previous_{view}",
                settings={"join_use_nulls": 1},
                columns=columns,
            )
            for view, columns in zip(test_case.replaced_views, previous_columns, strict=True)
        ]
        compared: list[tuple[tuple[tuple[object, ...], ...], tuple[tuple[object, ...], ...]]] = [
            described_except(
                config=clickhouse_e2e_config,
                relation=f"{recall_database}.{view}",
                previous=f"{recall_database}.previous_{view}",
            )
            for view in test_case.replaced_views
        ]
        types: list[bool] = [
            kept
            == described(
                config=clickhouse_e2e_config, relation=f"{recall_database}.previous_{view}"
            )
            for view, (kept, _) in zip(test_case.replaced_views, compared, strict=True)
        ]
        added: tuple[tuple[tuple[object, ...], ...], ...] = tuple(
            added_columns for _, added_columns in compared
        )
    finally:
        os.killpg(running.pid, signal.SIGKILL)
        _ = running.wait(timeout=30)
    snapshot_views(
        config=clickhouse_e2e_config, database=recall_database, names=test_case.all_views
    )
    without_join_nulls: list[int] = [
        differing_rows(
            config=clickhouse_e2e_config,
            left=f"{recall_database}.{view}",
            right=f"{recall_database}.snapshot_{view}",
            settings={"join_use_nulls": 0},
        )
        for view in test_case.all_views
    ]
    duplicate_recall_rows(config=clickhouse_e2e_config, database=recall_database)
    with_duplicates: list[int] = [
        differing_rows(
            config=clickhouse_e2e_config,
            left=f"{recall_database}.{view}",
            right=f"{recall_database}.snapshot_{view}",
            settings={},
        )
        for view in test_case.all_views
    ]
    previous_with_duplicates: list[int] = [
        differing_rows(
            config=clickhouse_e2e_config,
            left=f"{recall_database}.{view}",
            right=f"{recall_database}.previous_{view}",
            settings={"join_use_nulls": 1},
            columns=columns,
        )
        for view, columns in zip(test_case.replaced_views, previous_columns, strict=True)
    ]
    counts_with_duplicates: list[bool] = [
        fetch_clickhouse_rows(
            config=clickhouse_e2e_config,
            sql=f"SELECT (SELECT count() FROM {recall_database}.{view}) "
            f"= (SELECT count() FROM {recall_database}.snapshot_{view})",
        )
        == ((1,),)
        for view in test_case.all_views
    ]

    assert statuses == test_case.expected_statuses
    assert against_previous == [0 for _ in test_case.replaced_views]
    assert types == [True for _ in test_case.replaced_views]
    assert added == test_case.expected_added_columns
    assert without_join_nulls == [0 for _ in test_case.all_views]
    assert with_duplicates == [0 for _ in test_case.all_views]
    assert previous_with_duplicates == [0 for _ in test_case.replaced_views]
    assert counts_with_duplicates == [True for _ in test_case.all_views]


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRecallTableReadsTestCase(
            description="invocations reads its three tables once each",
            view="invocations",
            expected_tables=("recall_events", "recall_heartbeats", "recall_invocations"),
            expected_primary_key_reads=3,
        ),
        ClickHouseRecallTableReadsTestCase(
            description="statements reads the events, SQL references and stored SQL once each",
            view="statements",
            expected_tables=("recall_artifact_refs", "recall_artifacts", "recall_events"),
            expected_primary_key_reads=1,
        ),
        ClickHouseRecallTableReadsTestCase(
            description="output_invocations reads its three tables once each",
            view="output_invocations",
            expected_tables=("recall_delivery", "recall_invocations", "recall_output"),
            expected_primary_key_reads=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_one_invocation_lookup_when_planned_then_each_table_is_read_once_by_its_key(
    test_case: ClickHouseRecallTableReadsTestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
    recall_database: str,
) -> None:
    project_dir: Path = write_recall_project(
        parent=tmp_path, config=clickhouse_e2e_config, database=recall_database
    )
    assert run(project_dir, "build").returncode == 0
    # A command that ends before its first heartbeat writes none, and ClickHouse plans a table
    # with no parts as a read of nothing, so give the heartbeats table a part before planning.
    # The row starts now: an expired TTL would drop it on insert and its empty part soon after.
    execute_clickhouse_sql(
        config=clickhouse_e2e_config,
        statements=(
            f"INSERT INTO {recall_database}.recall_heartbeats "
            "(invocation_id, invocation_started_at, beat_at, stale_at) "
            "VALUES ('planning-setup', now64(6), now64(6), now64(6))",
        ),
    )
    sql: str = (
        f"SELECT * FROM {recall_database}.{test_case.view} WHERE invocation_id = 'any-invocation'"
    )

    reads: tuple[str, ...] = table_reads(config=clickhouse_e2e_config, sql=sql)
    keyed: int = primary_key_reads(config=clickhouse_e2e_config, sql=sql)

    assert reads == tuple(f"{recall_database}.{table}" for table in test_case.expected_tables)
    assert keyed == test_case.expected_primary_key_reads


@pytest.mark.parametrize(
    "test_case",
    [
        ClickHouseRecallPreviousVersionTestCase(
            description="a store of the previous version is upgraded by whoever may",
            replaced_views=("invocations", "statements", "output_invocations", "artifacts"),
            expected_notice="is behind this SQLBuild version, and this user may not upgrade it",
            expected_ddl_fragments=(
                "CREATE OR REPLACE VIEW `{database}`.`invocations`",
                "CREATE OR REPLACE VIEW `{database}`.`statements`",
                f"SELECT 'schema', 'schema_version', '{RECALL_SCHEMA_VERSION}'",
            ),
            expected_version=((RECALL_SCHEMA_VERSION,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_previous_version_store_when_built_then_limited_writers_warn_and_admins_upgrade(
    test_case: ClickHouseRecallPreviousVersionTestCase,
    tmp_path: Path,
    clickhouse_e2e_config: dict[str, object],
    recall_database: str,
) -> None:
    admin: Path = write_recall_project(
        parent=tmp_path / "admin", config=clickhouse_e2e_config, database=recall_database
    )
    assert run(admin, "build").returncode == 0
    create_previous_views(
        config=clickhouse_e2e_config,
        database=recall_database,
        names=test_case.replaced_views,
        prefix="",
    )
    execute_clickhouse_sql(
        config=clickhouse_e2e_config,
        statements=(
            f"ALTER TABLE {recall_database}.recall_meta UPDATE value = '2' "
            "WHERE component = 'schema' SETTINGS mutations_sync = 2",
        ),
    )

    with limited_user(config=clickhouse_e2e_config, database=recall_database) as writer:
        writer_dir: Path = write_recall_project(
            parent=tmp_path / "writer", config=writer, database=recall_database
        )
        limited: subprocess.CompletedProcess[bytes] = run(writer_dir, "build")
        printed: subprocess.CompletedProcess[bytes] = run(writer_dir, "recall", "schema", "--ddl")
    upgraded: subprocess.CompletedProcess[bytes] = run(admin, "build")
    repeated: subprocess.CompletedProcess[bytes] = run(admin, "recall", "schema", "--ddl")

    ddl: str = printed.stdout.decode()
    assert limited.returncode == 0, limited.stderr
    assert test_case.expected_notice in limited.stderr.decode(errors="replace")
    assert [
        fragment.format(database=recall_database) in ddl
        for fragment in test_case.expected_ddl_fragments
    ] == [True for _ in test_case.expected_ddl_fragments]
    assert upgraded.returncode == 0, upgraded.stderr
    assert b"Recall" not in upgraded.stderr
    assert (repeated.returncode, repeated.stdout) == (0, b"")
    assert (
        fetch_clickhouse_rows(
            config=clickhouse_e2e_config,
            sql=f"SELECT max(toInt64OrZero(value)) FROM {recall_database}.recall_meta "
            "WHERE component = 'schema'",
        )
        == test_case.expected_version
    )
    assert fetch_clickhouse_rows(
        config=clickhouse_e2e_config,
        sql=f"SELECT countIf(position(create_table_query, 'join_use_nulls') > 0) "
        f"FROM system.tables WHERE database = '{recall_database}' AND engine = 'View'",
    ) == ((0,),)
    assert recall_rows(admin, "SELECT count() FROM invocations") == [[3]]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
