"""Resolve the SQLQuack converter `sqb test --local` uses."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

from sqlbuild.executor.testing._helpers.local_engine import (
    checked_sqlquack_version,
    find_sqlquack,
    local_session,
)
from sqlbuild.executor.testing.constants import SQLQUACK_POLICY_FILE
from sqlbuild.executor.testing.models import LocalTestEngine


def resolve_local_test_engine(
    *, project_dir: Path, strict: bool, connection_config: Mapping[str, object]
) -> LocalTestEngine:
    """The SQLQuack executable to convert with, checked to be a supported version."""

    executable: Path = find_sqlquack()
    version: str = checked_sqlquack_version(executable)
    policy: Path = project_dir / SQLQUACK_POLICY_FILE
    session: tuple[tuple[str, str], ...]
    refusal: str | None
    session, refusal = local_session(connection_config)
    return LocalTestEngine(
        executable=executable,
        version=version,
        policy=policy if policy.is_file() else None,
        strict=strict,
        session_parameters=session,
        session_refusal=refusal,
    )
