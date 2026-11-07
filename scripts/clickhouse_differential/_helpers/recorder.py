"""Record every statement SQLBuild sends to an adapter, at its one statement boundary.

`install` wraps `StatementLifecycle`, which every adapter statement passes through, so each
process appends `{adapter, intent, sql}` lines to its own file under `RECORD_DIR_VARIABLE`.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from scripts.clickhouse_differential.constants import RECORD_DIR_VARIABLE

# The method every adapter statement passes through, wrapped to record its arguments.
WRAPPED_METHOD: str = "__init__"


def install() -> None:
    """Record statements of this process when the record directory is set."""
    directory: str | None = os.environ.get(RECORD_DIR_VARIABLE)
    if not directory:
        return
    from sqlbuild.runtime.observability.classes.statement_lifecycle import StatementLifecycle

    path: Path = Path(directory) / f"{os.getpid()}.jsonl"
    original: Any = StatementLifecycle.__init__

    def recording_init(self: StatementLifecycle, *args: Any, **kwargs: Any) -> None:
        record: dict[str, Any] = {
            "adapter": kwargs.get("adapter"),
            "intent": kwargs.get("intent"),
            "sql": kwargs.get("sql"),
        }
        with path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(record) + "\n")
        original(self, *args, **kwargs)

    setattr(StatementLifecycle, WRAPPED_METHOD, recording_init)
