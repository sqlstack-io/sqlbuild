"""Direct wrapper for the Postgres differential harness."""

from __future__ import annotations

from scripts.sqlquack_differential.main.run import run_sqlquack_differential


def main(argv: list[str] | None = None) -> int:
    """Compare SQLBuild's Polyglot analysis of Postgres models with SQLQuack's."""
    return run_sqlquack_differential(argv)


if __name__ == "__main__":
    raise SystemExit(main())
