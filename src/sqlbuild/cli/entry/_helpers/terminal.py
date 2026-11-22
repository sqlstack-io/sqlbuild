"""The terminal's settings, which a forced exit puts back after a hidden-input prompt."""

import os
import sys
from collections.abc import Callable


def terminal_restorer() -> Callable[[], None]:
    """Return what restores the settings the terminal has now; nothing without a terminal."""

    if sys.platform == "win32":
        return lambda: None
    import termios

    for descriptor in (0, 2, 1):
        if os.isatty(descriptor):
            settings: list[object] = termios.tcgetattr(descriptor)
            return lambda: termios.tcsetattr(descriptor, termios.TCSADRAIN, settings)
    return lambda: None
