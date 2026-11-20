"""`sqb dev` on a real terminal: sign-in in the foreground, prompt stops from keys and signals, and start-up failures."""

import os
import signal
import subprocess
import sys
import termios
import time
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import PUBLISHABLE_FILES
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentKeyStopTestCase,
    DevelopmentSignalStopTestCase,
    DevelopmentStartFailureTestCase,
    DevelopmentTerminalTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    development_environment,
    drain_terminal,
    processes_naming,
    read_terminal_until,
    reap_development,
    start_development_on_terminal,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

SIGN_IN_ADAPTER: str = (
    "import os, sys\nfrom pathlib import Path\n"
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class DuckDbSignInAdapter(DuckDbAdapter):\n"
    "    adapter_name = 'duckdb_sign_in'\n\n"
    "    def connect(self, config):\n"
    "        with Path(os.environ['ORDERS_CONNECTS']).open('a') as connects:\n"
    "            connects.write(f'{os.getpid()}\\n')\n"
    "        token = Path(os.environ['ORDERS_SIGN_IN_TOKEN'])\n"
    "        if not token.exists():\n"
    "            sys.stderr.write('Sign-in code: ')\n"
    "            sys.stderr.flush()\n"
    "            code = sys.stdin.readline().strip()\n"
    "            with Path(os.environ['ORDERS_SIGN_INS']).open('a') as sign_ins:\n"
    "                sign_ins.write(f'terminal={os.isatty(0)} code={code}\\n')\n"
    "            token.write_text(code)\n"
    "        return super().connect(config)\n"
)

BUSY_LOCATION: str = (
    "import sys, time\nif 'location' in sys.argv and 'runtime' in sys.argv:\n    time.sleep(300)\n"
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
            "the live screen passes the typed code to the client's own terminal",
            [],
            NO_PLAIN,
            "─ ready ",
            ["terminal=True code=424242"],
        ),
        DevelopmentTerminalTestCase(
            "--plain still signs in on the terminal before ready",
            ["--plain"],
            NO_PLAIN,
            "✓ ready  http",
            ["terminal=True code=424242"],
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
            "adapters/duckdb_sign_in.py": SIGN_IN_ADAPTER,
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
    assert stopped_in < 2, f"{stopped_in:.2f}s\n{shown}"
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
            "SIGTERM terminates the busy interpreter too",
            signal.SIGTERM,
            143,
            "sqb dev stopped (terminated)",
        ),
        DevelopmentSignalStopTestCase(
            "SIGINT interrupts it",
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
    assert stopped_in < 2, f"{stopped_in:.2f}s\n{shown}"
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
