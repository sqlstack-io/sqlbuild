"""Python CLI entrypoint the native `sqb` executable hands every invocation it does not serve."""

import os
import sys

from sqlbuild.cli.entry.classes.interrupt_watch import InterruptWatch
from sqlbuild.cli.entry.constants import (
    NATIVE_ALLOCATOR_PURGE_DELAY_MILLISECONDS,
    NATIVE_ALLOCATOR_PURGE_DELAY_VARIABLE,
)
from sqlbuild.cli.entry.main.entry import main as run_cli


def main() -> int:
    """Run the CLI with the `sqb` arguments in a process that exits once the command returns."""

    watch: InterruptWatch = InterruptWatch()
    watch.start()
    try:
        code: int = run_launched_command()
    except KeyboardInterrupt:
        return watch.finish_interrupted()
    except BaseException:
        if watch.interrupted():
            return watch.finish_interrupted()
        raise
    return watch.finish(code)


def run_launched_command() -> int:
    """Run the CLI with the arguments the native launcher handed over."""

    os.environ.setdefault(
        NATIVE_ALLOCATOR_PURGE_DELAY_VARIABLE, NATIVE_ALLOCATOR_PURGE_DELAY_MILLISECONDS
    )
    from sqlbuild._native import LAUNCH_RECORD_OPTION, defer_compile_teardown_to_exit

    _ = defer_compile_teardown_to_exit()
    arguments: list[str] = sys.argv[1:]
    offered: bool = bool(arguments) and arguments[0].startswith(LAUNCH_RECORD_OPTION)
    sys.argv = [sys.argv[0], *arguments[int(offered) :]]
    return run_cli(arguments)
