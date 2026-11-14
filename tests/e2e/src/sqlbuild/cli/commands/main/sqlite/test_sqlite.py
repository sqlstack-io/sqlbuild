from __future__ import annotations

import multiprocessing
import subprocess
import time
from multiprocessing.context import SpawnContext
from multiprocessing.process import BaseProcess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.sqlite._test_types import (
    SqliteCommandE2ETestCase,
    SqliteFailureE2ETestCase,
    SqliteLockE2ETestCase,
    SqliteOracleE2ETestCase,
    SqliteProjectE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.sqlite.helpers import (
    CHANGED_EVENTS_CSV,
    COMPARED_MODELS,
    CONCURRENT_MICROBATCH_PROJECT_FILES,
    CONTRACT_PROJECT_FILES,
    FUNCTION_PROJECT_FILES,
    LOCK_PROJECT_FILES,
    SQLITE_FILE,
    STRATEGY_PROJECT_FILES,
    append_late_orders,
    build_orders_project_files,
    execute_sqlite,
    hold_sqlite_write_lock,
    model_rows,
    sqlite_rows,
    table_names,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteOracleE2ETestCase(
            description="seed, view, table, append, merge and delete_insert match DuckDB",
            compared_models=COMPARED_MODELS,
            unexpected_rebuild_fragments=("Schema changed", "type changed"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_orders_project_when_building_twice_then_sqlite_matches_duckdb(
    tmp_path: Path, test_case: SqliteOracleE2ETestCase
) -> None:
    outputs: dict[str, list[str]] = {"sqlite": [], "duckdb": []}
    rows: dict[str, list[tuple[tuple[object, ...], ...]]] = {"sqlite": [], "duckdb": []}
    adapter: str
    for adapter in ("sqlite", "duckdb"):
        project_dir: Path = prepare_inline_project(
            tmp_path=tmp_path,
            project_name=f"orders_{adapter}",
            repo_files=build_orders_project_files(adapter=adapter),
        )
        first: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        append_late_orders(project_dir=project_dir)
        second: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
        outputs[adapter].extend((first.stdout + first.stderr, second.stdout + second.stderr))
        assert (first.returncode, second.returncode) == (test_case.expected_return_code,) * 2, (
            outputs[adapter]
        )
        rows[adapter].extend(
            model_rows(adapter=adapter, project_dir=project_dir, model=model, order_by=key)
            for model, key in test_case.compared_models
        )

    assert rows["sqlite"] == rows["duckdb"]
    for fragment in test_case.unexpected_rebuild_fragments:
        assert fragment not in outputs["sqlite"][1]


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteCommandE2ETestCase(
            description="test runs unit tests against the file",
            command=("--no-color", "test"),
            expected_fragments=("test_customer_totals", "PASS=1"),
        ),
        SqliteCommandE2ETestCase(
            description="seed reloads seeds",
            command=("--no-color", "seed"),
            expected_fragments=("Completed successfully", "PASS=2"),
        ),
        SqliteCommandE2ETestCase(
            description="query reads schema relations by their stored name",
            command=(
                "--no-color",
                "query",
                "--format",
                "long",
                "SELECT customer_id, total_cents FROM analytics__customer_totals ORDER BY 1",
            ),
            expected_fragments=(
                "customer_id | 1\ntotal_cents | 1975",
                "customer_id | 2\ntotal_cents | 3000",
                "customer_id | 3\ntotal_cents | 500",
            ),
        ),
        SqliteCommandE2ETestCase(
            description="debug connects and runs a query",
            command=("--no-color", "debug"),
            expected_fragments=("adapter: sqlite [OK found]", "connection test: [OK connected]"),
        ),
        SqliteCommandE2ETestCase(
            description="clone copies dev into prod within the file",
            command=("--no-color", "clone", "--from", "dev", "--to", "prod"),
            expected_fragments=(
                "analytics__orders_merged -> prod__orders_merged",
                "Completed successfully",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_project_when_running_command_then_sqlite_succeeds(
    tmp_path: Path, test_case: SqliteCommandE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_shop",
        repo_files=build_orders_project_files(adapter="sqlite"),
    )
    build: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert build.returncode == 0, build.stdout + build.stderr

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_return_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteCommandE2ETestCase(
            description="diff between a clone and an edited dev reports the changed row",
            command=("--no-color", "diff", "prod:dev", "--select", "orders_merged"),
            expected_fragments=("status", "mismatches=1", "order_id=12 | placed -> lost"),
            expected_return_code=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cloned_target_when_diffing_then_sqlite_reports_changed_rows(
    tmp_path: Path, test_case: SqliteCommandE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders_shop",
        repo_files=build_orders_project_files(adapter="sqlite"),
    )
    setup: tuple[subprocess.CompletedProcess[str], ...] = (
        run_sqb(command=("--no-color", "build"), project_dir=project_dir),
        run_sqb(
            command=("--no-color", "clone", "--from", "dev", "--to", "prod"),
            project_dir=project_dir,
        ),
    )
    assert [result.returncode for result in setup] == [0, 0], [r.stdout for r in setup]
    execute_sqlite(
        db_path=project_dir / SQLITE_FILE,
        sql="UPDATE analytics__orders_merged SET status = 'lost' WHERE order_id = 12",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_return_code, output
    for fragment in test_case.expected_fragments:
        assert fragment in output


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteProjectE2ETestCase(
            description="sequential microbatches rewrite each day without state tables",
            files=STRATEGY_PROJECT_FILES,
            changed_files={"seeds/order_events.csv": CHANGED_EVENTS_CSV},
            query="SELECT * FROM analytics__daily_revenue ORDER BY 1",
            expected_rows=(("2026-01-02 00:00:00", 2, 550), ("2026-01-03 00:00:00", 1, 400)),
            absent_tables=("analytics___sqlbuild_microbatches", "_sqlbuild_microbatches"),
        ),
        SqliteProjectE2ETestCase(
            description="keyed cursor delete_insert replaces the window and changed keys",
            files=STRATEGY_PROJECT_FILES,
            changed_files={"seeds/order_events.csv": CHANGED_EVENTS_CSV},
            query="SELECT * FROM analytics__orders_window ORDER BY 1, 3",
            expected_rows=(
                (1, 100, "2026-01-01 10:00:00"),
                (2, 250, "2026-01-02 12:00:00"),
                (3, 300, "2026-01-02 09:00:00"),
                (4, 400, "2026-01-03 08:00:00"),
            ),
        ),
        SqliteProjectE2ETestCase(
            description="timestamp snapshot closes updated and hard-deleted versions",
            files=STRATEGY_PROJECT_FILES,
            changed_files={"seeds/order_events.csv": CHANGED_EVENTS_CSV},
            query=(
                "SELECT order_id, amount_cents, valid_from, valid_to IS NULL "
                "FROM analytics__order_history ORDER BY 1, 3"
            ),
            expected_rows=(
                (1, 100, "2026-01-01 10:00:00", 0),
                (2, 200, "2026-01-01 11:00:00", 0),
                (2, 250, "2026-01-02 12:00:00", 1),
                (3, 300, "2026-01-02 09:00:00", 1),
                (4, 400, "2026-01-03 08:00:00", 1),
            ),
        ),
        SqliteProjectE2ETestCase(
            description="check snapshot versions a changed amount",
            files=STRATEGY_PROJECT_FILES,
            changed_files={"seeds/order_events.csv": CHANGED_EVENTS_CSV},
            query=(
                "SELECT order_id, amount_cents, valid_to IS NULL "
                "FROM analytics__order_amount_history ORDER BY 1, 2"
            ),
            expected_rows=((1, 100, 1), (2, 200, 0), (2, 250, 1), (3, 300, 1), (4, 400, 1)),
        ),
        SqliteProjectE2ETestCase(
            description="an enforced decimal contract stores canonical text",
            files=CONTRACT_PROJECT_FILES,
            changed_files={},
            query="SELECT order_id, amount, typeof(amount) FROM analytics__order_amounts ORDER BY 1",
            expected_rows=((1, "12.50", "text"), (2, "7.00", "text")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_strategy_project_when_building_twice_then_sqlite_writes_expected_rows(
    tmp_path: Path, test_case: SqliteProjectE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="events_shop", repo_files=test_case.files
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    prepare_inline_project(
        tmp_path=tmp_path, project_name="events_shop", repo_files=test_case.changed_files
    )
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert (first.returncode, second.returncode) == (test_case.expected_return_code,) * 2, (
        first.stdout + first.stderr + second.stdout + second.stderr
    )
    assert (
        sqlite_rows(db_path=project_dir / SQLITE_FILE, sql=test_case.query)
        == test_case.expected_rows
    )
    assert set(test_case.absent_tables).isdisjoint(table_names(db_path=project_dir / SQLITE_FILE))


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteFailureE2ETestCase(
            description="SQL functions fail with a coded error naming the limit",
            files=FUNCTION_PROJECT_FILES,
            expected_fragments=("error[A705]", "SQLite has no CREATE FUNCTION"),
        ),
        SqliteFailureE2ETestCase(
            description="concurrent microbatch DML is refused at plan time",
            files=CONCURRENT_MICROBATCH_PROJECT_FILES,
            expected_fragments=("does not support concurrent microbatch", "batch_concurrency = 1"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unsupported_feature_when_building_then_sqlite_fails_with_code(
    tmp_path: Path, test_case: SqliteFailureE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="function_shop", repo_files=test_case.files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode != 0, output
    for fragment in test_case.expected_fragments:
        assert fragment in output


@pytest.mark.parametrize(
    "test_case",
    [
        SqliteLockE2ETestCase(
            description="a build blocked by another process's writer fails with A702",
            busy_timeout=0.5,
            hold_seconds=20.0,
            expected_fragments=("A702", "is locked by another writer"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_locked_file_when_building_then_sqlite_fails_with_locked_code(
    tmp_path: Path, test_case: SqliteLockE2ETestCase
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="lock_shop", repo_files=LOCK_PROJECT_FILES
    )
    context: SpawnContext = multiprocessing.get_context("spawn")
    ready_path: Path = tmp_path / "ready"
    holder: BaseProcess = context.Process(
        target=hold_sqlite_write_lock,
        kwargs={
            "db_path": str(project_dir / SQLITE_FILE),
            "ready_path": str(ready_path),
            "hold_seconds": test_case.hold_seconds,
        },
    )
    holder.start()
    try:
        deadline: float = time.monotonic() + 30
        while not ready_path.exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "build"), project_dir=project_dir
        )
    finally:
        holder.terminate()
        holder.join(timeout=30)

    output: str = result.stdout + result.stderr
    assert result.returncode != 0, output
    for fragment in test_case.expected_fragments:
        assert fragment in output
