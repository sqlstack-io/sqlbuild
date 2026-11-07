"""Read the compiler differential's goldens into harness cases."""

from __future__ import annotations

import json
import re
from collections.abc import Iterator
from pathlib import Path
from typing import Any

from scripts.compiler_differential._helpers.corpus.failure_cases import all_failure_cases
from scripts.duckdb_differential.constants import (
    ADAPTER_SETTING,
    COMPILED_KINDS,
    CONFIG_FILE,
    DUCKDB_ADAPTER,
    GOLDEN_CORPORA,
    GOLDENS_DIR,
    NO_GATE,
    VALIDATE_MODE,
)


def other_adapter_failures() -> set[str]:
    """The failure cases whose project uses an adapter other than DuckDB."""
    names: set[str] = set()
    for case in all_failure_cases():
        for path, text in case.files.items():
            match = re.search(ADAPTER_SETTING, text) if path.endswith(CONFIG_FILE) else None
            if match is not None and match.group(1) != DUCKDB_ADAPTER:
                names.add(case.name)
    return names


def golden_cases(goldens: Path = GOLDENS_DIR) -> Iterator[dict[str, Any]]:
    """One case per compiled file of every DuckDB project's golden."""
    skipped: set[str] = other_adapter_failures()
    for corpus in GOLDEN_CORPORA:
        for path in sorted((goldens / corpus).glob("*.json")):
            project: str = path.stem
            # Seed variants such as `0-postgres` compile for other adapters.
            if project in skipped or corpus == "seed" and "-" in project:
                continue
            compiled: dict[str, list[str]] = json.loads(path.read_text(encoding="utf-8")).get(
                "compiled", {}
            )
            for name, lines in compiled.items():
                kind: str = COMPILED_KINDS.get(name.split("/", 1)[0], "compiled")
                yield {
                    "id": f"golden-{corpus}/{project}/{name}",
                    "source": f"golden-{corpus}",
                    "kind": kind,
                    "sql": "\n".join(lines),
                    "mode": VALIDATE_MODE if kind == "model" else NO_GATE,
                }
