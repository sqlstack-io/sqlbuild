"""Public executor entrypoint to end a process whose cancelled step an exit would wait for."""

from __future__ import annotations

import atexit
import os
import sys
import threading

from sqlbuild.executor.python_nodes.constants import ABANDONABLE_THREAD_PREFIX


def exit_past_abandoned_steps(*, exit_code: int, last_line: str | None = None) -> None:
    """End at once after the exit hooks and `last_line`, if an item's thread outlived its pool."""

    if not any(
        thread.name.startswith(ABANDONABLE_THREAD_PREFIX) for thread in threading.enumerate()
    ):
        return
    atexit._run_exitfuncs()
    _ = sys.stdout.flush()
    if last_line is not None:
        _ = sys.stderr.write(f"{last_line}\n")
    _ = sys.stderr.flush()
    os._exit(exit_code)
