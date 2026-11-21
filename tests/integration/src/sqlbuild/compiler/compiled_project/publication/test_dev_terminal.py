"""`sqb dev` on a real terminal: sign-in in the foreground, prompt stops from keys and signals, a stop that cancels runs in flight, start-up stops and failures, and `r` reloading the env file."""

import os
import re
import signal
import sqlite3
import subprocess
import sys
import termios
import time
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    PUBLISHABLE_FILES,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentActiveRunStopTestCase,
    DevelopmentEnvFileTestCase,
    DevelopmentHangUpTestCase,
    DevelopmentKeyStopTestCase,
    DevelopmentMissingSecretsTestCase,
    DevelopmentSignalStopTestCase,
    DevelopmentStartFailureTestCase,
    DevelopmentStartupStopTestCase,
    DevelopmentTerminalTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    REGION_PROVIDER_FILES,
    development_environment,
    development_run_state,
    drain_terminal,
    endless_build_files,
    latest_redraw,
    linked_project,
    processes_naming,
    read_terminal_until,
    reap_development,
    request_development_run,
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
            "the live screen passes the typed code to the client's own terminal, which echoes it",
            [],
            NO_PLAIN,
            READ_STDIN,
            "─ ready ",
            ["terminal=True code=424242"],
            True,
        ),
        DevelopmentTerminalTestCase(
            "the live screen's terminal is the client's controlling terminal; getpass hides the code",
            [],
            NO_PLAIN,
            READ_TTY,
            "─ ready ",
            ["terminal=True code=424242"],
            False,
        ),
        DevelopmentTerminalTestCase(
            "--plain still signs in on the terminal before ready, which echoes a plain prompt",
            ["--plain"],
            NO_PLAIN,
            READ_STDIN,
            "✓ ready  http",
            ["terminal=True code=424242"],
            True,
        ),
        DevelopmentTerminalTestCase(
            "--plain never prints the code a getpass prompt reads",
            ["--plain"],
            NO_PLAIN,
            READ_TTY,
            "✓ ready  http",
            ["terminal=True code=424242"],
            False,
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
    assert ("424242" in shown) == test_case.expected_code_shown, shown
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
                "Cancelling run annual_orders_job… 1 running node (Ctrl+C again to force)",
                "Cancelled run annual_orders_job  1 running node cancelled",
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
    write_files(project_dir=project, files=endless_build_files(DEVELOPMENT_PROJECT))
    environment: dict[str, str] = development_environment(
        {"ORDERS_CANCELS": str(cancels)}, NO_PLAIN
    )
    terminal, process = start_development_on_terminal(project, [], environment)
    try:
        shown: str = read_terminal_until(terminal, process, lambda shown: "\u2500 ready " in shown)
        control, run_id = request_development_run(project, environment)
        shown += read_terminal_until(
            terminal, process, lambda _: statement_started(control), timeout=90
        )
        _ = os.write(terminal, test_case.keys)
        exit_code: int = process.wait(timeout=40)
        shown += drain_terminal(terminal)
    finally:
        left: list[str] = reap_development(process, project)
        os.close(terminal)

    assert exit_code == test_case.expected_exit_code, shown
    assert list(filter(lambda line: line not in shown, test_case.expected_lines)) == [], shown
    assert "(forced)" not in shown, shown
    assert development_run_state(control, run_id) == (test_case.expected_status, []), shown
    assert cancels.read_text().split() != [], shown
    assert left == [], shown


@pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="reads /proc for leftover processes"
)
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentHangUpTestCase(
            "closing the terminal cancels the running build, as SIGTERM does",
            129,
            "cancelled",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_running_build_when_the_terminal_closes_then_its_worker_still_cancels_it(
    test_case: DevelopmentHangUpTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    cancels: Path = tmp_path / "cancels.txt"
    write_files(project_dir=project, files=endless_build_files(DEVELOPMENT_PROJECT))
    environment: dict[str, str] = development_environment(
        {"ORDERS_CANCELS": str(cancels)}, NO_PLAIN
    )
    terminal, process = start_development_on_terminal(project, [], environment)
    try:
        shown: str = read_terminal_until(terminal, process, lambda shown: "\u2500 ready " in shown)
        control, run_id = request_development_run(project, environment)
        shown += read_terminal_until(
            terminal, process, lambda _: statement_started(control), timeout=90
        )
    finally:
        os.close(terminal)
    try:
        exit_code: int = process.wait(timeout=40)
    finally:
        left: list[str] = reap_development(process, project)

    assert exit_code == test_case.expected_exit_code, shown
    assert development_run_state(control, run_id) == (test_case.expected_status, []), shown
    assert cancels.read_text().split() != [], shown
    assert left == [], shown


ENV_PROBE: str = (
    "import os\nfrom pathlib import Path\n\n"
    "with Path(os.environ['ORDERS_ENV_PROBES']).open('a') as probes:\n"
    "    probes.write(os.environ.get('ORDERS_ENV_PROBE', 'unset') + '\\n')\n"
)


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX pseudo-terminal contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentEnvFileTestCase(
            "an edited env file reaches the interpreter after r",
            "ORDERS_ENV_PROBE=first\n",
            "ORDERS_ENV_PROBE=second\n",
            ".env \u00b7 1 variable \u00b7 edit it, then press r to reload",
            ("location restarted", "env file changed"),
        ),
        DevelopmentEnvFileTestCase(
            "a symlinked project names its env file relative to the project",
            "ORDERS_ENV_PROBE=first\n",
            "ORDERS_ENV_PROBE=second\n",
            ".env \u00b7 1 variable \u00b7 edit it, then press r to reload",
            ("location restarted", "env file changed"),
            project_argument=linked_project,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_env_file_when_developing_then_start_up_names_it_and_r_reloads_it(
    test_case: DevelopmentEnvFileTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    probes: Path = tmp_path / "probes.txt"
    write_files(
        project_dir=project,
        files=PUBLISHABLE_FILES
        | {
            "sqlbuild_project.toml": 'default_target = "development"\n'
            + PUBLISHABLE_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
            "python/env_probe.py": ENV_PROBE,
        },
    )
    env_file: Path = project / ".env"
    _ = env_file.write_text(test_case.first_env_file)
    env_file.chmod(0o600)
    environment: dict[str, str] = development_environment(
        {"ORDERS_ENV_PROBES": str(probes)}, NO_PLAIN
    )
    served: Path = test_case.project_argument(project)
    terminal, process = start_development_on_terminal(served, [], environment)
    try:
        shown: str = read_terminal_until(terminal, process, lambda shown: "\u2500 ready " in shown)
        _ = env_file.write_text(test_case.second_env_file)
        _ = os.write(terminal, b"r")
        shown += read_terminal_until(
            terminal,
            process,
            lambda _: probes.exists() and "second" in probes.read_text().split(),
        )
        process.send_signal(signal.SIGTERM)
        exit_code: int = process.wait(timeout=15)
        shown += drain_terminal(terminal)
    finally:
        left: list[str] = reap_development(process, served)
        os.close(terminal)

    plain: str = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", shown)
    assert test_case.expected_card_row in plain, plain
    restart_line: str = next(filter(lambda line: "location restarted" in line, plain.splitlines()))
    assert all(part in restart_line for part in test_case.expected_restart), plain
    assert probes.read_text().split()[0] == "first", probes.read_text()
    assert exit_code == 128 + signal.SIGTERM, shown
    assert left == [], shown


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX pseudo-terminal contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentMissingSecretsTestCase(
            "a provider missing secrets is warned about until .env sets them and r reloads it",
            "UNRELATED_SETTING=1\n",
            "WORKER_ENV_REGION_CODE=42\nWORKER_ENV_TOKEN=worker-token\n",
            (
                "Secrets missing",
                "region WORKER_ENV_TOKEN, WORKER_ENV_REGION_CODE",
                "set them in .env or the environment; press r to reload",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_provider_missing_secrets_when_developing_then_a_warning_names_them_until_reloaded(
    test_case: DevelopmentMissingSecretsTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\ndefault_target = "development"\n'
                '[connections.local]\ndatabase = "warehouse.duckdb"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n"
                'SELECT CAST(code AS BIGINT) AS amount FROM __source("raw_regions")\n'
            ),
        }
        | REGION_PROVIDER_FILES,
    )
    env_file: Path = project / ".env"
    _ = env_file.write_text(test_case.first_env_file)
    env_file.chmod(0o600)
    terminal, process = start_development_on_terminal(
        project, ["--no-scheduler"], development_environment({}, NO_PLAIN)
    )
    try:
        warned: str = read_terminal_until(
            terminal,
            process,
            lambda shown: "\u2500 ready " in shown and "press r to reload" in latest_redraw(shown),
        )
        _ = env_file.write_text(test_case.second_env_file)
        _ = os.write(terminal, b"r")
        reloaded: str = read_terminal_until(
            terminal,
            process,
            lambda shown: (
                "env file changed" in shown
                and "keys " in latest_redraw(shown)
                and "Secrets missing" not in latest_redraw(shown)
            ),
        )
        process.send_signal(signal.SIGTERM)
        exit_code: int = process.wait(timeout=15)
    finally:
        left: list[str] = reap_development(process, project)
        os.close(terminal)

    warning: str = " ".join(latest_redraw(warned).split())
    assert all(" ".join(part.split()) in warning for part in test_case.expected_warning), warning
    assert "worker-token" not in warned + reloaded
    assert exit_code == 128 + signal.SIGTERM, reloaded
    assert left == [], reloaded
