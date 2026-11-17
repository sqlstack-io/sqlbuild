"""Interrupt the main thread once an observed event happens, the way Ctrl-C does."""

import _thread
import contextlib
import signal
import threading
from collections.abc import Callable, Iterator


@contextlib.contextmanager
def default_interrupt_handling() -> Iterator[None]:
    """Give the main thread Python's default Ctrl-C handling while the block runs.

    Test runners often start with SIGINT ignored; `_thread.interrupt_main` then does nothing."""

    previous: object = signal.signal(signal.SIGINT, signal.default_int_handler)
    try:
        yield
    finally:
        _ = signal.signal(signal.SIGINT, previous)


@contextlib.contextmanager
def interrupt_when(ready: Callable[[], bool]) -> Iterator[threading.Event]:
    """Interrupt the main thread once `ready()` holds.

    The event is set when the interrupt was sent; leaving the block stops the watcher, so no
    interrupt arrives after it."""

    sent: threading.Event = threading.Event()
    stop: threading.Event = threading.Event()
    lock: threading.Lock = threading.Lock()

    def watch() -> None:
        while not stop.wait(0.02):
            if ready():
                with lock:
                    if not stop.is_set():
                        sent.set()
                        _thread.interrupt_main()
                return

    watcher: threading.Thread = threading.Thread(target=watch, daemon=True)
    watcher.start()
    try:
        yield sent
    finally:
        with lock:
            stop.set()
        watcher.join()
