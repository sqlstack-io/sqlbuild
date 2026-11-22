"""Ctrl+C in the `sqb` process: exit 130, also where the interrupt can't unwind the command."""

import contextlib
import os
import select
import signal
import socket
import sys
import threading
import time
from collections.abc import Callable
from types import FrameType

from sqlbuild.cli.entry._helpers.connecting import connects_in_progress
from sqlbuild.cli.entry._helpers.terminal import terminal_restorer
from sqlbuild.cli.entry.constants import (
    FORCED_EXIT_WAIT_SECONDS,
    INTERRUPT_GRACE_SECONDS,
    INTERRUPT_POLL_SECONDS,
    INTERRUPTED_EXIT_CODE,
    INTERRUPTED_LINE,
    TERMINATED_EXIT_CODE,
)
from sqlbuild.runtime.execution_limits.main._cancel_running_statements import (
    cancel_running_statements,
)
from sqlbuild.runtime.execution_limits.main.statement_activity import statement_activity


class InterruptWatch:
    """Follow SIGINT through Python's wakeup descriptor, leaving its handlers as they are.

    The first Ctrl+C cancels the statements running then, as SIGTERM does, so a driver waiting in
    native code returns and the interrupt unwinds the command, which cleans up. The exit is forced
    after a grace period only where that can't cut warehouse cleanup short: a connect that was
    waiting when Ctrl+C came still is, no statement has run, or the command has returned and
    none is running. A second Ctrl+C forces it after the same grace, cancelling statements first.
    """

    def __init__(self) -> None:
        self._lock: threading.Lock = threading.Lock()
        self._presses: int = 0
        self._pressed_at: float = 0.0
        self._connecting: list[FrameType] = []
        self._returned: bool = False
        self._terminated: bool = False
        self._announced: bool = False
        self._restore_terminal: Callable[[], None] = lambda: None
        self._signals: tuple[socket.socket, socket.socket] | None = None

    def start(self) -> None:
        """Start watching, unless Ctrl+C is ignored or handled by someone else."""

        if threading.current_thread() is not threading.main_thread():
            return
        if signal.getsignal(signal.SIGINT) is not signal.default_int_handler:
            return
        with contextlib.suppress(OSError):
            self._restore_terminal = terminal_restorer()
        receiver, sender = socket.socketpair()
        receiver.setblocking(False)
        sender.setblocking(False)
        self._signals = (receiver, sender)
        _ = signal.set_wakeup_fd(sender.fileno(), warn_on_full_buffer=False)
        if sys.platform != "win32":
            os.register_at_fork(after_in_child=self._forget_in_child)
        threading.Thread(target=self._watch, name="sqlbuild-interrupt", daemon=True).start()

    def interrupted(self) -> bool:
        """Whether Ctrl+C has arrived."""

        self._receive()
        return self._presses > 0

    def finish(self, code: int) -> int:
        """Return the exit code of a command that returned `code`."""

        if code == TERMINATED_EXIT_CODE:
            self._terminated = True
            return code
        self._returned = True
        return self.finish_interrupted() if self.interrupted() else code

    def finish_interrupted(self) -> int:
        """Return 130 for a command Ctrl+C stopped, saying so."""

        self._returned = True
        self._announce()
        return INTERRUPTED_EXIT_CODE

    def _forget_in_child(self) -> None:
        _ = signal.set_wakeup_fd(-1)

    def _receive(self) -> None:
        if self._signals is None:
            return
        with self._lock:
            try:
                received: bytes = self._signals[0].recv(64)
            except BlockingIOError:
                return
            if signal.SIGINT not in received or self._terminated:
                return
            if self._termination_requested():
                self._terminated = True
                return
            if not self._presses:
                self._connecting = connects_in_progress()
                if statement_activity()[1]:
                    threading.Thread(target=cancel_running_statements, daemon=True).start()
            self._presses += 1
            self._pressed_at = time.monotonic()

    def _termination_requested(self) -> bool:
        native: object = sys.modules.get("sqlbuild._native")
        requested: Callable[[], bool] | None = getattr(native, "termination_requested", None)
        return requested is not None and requested()

    def _watch(self) -> None:
        assert self._signals is not None
        while True:
            _ = select.select([self._signals[0]], [], [], self._poll_seconds())
            self._receive()
            if self._may_force():
                self._force()

    def _poll_seconds(self) -> float | None:
        return INTERRUPT_POLL_SECONDS if self._presses else None

    def _may_force(self) -> bool:
        if self._terminated or not self._presses:
            return False
        if time.monotonic() - self._pressed_at < INTERRUPT_GRACE_SECONDS:
            return False
        if self._presses > 1:
            return True
        if self._connecting:
            current: list[FrameType] = connects_in_progress()
            if any(frame in current for frame in self._connecting):
                return True
        ever_ran, running = statement_activity()
        return not ever_ran or (self._returned and not running)

    def _force(self) -> None:
        stopping: threading.Thread = threading.Thread(target=self._stop_work, daemon=True)
        stopping.start()
        stopping.join(FORCED_EXIT_WAIT_SECONDS)
        with contextlib.suppress(Exception):
            self._restore_terminal()
        self._announce()
        os._exit(INTERRUPTED_EXIT_CODE)

    def _stop_work(self) -> None:
        try:
            cancel_running_statements()
        finally:
            _ = sys.stdout.flush()

    def _announce(self) -> None:
        if self._announced:
            return
        self._announced = True
        with contextlib.suppress(OSError):
            lead: str = "\n" if os.isatty(2) else ""
            _ = os.write(2, f"{lead}{INTERRUPTED_LINE}\n".encode())
