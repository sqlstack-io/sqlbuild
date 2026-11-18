"""`sqb dev` on a real terminal: the client signs in once, in the foreground; later connections reuse it."""

import os
import pty
import signal
import subprocess
import sys
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import PUBLISHABLE_FILES
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    DevelopmentTerminalTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    read_terminal_until,
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


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX pseudo-terminal contract")
@pytest.mark.parametrize(
    "test_case",
    [
        DevelopmentTerminalTestCase(
            "the live screen hands the terminal to the client while it prompts",
            {"TERM": "xterm"},
            frozenset({"NO_COLOR", "CI", "SQLBUILD_NO_PROGRESS"}),
            "ready in",
            ["terminal=True code=424242"],
        ),
        DevelopmentTerminalTestCase(
            "plain output still signs in on the terminal before ready",
            {"NO_COLOR": "1"},
            frozenset(),
            "sqlbuild dev: ready ",
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
            "sqlbuild_project.toml": 'default_target = "development"\n'
            + PUBLISHABLE_FILES["sqlbuild_project.toml"].replace(
                'adapter = "duckdb"', 'adapter = "duckdb_sign_in"'
            )
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '[targets.development]\nschema = "main"\n',
            "adapters/duckdb_sign_in.py": SIGN_IN_ADAPTER,
        },
    )
    environment: dict[str, str] = (
        dict(os.environ)
        | {
            "ORDERS_SIGN_IN_TOKEN": str(tmp_path / "token"),
            "ORDERS_SIGN_INS": str(sign_ins),
            "ORDERS_CONNECTS": str(connects),
        }
        | test_case.environment
    )
    environment = {
        key: environment[key]
        for key in environment.keys() - test_case.unset - {"SQLBUILD_PLATFORM_WEBHOOK_URL"}
    }
    executable: str = str(Path(sys.executable).with_name("sqb"))
    terminal, attached = pty.openpty()
    process: subprocess.Popen[bytes] = subprocess.Popen(
        [executable, "dev", "--project-dir", str(project), "--port", "0"],
        stdin=attached,
        stdout=attached,
        stderr=attached,
        env=environment,
        start_new_session=True,
    )
    os.close(attached)
    try:
        prompted: str = read_terminal_until(
            terminal, process, lambda shown: "Sign-in code: " in shown
        )
        _ = os.write(terminal, b"424242\n")
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
    assert exit_code == 0, shown
