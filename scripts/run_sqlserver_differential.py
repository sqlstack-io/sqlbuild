"""Direct wrapper for the SQL Server parse differential."""

from __future__ import annotations

from scripts.sqlserver_differential.main.run import run_sqlserver_differential


def main(argv: list[str] | None = None) -> int:
    """Compare Polyglot's SQL Server parse of SQLBuild's SQL with SQLQuack's SQL Server parser."""
    return run_sqlserver_differential(argv)


if __name__ == "__main__":
    raise SystemExit(main())
