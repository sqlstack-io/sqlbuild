"""One disposable database per native suite; Rust fixtures isolate their own schemas."""

from __future__ import annotations

import os
import subprocess
import sys

from testcontainers.postgres import PostgresContainer

from scripts.native_tests.constants import POSTGRES_IMAGE


def run_native_tests(argv: list[str] | None = None) -> int:
    """Fail closed on Docker/startup errors and propagate Cargo's result after cleanup."""
    arguments: list[str] = sys.argv[1:] if argv is None else argv
    print("Starting disposable PostgreSQL for native integration tests...", flush=True)
    try:
        with PostgresContainer(POSTGRES_IMAGE, driver=None) as postgres:
            print("PostgreSQL ready. Running native tests with isolated schemas...", flush=True)
            result: subprocess.CompletedProcess[bytes] = subprocess.run(
                ["cargo", "test", *arguments],
                env=os.environ
                | {
                    "SQLBUILD_TEST_CONTROL_POSTGRES_URL": postgres.get_connection_url(),
                },
                check=False,
            )
        print(f"PostgreSQL removed. Native tests exited {result.returncode}.", flush=True)
        return result.returncode
    except Exception as error:
        print(f"Native integration tests failed: {error}", file=sys.stderr, flush=True)
        return 1
