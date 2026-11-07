"""Read SQL bodies listed in JSON-lines files: the adapter's templates and other harvested SQL."""

from __future__ import annotations

import json
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from scripts.sqlserver_differential.constants import NO_GATE
from scripts.sqlserver_differential.exceptions import SqlServerDifferentialError


def listed_cases(*, path: Path, source: str) -> Iterator[dict[str, Any]]:
    """One case per distinct SQL text of a file of `{"id", "sql"[, "kind", "mode"]}` lines.

    Bodies SQLBuild emits without its syntax gate, such as the adapter's statements, have mode
    `none`; a line may name `validate` or `parse` for SQL the gate checks.
    """
    if not path.is_file():
        raise SqlServerDifferentialError(f"no SQL list at {path}")
    seen: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        item: dict[str, Any] = json.loads(line)
        sql: Any = item.get("sql")
        if not isinstance(sql, str) or not sql.strip() or sql in seen:
            continue
        seen.add(sql)
        yield {
            "id": f"{source}/{item.get('id') or len(seen)}",
            "source": source,
            "kind": str(item.get("kind") or item.get("method") or source),
            "sql": sql,
            "mode": str(item.get("mode") or NO_GATE),
        }
