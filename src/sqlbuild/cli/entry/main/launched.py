"""Python CLI entrypoint the native `sqb` executable hands every invocation it does not serve."""

import os
import sys

from sqlbuild.cli.entry.constants import (
    INTERRUPTED_EXIT_CODE,
    INTERRUPTED_LINE,
    NATIVE_ALLOCATOR_PURGE_DELAY_MILLISECONDS,
    NATIVE_ALLOCATOR_PURGE_DELAY_VARIABLE,
)
from sqlbuild.cli.entry.main.entry import main as run_cli


def main() -> int:
    """Run the CLI with the `sqb` arguments in a process that exits once the command returns."""

    os.environ.setdefault(
        NATIVE_ALLOCATOR_PURGE_DELAY_VARIABLE, NATIVE_ALLOCATOR_PURGE_DELAY_MILLISECONDS
    )
    try:
        from sqlbuild._native import (
            LAUNCH_RECORD_OPTION,
            defer_compile_teardown_to_exit,
            run_interruptible,
        )
        from sqlbuild.executor.python_nodes.main.exit_past_abandoned_steps import (
            exit_past_abandoned_steps,
        )
    except KeyboardInterrupt:
        _ = sys.stderr.write(f"{INTERRUPTED_LINE}\n")
        return INTERRUPTED_EXIT_CODE

    _ = defer_compile_teardown_to_exit()
    arguments: list[str] = sys.argv[1:]
    offered: bool = bool(arguments) and arguments[0].startswith(LAUNCH_RECORD_OPTION)
    sys.argv = [sys.argv[0], *arguments[int(offered) :]]

    def run_command() -> int:
        """Run the command, then end past a cancelled step still running, inside the watch."""

        try:
            exit_code: int = run_cli(arguments)
        except KeyboardInterrupt:
            _ = exit_past_abandoned_steps(
                exit_code=INTERRUPTED_EXIT_CODE, last_line=INTERRUPTED_LINE
            )
            raise
        _ = exit_past_abandoned_steps(exit_code=exit_code)
        return exit_code

    return run_interruptible(run_command)
