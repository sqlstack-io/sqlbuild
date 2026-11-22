"""A worker files what each node prints under that node in the output store, with its ctx.log lines."""

import sqlite3
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    NulLogTestCase,
    RunOutputTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    control_database,
    control_query,
    scheduled_worker,
    wait_for_run,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RunOutputTestCase(
            "a post-hook printing about 50 MB",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            files={
                "models/orders.sql": (
                    "MODEL (description 'Orders.', materialized table,\n"
                    '  post_hooks [python("chatty")]);\n'
                    "SELECT 17 AS amount\n"
                ),
                "hooks/python/chatty.py": (
                    "import sys\n\n"
                    "from sqlbuild.hooks import hook\n\n\n"
                    "@hook\n"
                    "def chatty(ctx):\n"
                    "    '''Print a large log and two logged lines.'''\n"
                    "    ctx.log.info('loaded orders')\n"
                    "    block = ''.join(f'order row {n:07d} ' + 'x' * 80 + '\\n' for n in range(1000))\n"
                    "    for _ in range(500):\n"
                    "        sys.stdout.write(block)\n"
                    "    sys.stdout.write('ERROR: one order had no customer\\n')\n"
                    "    sys.stdout.flush()\n"
                    "    ctx.log.warning('1 order had no customer')\n"
                ),
            },
            expected_lines=500_001,
            expected_error_lines=[500_001],
            expected_logs=[
                ("model:orders", "info", "loaded orders"),
                ("model:orders", "warning", "1 order had no customer"),
            ],
        ),
        RunOutputTestCase(
            "a hook logging past the per-node cap",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            files={
                "models/orders.sql": (
                    "MODEL (description 'Orders.', materialized table,\n"
                    '  post_hooks [python("chatty")]);\n'
                    "SELECT 17 AS amount\n"
                ),
                "hooks/python/chatty.py": (
                    "from sqlbuild.hooks import hook\n\n\n"
                    "@hook\n"
                    "def chatty(ctx):\n"
                    "    '''Log a long line, then more lines than a node reports.'''\n"
                    "    ctx.log.error('x' * 5000)\n"
                    "    for n in range(1, 250):\n"
                    "        ctx.log.info(f'line {n}')\n"
                    "    print('done')\n"
                ),
            },
            expected_lines=1,
            expected_error_lines=[],
            expected_logs=[("model:orders", "error", "x" * 4093 + "…")]
            + [("model:orders", "info", f"line {n}") for n in range(1, 200)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_node_printing_a_large_log_when_a_worker_runs_it_then_its_lines_are_filed_under_it(
    test_case: RunOutputTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        assert run["status"] == test_case.expected_status, worker.log.read_text()[-4000:]
        with sqlite3.connect(worker.control_store) as control:
            streams: dict[tuple[str, str], int] = {
                (node, stream): lines
                for node, stream, lines in control.execute(
                    "SELECT node_key, stream, line_count FROM sqb_run_output_streams "
                    "WHERE run_id=?",
                    (run["run_id"],),
                ).fetchall()
            }
            error_lines: list[int] = [
                line
                for (line,) in control.execute(
                    "SELECT node_line FROM sqb_run_output_marks WHERE run_id=? "
                    "AND node_key='model:orders' AND level='error' ORDER BY node_line",
                    (run["run_id"],),
                ).fetchall()
            ]
            totals: tuple[int, int] = control.execute(
                "SELECT dropped_bytes, omitted_bytes FROM sqb_run_output WHERE run_id=?",
                (run["run_id"],),
            ).fetchone()
            logs: list[tuple[str, str, str]] = control.execute(
                "SELECT node_key, level, message FROM sqb_run_node_logs WHERE run_id=? "
                "ORDER BY logged_at, event_id",
                (run["run_id"],),
            ).fetchall()
        assert totals == (0, 0), streams
        assert (streams[("model:orders", "stdout")], error_lines) == (
            test_case.expected_lines,
            test_case.expected_error_lines,
        )
        assert any(node == "run" for node, _ in streams), streams
        assert logs == test_case.expected_logs
        assert any((worker.control_store.parent / "output" / "runs").rglob("*.zst"))


NUL_LOG_FILES: dict[str, str] = {
    "models/orders.sql": (
        "MODEL (description 'Orders.', materialized table,\n"
        '  post_hooks [python("raw")]);\n'
        "SELECT 17 AS amount\n"
    ),
    "hooks/python/raw.py": (
        "from sqlbuild.hooks import hook\n\n\n"
        "@hook\n"
        "def raw(ctx):\n"
        "    '''Log text holding NUL.'''\n"
        "    ctx.log.info('loaded ' + chr(0) + 'orders')\n"
        "    ctx.log.warning(chr(0) + 'y' * 4096)\n"
    ),
}
# NUL goes before the cut, so a message that fits without it keeps its last character.
NUL_LOGS: list[tuple[str, str]] = [("info", "loaded orders"), ("warning", "y" * 4096)]


@pytest.mark.parametrize(
    "test_case",
    [
        NulLogTestCase(
            "SQLite",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            backend="sqlite",
            files=NUL_LOG_FILES,
            expected_logs=NUL_LOGS,
        ),
        NulLogTestCase(
            "PostgreSQL",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            backend="postgres",
            files=NUL_LOG_FILES,
            expected_logs=NUL_LOGS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_nul_in_a_ctx_log_message_when_a_worker_reports_it_then_the_log_and_finish_are_kept(
    test_case: NulLogTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with (
        control_database(test_case.backend) as url,
        scheduled_worker(tmp_path, monkeypatch, test_case, database_url=url) as worker,
    ):
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        database: str = url or f"sqlite://{worker.control_store}"
        logs: list[tuple[str, str]] = [
            (level, message)
            for level, message in control_query(
                database,
                "SELECT level, message FROM sqb_run_node_logs "
                "WHERE node_key='model:orders' ORDER BY logged_at, event_id",
            )
        ]
        nodes: list[tuple[str, bool]] = [
            (status, finished is not None)
            for status, finished in control_query(
                database,
                "SELECT status, finished_storage_id FROM sqb_run_nodes "
                "WHERE node_key='model:orders' AND kind='node'",
            )
        ]
        assert (run["status"], nodes, logs) == (
            test_case.expected_status,
            [("success", True)],
            test_case.expected_logs,
        ), worker.log.read_text()[-4000:]
