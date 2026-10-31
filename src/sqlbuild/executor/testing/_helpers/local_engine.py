"""Find the SQLQuack executable and check its version."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import cast

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.testing.constants import (
    CONNECTION_SESSION_KEYS,
    LOCAL_TEST_ENGINE_CODE,
    LOCAL_TEST_INSTALL_HELP,
    SESSION_NAME_SEPARATOR,
    SESSION_PARAMETERS_KEY,
    SQLQUACK_EXECUTABLE_ENV,
    SQLQUACK_EXECUTABLE_NAME,
    SQLQUACK_MINIMUM_VERSION,
    SQLQUACK_VERSION_PATTERN,
    SQLQUACK_VERSION_TIMEOUT_SECONDS,
)


def local_session(
    connection_config: Mapping[str, object],
) -> tuple[tuple[tuple[str, str], ...], str | None]:
    """The session parameters to tell SQLQuack, and why they cannot be told when they cannot."""

    parameters: dict[str, tuple[str, str]] = {}
    problems: list[str] = []
    declared: object = connection_config.get(SESSION_PARAMETERS_KEY)
    if declared is not None and not isinstance(declared, Mapping):
        problems.append(f"the connection's `{SESSION_PARAMETERS_KEY}` is not a table")
    named: list[tuple[str, object]] = [
        (str(name), value) for name, value in cast(Mapping[object, object], declared or {}).items()
    ]
    named.extend(
        (parameter, connection_config[key])
        for key, parameter in CONNECTION_SESSION_KEYS.items()
        if key in connection_config
    )
    for name, value in named:
        problem: str | None = _session_problem(name=name, value=value)
        if problem is not None:
            problems.append(problem)
            continue
        text: str = _session_text(value)
        earlier: tuple[str, str] | None = parameters.get(name.upper())
        if earlier is not None and earlier[1] != text:
            problems.append(
                f"session parameter {name.upper()} is set to both {earlier[1]!r} and {text!r}"
            )
            continue
        parameters[name.upper()] = (name, text)
    if problems:
        return (), "the connection's session cannot be passed to SQLQuack: " + "; ".join(problems)
    return tuple(sorted(parameters.values())), None


def _session_problem(*, name: str, value: object) -> str | None:
    if not name.strip() or SESSION_NAME_SEPARATOR in name:
        return f"session parameter name {name!r} is not a plain name"
    if not isinstance(value, str | int | float | bool):
        return f"session parameter {name} has a {type(value).__name__} value, not a scalar"
    return None


def _session_text(value: object) -> str:
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
    matched: re.Match[str] | None = (
        SQLQUACK_VERSION_PATTERN.match(completed.stdout.strip())
        if completed.returncode == 0
        else None
    )
    minimum: str = ".".join(str(part) for part in SQLQUACK_MINIMUM_VERSION)
    if matched is None:
        raise ExecutorInputError(
            f"{executable} did not report a SQLQuack release version "
            f"(exit {completed.returncode}): "
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
