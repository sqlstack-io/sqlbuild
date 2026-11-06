"""Build and run the Rust harness against a SQLQuack checkout."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

from scripts.pg_differential.constants import HARNESS_BINARY, HARNESS_MANIFEST, SQLQUACK_CRATES
from scripts.pg_differential.exceptions import PgDifferentialError


def patch_flags(sqlquack: Path) -> list[str]:
    """`--config` flags that take the unpublished SQLQuack crates from `sqlquack`."""
    flags: list[str] = []
    for crate in SQLQUACK_CRATES:
        path: Path = (sqlquack / "crates" / crate).resolve()
        if not (path / "Cargo.toml").is_file():
            raise PgDifferentialError(f"{path} is not a SQLQuack crate")
        flags.extend(["--config", f'patch.crates-io.{crate}.path="{path}"'])
    return flags


def run_harness(*, sqlquack: Path, cases: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Build the harness and return its comparison of each case."""
    command: list[str] = [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(HARNESS_MANIFEST),
        "--bin",
        HARNESS_BINARY,
        *patch_flags(sqlquack),
    ]
    text: str = "".join(json.dumps(case) + "\n" for case in cases)
    completed: subprocess.CompletedProcess[str] = subprocess.run(
        command, input=text, capture_output=True, text=True, check=False
    )
    if completed.returncode != 0:
        raise PgDifferentialError(f"harness failed: {completed.stderr[-4000:]}")
    return [json.loads(line) for line in completed.stdout.splitlines() if line.strip()]
