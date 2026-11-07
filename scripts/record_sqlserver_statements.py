"""Run pytest with every statement SQLBuild sends to an adapter recorded per process.

Usage: `python -m scripts.record_sqlserver_statements DIR PYTEST_ARG...`. The SQL Server differential
reads `DIR` with `--recorded LABEL=DIR`.
"""

from __future__ import annotations

import sys
from pathlib import Path

from scripts.sqlserver_differential._helpers.recorded import record_suite


def main(argv: list[str] | None = None) -> int:
    """Record the statements of one pytest run."""
    arguments: list[str] = sys.argv[1:] if argv is None else argv
    if not arguments:
        raise SystemExit(__doc__)
    return record_suite(record_dir=Path(arguments[0]), pytest_args=arguments[1:])


if __name__ == "__main__":
    raise SystemExit(main())
