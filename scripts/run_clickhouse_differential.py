"""Direct wrapper for the ClickHouse parse differential."""

from __future__ import annotations

from scripts.clickhouse_differential.main.run import run_clickhouse_differential


def main(argv: list[str] | None = None) -> int:
    """Compare Polyglot's ClickHouse parse of SQLBuild's SQL with SQLQuack's ClickHouse parser."""
    return run_clickhouse_differential(argv)


if __name__ == "__main__":
    raise SystemExit(main())
