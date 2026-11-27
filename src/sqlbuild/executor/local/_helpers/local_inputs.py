"""A project's SQLQuack allowances and overrides for local tests, and what conversions relied on."""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from sqlquack import ConvertOptions

from sqlbuild.executor.local.constants import (
    ALLOWED_LABEL,
    FUNCTION_CONSTRUCT_PREFIX,
    LOCAL_ALLOW_LOCATION,
    LOCAL_OVERRIDES_DIR,
    OVERRIDE_FILE_PATTERN,
    OVERRIDE_LABEL,
    USER_OVERRIDE_DIFFERENCE,
)
from sqlbuild.executor.local.models import LocalInputs


def local_inputs(*, project_dir: Path, allow: Mapping[str, bool]) -> LocalInputs:
    """The project's allowances and `tests/overrides` folder, with one digest of both."""

    folder: Path = project_dir / LOCAL_OVERRIDES_DIR
    files: list[Path] = sorted(folder.rglob(OVERRIDE_FILE_PATTERN)) if folder.is_dir() else []
    allowed: tuple[tuple[str, bool], ...] = tuple(sorted(allow.items()))
    digest: str = hashlib.sha256(
        json.dumps(
            [
                allowed,
                [
                    [
                        file.relative_to(folder).as_posix(),
                        hashlib.sha256(file.read_bytes()).hexdigest(),
                    ]
                    for file in files
                ],
            ]
        ).encode()
    ).hexdigest()
    return LocalInputs(allow=allowed, overrides=str(folder) if files else None, digest=digest)


def convert_options(
    *, inputs: LocalInputs, schema: Mapping[str, Any] | None, session: Mapping[str, str]
) -> ConvertOptions:
    """SQLQuack's options for local test SQL, with the project's allowances and overrides."""

    return ConvertOptions(
        no_config=True,
        schema=schema,
        session=session,
        compact=True,
        bind_check=False,
        allow=dict(inputs.allow),
        allow_location=LOCAL_ALLOW_LOCATION,
        overrides=inputs.overrides,
    )


def relied_on(report: Mapping[str, Any]) -> tuple[str, ...]:
    """What a conversion relied on from the project, such as `HASH (allowed)` or `(override)`."""

    labels: list[str] = []
    for use in report.get("known_differences", ()):
        construct: str = str(use["construct"]).removeprefix(FUNCTION_CONSTRUCT_PREFIX)
        if use["allowed_by"] == LOCAL_ALLOW_LOCATION:
            labels.append(f"{construct} ({ALLOWED_LABEL})")
        elif use["name"] == USER_OVERRIDE_DIFFERENCE:
            labels.append(f"{construct} ({OVERRIDE_LABEL})")
    return tuple(dict.fromkeys(labels))
