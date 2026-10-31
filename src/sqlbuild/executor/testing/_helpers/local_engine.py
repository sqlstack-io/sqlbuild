"""Find the SQLQuack executable and check its version."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing.constants import (
    LOCAL_TEST_ENGINE_CODE,
    LOCAL_TEST_INSTALL_HELP,
    SQLQUACK_EXECUTABLE_ENV,
    SQLQUACK_EXECUTABLE_NAME,
    SQLQUACK_MINIMUM_VERSION,
    SQLQUACK_VERSION_PATTERN,
    SQLQUACK_VERSION_TIMEOUT_SECONDS,
)


def session_value(value: object) -> str:
    """A Snowflake session parameter value as SQLQuack's `--session` reads it."""

    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value)


def find_sqlquack() -> Path:
    """The configured or PATH `sqlquack` executable."""

    configured: str | None = os.environ.get(SQLQUACK_EXECUTABLE_ENV)
    found: str | None = configured if configured else shutil.which(SQLQUACK_EXECUTABLE_NAME)
    if found is None or not Path(found).is_file():
        where: str = f"{SQLQUACK_EXECUTABLE_ENV}={configured}" if configured else "PATH"
        raise ExecutorInputError(
            f"`sqb test --local` needs the SQLQuack converter, which was not found in {where}",
            code=LOCAL_TEST_ENGINE_CODE,
            help=LOCAL_TEST_INSTALL_HELP,
        )
    return Path(found).resolve()


def checked_sqlquack_version(executable: Path) -> str:
    """The executable's SQLQuack version, refused below the minimum."""

    try:
        completed: subprocess.CompletedProcess[str] = subprocess.run(
            [str(executable), "--version"],
            capture_output=True,
            text=True,
            timeout=SQLQUACK_VERSION_TIMEOUT_SECONDS,
            check=False,
        )
    except OSError as error:
        raise ExecutorInputError(
            f"SQLQuack at {executable} could not be started: {error}",
            code=LOCAL_TEST_ENGINE_CODE,
            help=LOCAL_TEST_INSTALL_HELP,
        ) from error
    matched: re.Match[str] | None = SQLQUACK_VERSION_PATTERN.match(completed.stdout.strip())
    minimum: str = ".".join(str(part) for part in SQLQUACK_MINIMUM_VERSION)
    if matched is None:
        raise ExecutorInputError(
            f"{executable} did not report a SQLQuack version: "
            f"{(completed.stdout or completed.stderr).strip()[:200]}",
            code=LOCAL_TEST_ENGINE_CODE,
            help=LOCAL_TEST_INSTALL_HELP,
        )
    version: tuple[int, int, int] = (
        int(matched.group(1)),
        int(matched.group(2)),
        int(matched.group(3)),
    )
    if version < SQLQUACK_MINIMUM_VERSION:
        raise ExecutorInputError(
            f"SQLQuack {'.'.join(str(part) for part in version)} at {executable} is older than "
            f"the {minimum} `sqb test --local` needs",
            code=LOCAL_TEST_ENGINE_CODE,
            help=LOCAL_TEST_INSTALL_HELP,
        )
    return ".".join(str(part) for part in version)
