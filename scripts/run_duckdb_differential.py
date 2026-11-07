"""Direct wrapper for the DuckDB parse differential."""

from __future__ import annotations

from scripts.duckdb_differential.main.run import run_duckdb_differential


def main(argv: list[str] | None = None) -> int:
    """Compare Polyglot's DuckDB parse of SQLBuild's SQL with SQLQuack's DuckDB parser."""
    return run_duckdb_differential(argv)


if __name__ == "__main__":
    raise SystemExit(main())
