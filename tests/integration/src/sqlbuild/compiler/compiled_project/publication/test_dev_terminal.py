"""`sqb dev` on a real terminal: sign-in in the foreground, prompt stops from keys and signals, a stop that cancels runs in flight, and start-up stops and failures."""

import os
import signal
import sqlite3
import subprocess
import sys
import termios
import time
from pathlib import Path
from uuid import uuid4

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    PUBLISHABLE_FILES,
    duckshop_orchestration,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentActiveRunStopTestCase,
    DevelopmentKeyStopTestCase,
    DevelopmentSignalStopTestCase,
    DevelopmentStartFailureTestCase,
    DevelopmentStartupStopTestCase,
    DevelopmentTerminalTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    development_environment,
    drain_terminal,
    processes_naming,
    read_terminal_until,
    reap_development,
    start_development_on_terminal,
    statement_started,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

SIGN_IN_ADAPTER: str = (
    "import getpass, os, sys\nfrom pathlib import Path\n"
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class DuckDbSignInAdapter(DuckDbAdapter):\n"
    "    adapter_name = 'duckdb_sign_in'\n\n"
    "    def connect(self, config):\n"
    "        with Path(os.environ['ORDERS_CONNECTS']).open('a') as connects:\n"
    "            connects.write(f'{os.getpid()}\\n')\n"
    "        token = Path(os.environ['ORDERS_SIGN_IN_TOKEN'])\n"
    "        if not token.exists():\n"
    "            code = READ_CODE.strip()\n"
    "            with Path(os.environ['ORDERS_SIGN_INS']).open('a') as sign_ins:\n"
    "                sign_ins.write(f'terminal={os.isatty(0)} code={code}\\n')\n"
    "            token.write_text(code)\n"
    "        return super().connect(config)\n"
)

# Reads the code from stdin, after writing its prompt to stderr.
READ_STDIN: str = (
    "(sys.stderr.write('Sign-in code: '), sys.stderr.flush(), sys.stdin.readline())[2]"
)
# Reads the code from /dev/tty with echo off, as getpass-style prompts do.
READ_TTY: str = "getpass.getpass('Sign-in code: ')"

# The location's runtime ignores SIGTERM, so only the kill fallback can stop it.
BUSY_LOCATION: str = (
    "import signal, sys, time\n"
    "if 'location' in sys.argv and 'runtime' in sys.argv:\n"
    "    signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
    "    time.sleep(300)\n"
)

# DuckDB whose statement cancellation leaves a mark, so a test sees the cancel path run.
CANCEL_MARKING_ADAPTER: str = (
    "import os\nfrom pathlib import Path\n"
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class DuckDbCancelMarkingAdapter(DuckDbAdapter):\n"
    "    adapter_name = 'duckdb_cancel_marking'\n\n"
    "    def interrupt_connection(self, connection):\n"
    "        with Path(os.environ['ORDERS_CANCELS']).open('a') as cancels:\n"
    "            cancels.write(f'{os.getpid()}\\n')\n"
    "        return super().interrupt_connection(connection)\n"
)

# A model whose one statement runs until it is cancelled.
ENDLESS_ORDERS: str = (
    "MODEL (description 'Typed orders.',\n"
    "  columns (order_id (type INTEGER), amount (type BIGINT)),\n"
    ");\n\n"
    "SELECT CAST(1 AS INTEGER) AS order_id, CAST(sum(sin(value)) AS BIGINT) AS amount\n"
    "FROM range(10000000000) t(value)\n"
)

DEVELOPMENT_PROJECT: str = (
    'default_target = "development"\n'
    + PUBLISHABLE_FILES["sqlbuild_project.toml"]
    + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
    + '[targets.development]\nschema = "main"\n'
)


