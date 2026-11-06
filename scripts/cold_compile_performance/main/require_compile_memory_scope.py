"""Require a per-compile memory limit when guards run in CI."""

import os

from scripts.cold_compile_performance.constants import COMPILE_MEMORY_LIMIT_ENV_VAR
from scripts.cold_compile_performance.exceptions import CgroupMemoryLimitError


def require_compile_memory_scope() -> None:
    """Refuse to run in CI unless each timed compile gets its own memory-limited scope."""

    if os.environ.get("CI") and COMPILE_MEMORY_LIMIT_ENV_VAR not in os.environ:
        raise CgroupMemoryLimitError(
            f"CI must set {COMPILE_MEMORY_LIMIT_ENV_VAR} so every timed compile runs in a "
            "memory-limited transient systemd scope"
        )
