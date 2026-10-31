"""Resolve the SQLQuack converter `sqb test --local` uses."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

from sqlbuild.executor.testing._helpers.local_engine import (
    checked_sqlquack_version,
    find_sqlquack,
    session_value,
)
from sqlbuild.executor.testing.constants import SQLQUACK_POLICY_FILE
from sqlbuild.executor.testing.models import LocalTestEngine


def resolve_local_test_engine(
    *, project_dir: Path, strict: bool, session_parameters: Mapping[str, object] | None
) -> LocalTestEngine:
    """The SQLQuack executable to convert with, checked to be a supported version."""

    executable: Path = find_sqlquack()
    version: str = checked_sqlquack_version(executable)
    policy: Path = project_dir / SQLQUACK_POLICY_FILE
    return LocalTestEngine(
        executable=executable,
        version=version,
        policy=policy if policy.is_file() else None,
        strict=strict,
        session_parameters=tuple(
            (str(name), session_value(value))
            for name, value in sorted((session_parameters or {}).items())
        ),
    )
