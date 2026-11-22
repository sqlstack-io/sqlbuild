"""End a process whose cancelled step is still running, which a normal exit would wait for."""

from __future__ import annotations

import atexit
import os
import sys
import threading

from sqlbuild.runtime.execution_limits.constants import ABANDONABLE_THREAD_PREFIX


def exit_past_abandoned_steps(*, exit_code: int) -> None:
    """Run the exit hooks, flush and end at once, if a step's thread outlived its command."""

    if not any(
        thread.name.startswith(ABANDONABLE_THREAD_PREFIX) for thread in threading.enumerate()
    ):
        return
    atexit._run_exitfuncs()
    _ = sys.stdout.flush()
    _ = sys.stderr.flush()
    os._exit(exit_code)