NO_PLAIN: frozenset[str] = frozenset({"CI", "SQLBUILD_NO_PROGRESS"})


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX pseudo-terminal contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentTerminalTestCase(
            "the live screen passes the typed code to the client's own terminal, without echo",
            [],
            NO_PLAIN,
            READ_STDIN,
            "─ ready ",
            ["terminal=True code=424242"],
            0,
        ),
        DevelopmentTerminalTestCase(
            "the live screen's terminal is the client's controlling terminal, so /dev/tty works",
            [],
            NO_PLAIN,
            READ_TTY,
            "─ ready ",
            ["terminal=True code=424242"],
            0,
        ),
        DevelopmentTerminalTestCase(
            "--plain still signs in on the terminal before ready; only the terminal echoes stdin",
            ["--plain"],
            NO_PLAIN,
            READ_STDIN,
            "✓ ready  http",
            ["terminal=True code=424242"],
            1,
        ),
        DevelopmentTerminalTestCase(
            "--plain never prints the code a /dev/tty prompt reads",
            ["--plain"],
            NO_PLAIN,
            READ_TTY,
            "✓ ready  http",
            ["terminal=True code=424242"],
            0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_interactive_adapter_when_developing_then_sign_in_happens_once_in_the_foreground(
    test_case: DevelopmentTerminalTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    sign_ins: Path = tmp_path / "sign_ins.txt"
    connects: Path = tmp_path / "connects.txt"
    write_files(
        project_dir=project,
        files=PUBLISHABLE_FILES
        | {
            "sqlbuild_project.toml": DEVELOPMENT_PROJECT.replace(
                'adapter = "duckdb"', 'adapter = "duckdb_sign_in"'
            ),
            "adapters/duckdb_sign_in.py": SIGN_IN_ADAPTER.replace("READ_CODE", test_case.read_code),
        },
    )
    environment: dict[str, str] = development_environment(
        {
            "ORDERS_SIGN_IN_TOKEN": str(tmp_path / "token"),
            "ORDERS_SIGN_INS": str(sign_ins),
            "ORDERS_CONNECTS": str(connects),
        },
        test_case.unset,
    )
    terminal, process = start_development_on_terminal(project, test_case.arguments, environment)
    try:
        prompted: str = read_terminal_until(
            terminal, process, lambda shown: "Sign-in code:" in shown
        )
        _ = os.write(terminal, b"424242\r")
        shown: str = prompted + read_terminal_until(
            terminal, process, lambda shown: test_case.expected_ready in shown
        )
        shown += read_terminal_until(
            terminal, process, lambda _: len(set(connects.read_text().split())) >= 2
        )
    finally:
        process.send_signal(signal.SIGTERM)
        exit_code: int = process.wait(timeout=15)
        os.close(terminal)

    assert sign_ins.read_text().splitlines() == test_case.expected_sign_ins, shown
    assert shown.count("424242") == test_case.expected_code_shown, shown
    assert exit_code == 128 + signal.SIGTERM, shown


@pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="reads /proc for leftover processes"
)
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentKeyStopTestCase(
            "Ctrl+C read as a byte is an interrupt",
            b"\x03",
            130,
            "sqb dev stopped (interrupted)",
        ),
        DevelopmentKeyStopTestCase("q quits", b"q", 0, "sqb dev stopped  ran"),
    ],
    ids=lambda case: case.description,
)
def test_given_the_ready_screen_when_a_stop_key_is_typed_then_it_exits_promptly_without_orphans(
    test_case: DevelopmentKeyStopTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=PUBLISHABLE_FILES | {"sqlbuild_project.toml": DEVELOPMENT_PROJECT},
    )
    terminal, process = start_development_on_terminal(
        project, [], development_environment({}, NO_PLAIN)
    )
    try:
        shown: str = read_terminal_until(terminal, process, lambda shown: "\u2500 ready " in shown)
        stopped_at: float = time.monotonic()
        _ = os.write(terminal, test_case.keys)
        exit_code: int = process.wait(timeout=10)
        stopped_in: float = time.monotonic() - stopped_at
        shown += drain_terminal(terminal)
        local_modes: int = termios.tcgetattr(terminal)[3]
    finally:
        left: list[str] = reap_development(process, project)
        os.close(terminal)

    assert exit_code == test_case.expected_exit_code, shown
    assert stopped_in < 3, f"{stopped_in:.2f}s\n{shown}"
    assert left == [], shown
    assert local_modes & termios.ICANON and local_modes & termios.ECHO, shown
    assert test_case.expected_result in shown, shown


@pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="reads /proc for leftover processes"
)
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentSignalStopTestCase(
            "SIGTERM kills the busy interpreter that ignores it",
            signal.SIGTERM,
            143,
            "sqb dev stopped (terminated)",
        ),
        DevelopmentSignalStopTestCase(
            "SIGINT interrupts it, killing the interpreter too",
            signal.SIGINT,
            130,
            "sqb dev stopped (interrupted)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_busy_code_location_when_signalled_then_it_exits_promptly_without_orphans(
    test_case: DevelopmentSignalStopTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    hooks: Path = tmp_path / "hooks"
    write_files(
        project_dir=project,
        files=PUBLISHABLE_FILES | {"sqlbuild_project.toml": DEVELOPMENT_PROJECT},
    )
    write_files(project_dir=hooks, files={"sitecustomize.py": BUSY_LOCATION})
    python_path: str = os.pathsep.join([str(hooks), os.environ.get("PYTHONPATH", "")])
    terminal, process = start_development_on_terminal(
        project, [], development_environment({"PYTHONPATH": python_path}, NO_PLAIN)
    )
    try:
        shown: str = read_terminal_until(
            terminal,
            process,
            lambda _: any(" runtime " in line for line in processes_naming(project)),
        )
        stopped_at: float = time.monotonic()
        process.send_signal(test_case.stop)
        exit_code: int = process.wait(timeout=10)
        stopped_in: float = time.monotonic() - stopped_at
        shown += drain_terminal(terminal)
        local_modes: int = termios.tcgetattr(terminal)[3]
    finally:
        left: list[str] = reap_development(process, project)
        os.close(terminal)

    assert exit_code == test_case.expected_exit_code, shown
    assert stopped_in < 3, f"{stopped_in:.2f}s\n{shown}"
    assert left == [], shown
    assert local_modes & termios.ICANON and local_modes & termios.ECHO, shown
    assert test_case.expected_result in shown, shown


@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentStartFailureTestCase(
            "an unknown key is named with its suggestion, and nothing reports a successful stop",
            DEVELOPMENT_PROJECT + 'schemaa = "x"\n',
            2,
            [
                "sqb dev did not start  configuration error in sqlbuild_project.toml",
                "unknown key(s): schemaa",
                "did you mean 'schema'?",
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_invalid_configuration_when_developing_then_it_does_not_start_and_says_why(
    test_case: DevelopmentStartFailureTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=PUBLISHABLE_FILES | {"sqlbuild_project.toml": test_case.configuration},
    )

    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "dev",
            "--project-dir",
            str(project),
            "--port",
            "0",
        ],
        capture_output=True,
        text=True,
        timeout=60,
        stdin=subprocess.DEVNULL,
        env=development_environment({}, frozenset()),
    )

    assert completed.returncode == test_case.expected_exit_code, completed.stderr
    missing: list[str] = list(
        filter(lambda line: line not in completed.stderr, test_case.expected_lines)
    )
    assert missing == [], completed.stderr
    assert "stopped successfully" not in completed.stderr, completed.stderr


@pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="reads /proc for leftover processes"
)
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentStartupStopTestCase(
            "Ctrl+C while the control store is locked",
            b"\x03",
            130,
            "sqb dev stopped (interrupted) before it was ready",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_start_up_step_waiting_when_stopped_then_it_stops_cleanly_before_it_was_ready(
    test_case: DevelopmentStartupStopTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=PUBLISHABLE_FILES | {"sqlbuild_project.toml": DEVELOPMENT_PROJECT},
    )
    (project / ".sqlbuild").mkdir()
    holder: sqlite3.Connection = sqlite3.connect(
        project / ".sqlbuild/control.sqlite3", isolation_level=None
    )
    _ = holder.execute("BEGIN EXCLUSIVE")
    terminal, process = start_development_on_terminal(
        project, [], development_environment({}, NO_PLAIN)
    )
    try:
        shown: str = read_terminal_until(
            terminal, process, lambda shown: "Opening control store" in shown
        )
        time.sleep(0.5)
        stopped_at: float = time.monotonic()
        _ = os.write(terminal, test_case.keys)
        exit_code: int = process.wait(timeout=10)
        stopped_in: float = time.monotonic() - stopped_at
        shown += drain_terminal(terminal)
    finally:
        holder.close()
        left: list[str] = reap_development(process, project)
        os.close(terminal)

    assert exit_code == test_case.expected_exit_code, shown
    assert stopped_in < 3, f"{stopped_in:.2f}s\n{shown}"
    assert test_case.expected_result in shown, shown
    assert "(forced)" not in shown, shown
    assert left == [], shown


@pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="reads /proc for leftover processes"
)
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentActiveRunStopTestCase(
            "Ctrl+C cancels the running build through its worker before stopping",
            b"\x03",
            130,
            [
                "Cancelling run annual_orders_job… 1 query (Ctrl+C again to force)",
                "Cancelled run annual_orders_job  1 query cancelled",
                "sqb dev stopped (interrupted)",
            ],
            "cancelled",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_build_when_stopped_then_its_worker_cancels_it_and_releases_its_claims(
    test_case: DevelopmentActiveRunStopTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    cancels: Path = tmp_path / "cancels.txt"
    write_files(
        project_dir=project,
        files=PUBLISHABLE_FILES
        | {
            "sqlbuild_project.toml": DEVELOPMENT_PROJECT.replace(
                'adapter = "duckdb"', 'adapter = "duckdb_cancel_marking"'
            ),
            "adapters/duckdb_cancel_marking.py": CANCEL_MARKING_ADAPTER,
            "models/orders.sql": ENDLESS_ORDERS,
            **duckshop_orchestration("development_orders"),
        },
    )
    environment: dict[str, str] = development_environment(
        {"ORDERS_CANCELS": str(cancels)}, NO_PLAIN
    )
    control: Path = project / ".sqlbuild/control.sqlite3"
    run_id: str = str(uuid4())
    terminal, process = start_development_on_terminal(project, [], environment)
    try:
        shown: str = read_terminal_until(terminal, process, lambda shown: "─ ready " in shown)
        with sqlite3.connect(control) as store:
            (location_id,) = store.execute(
                "SELECT location_id FROM sqb_development_locations"
            ).fetchone()
        requested: subprocess.CompletedProcess[str] = subprocess.run(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "admin",
                "request-run",
                str(location_id),
                "annual_orders_job",
                "--target",
                "development",
                "--run-id",
                run_id,
                "--reason",
                "A build to stop",
            ],
            env=environment | {"SQLBUILD_CONTROL_DATABASE_URL": str(control)},
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
        assert requested.returncode == 0, requested.stderr
        shown += read_terminal_until(
            terminal, process, lambda _: statement_started(control), timeout=90
        )
        _ = os.write(terminal, test_case.keys)
        exit_code: int = process.wait(timeout=40)
        shown += drain_terminal(terminal)
    finally:
        left: list[str] = reap_development(process, project)
        os.close(terminal)
    with sqlite3.connect(control) as store:
        (status,) = store.execute(
            "SELECT status FROM sqb_runs WHERE run_id = ?", (run_id,)
        ).fetchone()
        claims: list[tuple[str]] = store.execute(
            "SELECT node_key FROM sqb_write_claims WHERE run_id = ?", (run_id,)
        ).fetchall()

    assert exit_code == test_case.expected_exit_code, shown
    assert list(filter(lambda line: line not in shown, test_case.expected_lines)) == [], shown
    assert "(forced)" not in shown, shown
    assert status == test_case.expected_status, shown
    assert claims == [], shown
    assert cancels.read_text().split() != [], shown
    assert left == [], shown
