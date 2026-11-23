"""Ctrl+C stops a command at any point, run by the real CLI on a pseudo-terminal."""

import sys
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    InterruptedCommandTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    interrupt_on_terminal,
    recorded_ending,
    write_prompting_project,
)

_LINE: str = "error: interrupted (Ctrl+C)"
_INPUT: str = '        input("Sign-in code: ")\n'
_SWALLOWED: str = (
    "        try:\n"
    '            input("Sign-in code: ")\n'
    "        except BaseException as error:\n"
    '            raise RuntimeError("sign-in failed") from error\n'
)
_RETRIED: str = (
    "        try:\n"
    '            input("Sign-in code: ")\n'
    "        except KeyboardInterrupt:\n"
    '            input("Sign-in code again: ")\n'
)
_HIDDEN: str = (
    "        try:\n"
    '            input("Sign-in code: ")\n'
    "        except KeyboardInterrupt:\n"
    '            getpass.getpass("Password: ")\n'
)
_THREADED: str = (
    "        with ThreadPoolExecutor(1) as pool:\n"
    '            pool.submit(input, "Sign-in code: ").result()\n'
)
_ENDLESS: str = (
    "SELECT count(*) AS amount FROM range(100000000) a, range(100000000) b "
    "WHERE (a.range * b.range) % 7 = 1"
)
_READING_ASSET: str = (
    "from concurrent.futures import ThreadPoolExecutor\n\n"
    "from sqlbuild.assets import asset\n"
    "from sqlbuild.refs import model\n\n\n"
    '@asset(depends_on=model("customers"))\n'
    "def reader(ctx):\n"
    '    """Read a line on another thread."""\n'
    "    with ThreadPoolExecutor(1) as pool:\n"
    '        pool.submit(input, "Value: ").result()\n'
)


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX pseudo-terminal contract")
@pytest.mark.parametrize(
    "test_case",
    [
        InterruptedCommandTestCase(
            "debug at a sign-in prompt", ("debug",), _INPUT, "Sign-in code:", _LINE
        ),
        InterruptedCommandTestCase(
            "build at a sign-in prompt", ("build",), _INPUT, "Sign-in code:", _LINE
        ),
        InterruptedCommandTestCase(
            "sqb dev at a sign-in prompt",
            ("dev", "--port", "0", "--no-scheduler"),
            _INPUT,
            "Sign-in code:",
            "sqb dev stopped (interrupted)",
        ),
        InterruptedCommandTestCase(
            "a driver that turns the interrupt into its own error",
            ("debug",),
            _SWALLOWED,
            "Sign-in code:",
            _LINE,
        ),
        InterruptedCommandTestCase(
            "a driver that asks again", ("debug",), _RETRIED, "Sign-in code:", _LINE
        ),
        InterruptedCommandTestCase(
            "a driver that asks again without echo", ("debug",), _HIDDEN, "Sign-in code:", _LINE
        ),
        InterruptedCommandTestCase(
            "a prompt read on another thread", ("build",), _THREADED, "Sign-in code:", _LINE
        ),
        InterruptedCommandTestCase(
            "a running build statement is cancelled",
            ("build", "--select", "orders"),
            "",
            "Warehouse connected",
            _LINE,
            settle_seconds=1.5,
            files={
                "models/orders.sql": (
                    f"MODEL (description 'Orders.', materialized table);\n{_ENDLESS}\n"
                )
            },
            expected_within_seconds=5.0,
        ),
        InterruptedCommandTestCase(
            "a running query is cancelled",
            ("query", _ENDLESS),
            "",
            "Running query",
            _LINE,
            settle_seconds=1.0,
            expected_within_seconds=5.0,
        ),
        InterruptedCommandTestCase(
            "a second press ends a read that outlives the statements before it",
            ("build",),
            "",
            "Value: ",
            _LINE,
            presses=2,
            files={"python/assets/reader.py": _READING_ASSET},
            expected_within_seconds=5.0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_waiting_command_when_interrupted_then_it_exits_130_promptly_and_says_so(
    test_case: InterruptedCommandTestCase, tmp_path: Path
) -> None:
    project: Path = write_prompting_project(
        tmp_path / "orders", connect=test_case.connect, files=test_case.files
    )

    exit_code, seconds, shown, echoes = interrupt_on_terminal(
        project,
        command=test_case.command,
        waiting=test_case.waiting,
        settle=test_case.settle_seconds,
        presses=test_case.presses,
    )

    assert exit_code == 130, shown
    assert seconds < test_case.expected_within_seconds, shown
    assert test_case.expected_line in shown, shown
    assert "Traceback" not in shown, shown
    assert echoes, shown


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX pseudo-terminal contract")
@pytest.mark.parametrize(
    "test_case",
    [
        InterruptedCommandTestCase(
            "an interrupt the driver turned into its own error",
            ("debug",),
            _SWALLOWED,
            "Sign-in code:",
            _LINE,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_swallowed_interrupt_when_the_command_ends_then_recall_records_it_cancelled(
    test_case: InterruptedCommandTestCase, tmp_path: Path
) -> None:
    project: Path = write_prompting_project(
        tmp_path / "orders", connect=test_case.connect, files=test_case.files
    )

    _, _, shown, _ = interrupt_on_terminal(
        project,
        command=test_case.command,
        waiting=test_case.waiting,
        settle=test_case.settle_seconds,
        presses=test_case.presses,
    )

    assert test_case.expected_line in shown, shown
    assert recorded_ending(project) == ["cancelled", 130, 2], shown
