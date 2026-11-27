"""A worker files what each node prints under that node in the output store, with its ctx.log lines."""

import json
import sqlite3
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    NulFailureTestCase,
    NulLogTestCase,
    PhaseEventsTestCase,
    PythonFailureTestCase,
    RunOutputTestCase,
    SinkDeliveryTestCase,
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
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders.', materialized table,\n"
                    '  post_hooks [python("chatty")]);\n'
                    "SELECT 17 AS amount\n"
                ),
                "sql/hooks/python/chatty.py": (
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
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders.', materialized table,\n"
                    '  post_hooks [python("chatty")]);\n'
                    "SELECT 17 AS amount\n"
                ),
                "sql/hooks/python/chatty.py": (
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
            + [("model:orders", "info", f"line {n}") for n in range(1, 200)]
            + [
                (
                    "model:orders",
                    "warning",
                    "50 more lines not kept as log lines; ctx.log keeps 200 per node, "
                    "and print() output is kept in full in Output",
                )
            ],
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


@pytest.mark.parametrize(
    "test_case",
    [
        PythonFailureTestCase(
            "an asset failing sixty calls deep",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "failed",
            [(17,)],
            files={
                "python/nodes/assets/score_orders/asset.py": (
                    "from sqlbuild.assets import asset\n\n\n"
                    "def lookup_0(row):\n"
                    "    return row['customer_segment']\n\n\n"
                    + "".join(
                        f"def lookup_{n}(row):\n    return lookup_{n - 1}(row)\n\n\n"
                        for n in range(1, 61)
                    )
                    + "@asset(description='Scores orders.')\n"
                    "def score_orders(ctx):\n"
                    "    return lookup_60({'customer_id': 1})\n"
                ),
                "orchestration/jobs/annual_orders_job.toml": (
                    'name = "annual_orders_job"\n'
                    'description = "Build orders and score them."\n'
                    'select = "orders score_orders"\n'
                    'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                    'starts_at = "2000-01-01T00:00:00Z" }\n'
                ),
            },
            expected_error_start="Traceback (most recent call last):\n  ... ",
            expected_error_end="KeyError: 'customer_segment'\n",
            expected_error_below=2_000,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failing_python_node_when_a_worker_runs_it_then_its_run_error_is_the_end_of_its_traceback(
    test_case: PythonFailureTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        assert run["status"] == test_case.expected_status, worker.log.read_text()[-4000:]
        with sqlite3.connect(worker.control_store) as control:
            (error,) = control.execute(
                "SELECT json_extract(e.payload, '$.change.change.error_message') "
                "FROM sqb_run_nodes n JOIN recall_events e ON e.storage_id=n.finished_storage_id "
                "WHERE n.run_id=? AND n.node_key LIKE '%score_orders'",
                (run["run_id"],),
            ).fetchone()
        assert (
            error.startswith(test_case.expected_error_start),
            error.endswith(test_case.expected_error_end),
            len(error) < test_case.expected_error_below,
            "in lookup_0" in error,
        ) == (True, True, True, True), error


NUL_LOG_FILES: dict[str, str] = {
    "sql/models/orders.sql": (
        "MODEL (description 'Orders.', materialized table,\n"
        '  post_hooks [python("raw")]);\n'
        "SELECT 17 AS amount\n"
    ),
    "sql/hooks/python/raw.py": (
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


NUL_FAILURE_FILES: dict[str, str] = {
    "python/nodes/assets/score_orders/asset.py": (
        "from sqlbuild.assets import asset\n\n\n"
        "@asset(description='Scores orders.')\n"
        "def score_orders(ctx):\n"
        "    raise ValueError('refused ' + chr(0) + 'order 17')\n"
    ),
    "orchestration/jobs/annual_orders_job.toml": (
        'name = "annual_orders_job"\n'
        'description = "Build orders and score them."\n'
        'select = "orders score_orders"\n'
        'schedule = { cron = "0 0 * * *", timezone = "UTC", '
        'starts_at = "2000-01-01T00:00:00Z" }\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        NulFailureTestCase(
            "SQLite",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "failed",
            [(17,)],
            backend="sqlite",
            files=NUL_FAILURE_FILES,
            expected_error_end="ValueError: refused order 17\n",
        ),
        NulFailureTestCase(
            "PostgreSQL",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "failed",
            [(17,)],
            backend="postgres",
            files=NUL_FAILURE_FILES,
            expected_error_end="ValueError: refused order 17\n",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_nul_in_an_exception_when_its_traceback_is_reported_then_the_failed_finish_is_kept(
    test_case: NulFailureTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with (
        control_database(test_case.backend) as url,
        scheduled_worker(tmp_path, monkeypatch, test_case, database_url=url) as worker,
    ):
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        finishes: list[tuple[str, str]] = [
            (status, json.loads(payload)["change"]["change"].get("error_message", ""))
            for status, payload in control_query(
                url or f"sqlite://{worker.control_store}",
                "SELECT n.status, CAST(e.payload AS TEXT) FROM sqb_run_nodes n "
                "JOIN recall_events e ON e.storage_id=n.finished_storage_id "
                "WHERE n.node_key='asset:score_orders'",
            )
        ]
        assert (
            run["status"],
            [
                (status, error.endswith(test_case.expected_error_end), chr(0) in error)
                for status, error in finishes
            ],
        ) == (test_case.expected_status, [("failed", True, False)]), (
            finishes,
            worker.log.read_text()[-4000:],
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PhaseEventsTestCase(
            "SQLite",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            backend="sqlite",
            expected_phases=(
                ("discovered", True),
                ("compiled", True),
                ("matched", True),
                ("connected", True),
                ("inspected", True),
                ("planned", True),
                ("started_execution", False),
                ("connected", True),
                ("node", False),
            ),
            expected_planned="1 selected",
        ),
        PhaseEventsTestCase(
            "PostgreSQL",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            backend="postgres",
            expected_phases=(
                ("discovered", True),
                ("compiled", True),
                ("matched", True),
                ("connected", True),
                ("inspected", True),
                ("planned", True),
                ("started_execution", False),
                ("connected", True),
                ("node", False),
            ),
            expected_planned="1 selected",
        ),
        PhaseEventsTestCase(
            "a job of one Python asset starts execution before the asset and opens no SQL connections",
            "SELECT 17 AS amount",
            "SELECT 1",
            "success",
            [(1,)],
            files={
                "python/nodes/assets/score_orders/asset.py": (
                    "from sqlbuild.assets import asset\n\n\n"
                    "@asset(description='Scores orders.')\n"
                    "def score_orders(ctx):\n"
                    "    return 17\n"
                ),
                "orchestration/jobs/annual_orders_job.toml": (
                    'name = "annual_orders_job"\n'
                    'description = "Score orders."\n'
                    'select = "score_orders"\n'
                    'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                    'starts_at = "2000-01-01T00:00:00Z" }\n'
                ),
            },
            expected_phases=(
                ("discovered", True),
                ("compiled", True),
                ("matched", True),
                ("connected", True),
                ("inspected", True),
                ("planned", True),
                ("started_execution", False),
                ("node", False),
            ),
            expected_planned="1 Python node",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_worker_run_when_it_prepares_then_each_phase_is_a_run_event_with_its_duration(
    test_case: PhaseEventsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with (
        control_database(test_case.backend) as url,
        scheduled_worker(tmp_path, monkeypatch, test_case, database_url=url) as worker,
    ):
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        database: str = url or f"sqlite://{worker.control_store}"
        events: list[tuple[Any, ...]] = control_query(
            database,
            "SELECT COALESCE(payload->>'phase', 'node'), duration_ms, payload->>'name' "
            "FROM recall_events "
            "WHERE event_type IN ('invocation_phase', 'resource_attempt_started') "
            "ORDER BY occurred_at, storage_id",
        )
    first_node: int = [phase for phase, _, _ in events].index("node")
    phases: list[tuple[str, bool]] = [
        (str(phase), duration is not None) for phase, duration, _ in events[: first_node + 1]
    ]
    planned: Any = {phase: name for phase, _, name in events}["planned"]

    assert run["status"] == test_case.expected_status, worker.log.read_text()[-4000:]
    assert (phases, planned) == (
        list(test_case.expected_phases),
        test_case.expected_planned,
    ), test_case.description


SINK_FILES: dict[str, str] = {
    "python/event_sinks/events.py": (
        "from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink\n\n\n"
        "@lifecycle_event_sink\n"
        "def broker(event: LifecycleEvent) -> None:\n"
        "    raise RuntimeError('broker unavailable')\n\n\n"
        "@lifecycle_event_sink\n"
        "def archive(event: LifecycleEvent) -> None:\n"
        "    del event\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        SinkDeliveryTestCase(
            "a sink that fails every event and one that delivers them",
            "SELECT 17 AS amount",
            "SELECT amount FROM orders",
            "success",
            [(17,)],
            files=SINK_FILES,
            expected_sinks=(("archive", False), ("broker", True)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_worker_run_with_event_sinks_when_it_finishes_then_it_reports_each_sinks_deliveries(
    test_case: SinkDeliveryTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        events: list[tuple[Any, ...]] = control_query(
            f"sqlite://{worker.control_store}",
            "SELECT event_type, payload FROM recall_events "
            "WHERE event_type IN ('run_sink_deliveries', 'run_terminated') ORDER BY storage_id",
        )
    reported: dict[str, Any] = json.loads(events[0][1])["change"]["change"]
    sinks: list[tuple[str, bool]] = sorted(
        (sink["name"], sink["failed"] == sink["accepted"] > 0) for sink in reported["sinks"]
    )
    assert (run["status"], [event_type for event_type, _ in events], sinks) == (
        test_case.expected_status,
        ["run_sink_deliveries", "run_terminated"],
        list(test_case.expected_sinks),
    ), worker.log.read_text()[-4000:]
