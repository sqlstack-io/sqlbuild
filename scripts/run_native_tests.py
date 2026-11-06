"""Direct wrapper for native tests with disposable PostgreSQL."""

from scripts.native_tests.main.run import run_native_tests


def main(argv: list[str] | None = None) -> int:
    """Run Cargo tests with a shared Testcontainers PostgreSQL instance."""
    return run_native_tests(argv)


if __name__ == "__main__":
    raise SystemExit(main())
