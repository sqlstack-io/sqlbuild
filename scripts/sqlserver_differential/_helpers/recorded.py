"""Run a test suite with the statement recorder and read what it recorded."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from scripts.sqlserver_differential.constants import (
    SQLSERVER_ADAPTERS,
    NO_GATE,
    RECORD_DIR_VARIABLE,
    REPO_ROOT,
)

SITECUSTOMIZE: str = (
    "from scripts.sqlserver_differential._helpers.recorder import install\n\ninstall()\n"
)


def record_suite(*, record_dir: Path, pytest_args: list[str]) -> int:
    """Run pytest on `pytest_args` with every process recording into `record_dir`."""
    record_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as site:
        (Path(site) / "sitecustomize.py").write_text(SITECUSTOMIZE, encoding="utf-8")
        env: dict[str, str] = dict(os.environ)
        paths: list[str] = [site, str(REPO_ROOT), env.get("PYTHONPATH", "")]
        env["PYTHONPATH"] = os.pathsep.join(path for path in paths if path)
        env[RECORD_DIR_VARIABLE] = str(record_dir)
        command: list[str] = [sys.executable, "-m", "pytest", *pytest_args]
        return subprocess.run(command, cwd=REPO_ROOT, env=env, check=False).returncode


def recorded_cases(*, record_dir: Path, source: str) -> Iterator[dict[str, Any]]:
    """One case per distinct SQL Server statement recorded under `record_dir`."""
    seen: set[str] = set()
    for path in sorted(record_dir.glob("*.jsonl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            record: dict[str, Any] = json.loads(line)
            sql: Any = record.get("sql")
            if record.get("adapter") not in SQLSERVER_ADAPTERS or not isinstance(sql, str):
                continue
            if sql in seen or not sql.strip():
                continue
            seen.add(sql)
            yield {
                "id": f"{source}/{len(seen)}",
                "source": source,
                "kind": f"runtime_{record.get('intent')}",
                "sql": sql,
                "mode": NO_GATE,
            }
