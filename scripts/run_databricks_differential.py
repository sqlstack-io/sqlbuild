"""Direct wrapper for the Databricks parse differential."""

from __future__ import annotations

from scripts.databricks_differential.main.run import run_databricks_differential


def main(argv: list[str] | None = None) -> int:
    """Compare Polyglot's Databricks parse of SQLBuild's SQL with SQLQuack's Databricks parser."""
    return run_databricks_differential(argv)


if __name__ == "__main__":
    raise SystemExit(main())
