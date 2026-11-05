import json
import sqlite3
import subprocess
from importlib.metadata import version
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.recall._test_types import (
    BehindStoreTestCase,
    LayeredConnectionsTestCase,
    ReadOnlyReaderTestCase,
    RecallConnectionErrorTestCase,
    SplitCredentialsTestCase,
    UnsetWriterVariableTestCase,
)
from tests.integration.src.sqlbuild.cli.recall.helpers import (
    SPLIT_CREDENTIALS_TOML,
    make_store_behind,
    recall_rows,
    run,
    shared_store_project,
    store_schema,
    write_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RecallConnectionErrorTestCase(
            description="the removed connection key names its replacements on a build",
            recall_toml='[recall]\nconnection = "history"\n',
            args=("build",),
            expected_message=(
                "recall.connection was replaced by write_connection and read_connection: "
                'set write_connection = "history" and read_connection = "history"'
            ),
        ),
        RecallConnectionErrorTestCase(
            description="the removed connection key fails recall queries too",
            recall_toml='[targets.dev.recall]\nconnection = "history"\n',
            args=("recall", "query", "SELECT 1"),
            expected_message="targets.dev.recall.connection was replaced by write_connection",
        ),
        RecallConnectionErrorTestCase(
            description="a write connection without a read connection is rejected",
            recall_toml='[recall]\nwrite_connection = "history"\n',
            args=("build",),
            expected_message="recall.write_connection requires recall.read_connection",
        ),
        RecallConnectionErrorTestCase(
            description="a misspelled write connection fails the build",
            recall_toml=(
                '[recall]\nwrite_connection = "histroy_writer"\nread_connection = "history"\n'
            ),
            args=("build",),
            expected_message="error[C975]: recall.write_connection names unknown connection",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_recall_connections_when_running_then_the_command_fails_naming_the_keys(
    test_case: RecallConnectionErrorTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(
        tmp_path / "orders_shop",
        extra_toml=(
            '\n[connections.history]\nadapter = "sqlite"\npath = "history.sqlite3"\n\n'
            + test_case.recall_toml
        ),
    )

    completed: subprocess.CompletedProcess[bytes] = run(project_dir, *test_case.args)

    assert completed.returncode != 0
    assert test_case.expected_message in completed.stderr.decode(errors="replace")
    assert not (project_dir / "history.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        ReadOnlyReaderTestCase(
            description="a reader sees shared history and records its own runs locally",
            expected_shared_invocations=[["build", "completed"]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_only_a_read_connection_when_running_then_reads_are_shared_and_writes_local(
    test_case: ReadOnlyReaderTestCase, tmp_path: Path
) -> None:
    store: Path = tmp_path / "shared" / "history.sqlite3"
    writer: Path = shared_store_project(
        parent=tmp_path / "writer",
        store=store,
        keys='write_connection = "history"\nread_connection = "history"\n',
    )
    assert run(writer, "build").returncode == 0
    reader: Path = shared_store_project(
        parent=tmp_path / "reader", store=store, keys='read_connection = "history"\n'
    )
    schema_before: list[tuple[object, ...]] = store_schema(store)

    built: subprocess.CompletedProcess[bytes] = run(reader, "build")
    shared: list[list[object]] = recall_rows(reader, "SELECT command, status FROM invocations")

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert b"Recall" not in built.stderr
    assert shared == test_case.expected_shared_invocations
    assert (reader / "logs" / "recall.sqlite3").exists()
    assert store_schema(store) == schema_before


@pytest.mark.parametrize(
    "test_case",
    [
        BehindStoreTestCase(
            description="readers print the upgrade without applying it, and it applies cleanly",
            expected_ddl_fragments=(
                'ALTER TABLE "recall_heartbeats" ADD COLUMN "subcommand" TEXT;',
                'DROP VIEW IF EXISTS "invocations";',
                'CREATE VIEW IF NOT EXISTS "invocations" AS',
                'INSERT OR IGNORE INTO "recall_meta"',
                f"'{version('sqlbuild')}', strftime(",
            ),
            expected_up_to_date_note="is up to date with this SQLBuild version",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_store_one_version_behind_when_read_then_no_ddl_runs_and_the_printed_ddl_applies(
    test_case: BehindStoreTestCase, tmp_path: Path
) -> None:
    store: Path = tmp_path / "shared" / "history.sqlite3"
    project_dir: Path = shared_store_project(
        parent=tmp_path,
        store=store,
        keys='write_connection = "history"\nread_connection = "history"\n',
    )
    assert run(project_dir, "build").returncode == 0
    make_store_behind(store)
    schema_before: list[tuple[object, ...]] = store_schema(store)

    queried: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "query", "SELECT count(*) FROM recall_invocations"
    )
    printed: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "schema", "--ddl")
    schema_after_reads: list[tuple[object, ...]] = store_schema(store)
    with sqlite3.connect(store) as connection:
        connection.executescript(printed.stdout.decode())
    repeated: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "schema", "--ddl")
    rebuilt: subprocess.CompletedProcess[bytes] = run(project_dir, "build")

    ddl: str = printed.stdout.decode()
    assert (queried.returncode, printed.returncode, repeated.returncode) == (0, 0, 0)
    assert schema_after_reads == schema_before
    assert [fragment in ddl for fragment in test_case.expected_ddl_fragments] == [
        True for _ in test_case.expected_ddl_fragments
    ]
    assert "CREATE TABLE" not in ddl
    assert repeated.stdout == b""
    assert test_case.expected_up_to_date_note in repeated.stderr.decode()
    assert rebuilt.returncode == 0
    assert b"Recall" not in rebuilt.stderr
    assert recall_rows(project_dir, "SELECT count(*) FROM invocations") == [[2]]


@pytest.mark.parametrize(
    "test_case",
    [
        SplitCredentialsTestCase(
            description="the writer needs only its variable and the reader only its own",
            writer_environment={"ORDERS_RECALL_WRITER_PATH": "history.sqlite3"},
            reader_environment={"ORDERS_RECALL_READER_PATH": "history.sqlite3"},
            expected_invocations=[["build", "completed"]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_split_credentials_when_each_side_runs_then_it_resolves_only_its_connection(
    test_case: SplitCredentialsTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(
        tmp_path / "orders_shop",
        extra_toml=SPLIT_CREDENTIALS_TOML,
    )

    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, "build", **test_case.writer_environment
    )
    read: subprocess.CompletedProcess[bytes] = run(
        project_dir,
        "recall",
        "query",
        "--json",
        "SELECT command, status FROM invocations",
        **test_case.reader_environment,
    )

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert read.returncode == 0, read.stderr.decode(errors="replace")
    assert json.loads(read.stdout)["rows"] == test_case.expected_invocations
    assert not (project_dir / "logs" / "recall.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        LayeredConnectionsTestCase(
            description="a local read connection completes the project's write connection",
            project_toml='[recall]\nwrite_connection = "history"\n',
            local_toml='[recall]\nread_connection = "history"\n',
            expected_invocations=[["build", "completed"]],
        ),
        LayeredConnectionsTestCase(
            description="a target's write connection is completed by the project's read connection",
            project_toml=(
                '[recall]\nread_connection = "history"\n'
                '[targets.dev.recall]\nwrite_connection = "history"\n'
            ),
            local_toml="",
            expected_invocations=[["build", "completed"]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_connections_split_across_layers_when_building_then_history_is_recorded(
    test_case: LayeredConnectionsTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(
        tmp_path / "orders_shop",
        extra_toml=(
            '\n[connections.history]\nadapter = "sqlite"\npath = "history.sqlite3"\n\n'
            + test_case.project_toml
        ),
    )
    _ = (project_dir / "sqlbuild_local.toml").write_text(test_case.local_toml)

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert (
        recall_rows(project_dir, "SELECT command, status FROM invocations")
        == test_case.expected_invocations
    )
    assert (project_dir / "history.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        UnsetWriterVariableTestCase(
            description="an unset writer variable skips recording without failing the build",
            expected_exit_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unset_writer_variable_when_building_then_the_command_still_succeeds(
    test_case: UnsetWriterVariableTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", extra_toml=SPLIT_CREDENTIALS_TOML)

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")

    assert built.returncode == test_case.expected_exit_code, built.stderr.decode(errors="replace")
    assert not (project_dir / "history.sqlite3").exists()
