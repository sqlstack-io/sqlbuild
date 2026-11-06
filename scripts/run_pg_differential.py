"""Direct wrapper for the Postgres differential harness."""

from __future__ import annotations

from scripts.pg_differential.main.run import run_pg_differential


def main(argv: list[str] | None = None) -> int:
    """Compare SQLBuild's Polyglot analysis of Postgres models with SQLQuack's."""
    return run_pg_differential(argv)


if __name__ == "__main__":
    raise SystemExit(main())
