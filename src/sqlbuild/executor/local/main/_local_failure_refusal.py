"""Why a failure of converted SQL, known only by its text, keeps a run off DuckDB."""

from __future__ import annotations

from sqlbuild.errors.contracts.exceptions import LocalRunRefusedError
from sqlbuild.executor.local._helpers.runtime import local_failure_refusal as _refusal


def local_failure_refusal(message: str) -> LocalRunRefusedError | None:
    """A SQLQuack guard's raise or DuckDB internal error in `message`, or None."""

    return _refusal(message)
