"""Wrap one timed compile in a memory-limited transient systemd scope."""

import os
import shutil
import uuid

from scripts.cold_compile_performance.constants import COMPILE_MEMORY_LIMIT_ENV_VAR
from scripts.cold_compile_performance.exceptions import CgroupMemoryLimitError


def compile_memory_scope_command(*, label: str) -> tuple[str, ...]:
    """Prefix for a scope with the configured per-compile limit; refuse to run without it."""

    raw_limit: str | None = os.environ.get(COMPILE_MEMORY_LIMIT_ENV_VAR)
    if raw_limit is None:
        return ()
    unit: str = f"sqlbuild-compile-{label}-{os.getpid()}-{uuid.uuid4().hex[:8]}"
    systemd_run: str | None = shutil.which("systemd-run")
    sudo: str | None = shutil.which("sudo")
    caller_path_over_sudo_secure_path: str = f"PATH={os.environ.get('PATH', '')}"
    if systemd_run is None or sudo is None:
        raise CgroupMemoryLimitError(
            f"{COMPILE_MEMORY_LIMIT_ENV_VAR}={raw_limit} requires systemd-run and sudo to start "
            f"the transient scope {unit}.scope; refusing to compile without the memory limit"
        )
    return (
        sudo,
        "--non-interactive",
        "--preserve-env",
        systemd_run,
        "--scope",
        "--quiet",
        "--collect",
        f"--unit={unit}",
        f"--uid={os.getuid()}",
        f"--gid={os.getgid()}",
        f"--property=MemoryMax={int(raw_limit)}",
        "--property=MemorySwapMax=0",
        "--",
        "/usr/bin/env",
        caller_path_over_sudo_secure_path,
    )
