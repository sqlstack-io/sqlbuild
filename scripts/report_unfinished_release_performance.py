"""Direct wrapper for deciding whether a main comparison run needs an unfinished report."""

from __future__ import annotations

from scripts.release_performance_outcome.main.report import report_unfinished_comparison


def main(argv: list[str] | None = None) -> int:
    """Print why the comparison did not finish, or nothing."""

    return report_unfinished_comparison(argv)


if __name__ == "__main__":
    raise SystemExit(main())
