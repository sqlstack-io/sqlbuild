"""Direct wrapper for the shared parse guard check."""

from __future__ import annotations

from scripts.parse_guard.main.check import run_parse_guard_check


def main(argv: list[str] | None = None) -> int:
    """Check that native SQL parses use the shared parse guard."""
    return run_parse_guard_check(argv)


if __name__ == "__main__":
    raise SystemExit(main())
