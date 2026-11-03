"""Real initialized stores and installed repair processes for projection acceptance."""

import sqlite3
import subprocess
from collections.abc import Iterator
from contextlib import closing, contextmanager
from pathlib import Path

import psycopg
from testcontainers.postgres import PostgresContainer

from scripts.native_tests.constants import POSTGRES_IMAGE
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ProjectionStore,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    environment,
    serve,
)


@contextmanager
def sqlite_store(installation: tuple[Path, Path]) -> Iterator[ProjectionStore]:
    """Initialize through the real server before opening a diagnostic connection."""
    with serve(installation):
        pass
    with closing(sqlite3.connect(installation[1] / ".sqlbuild/control.sqlite3")) as connection:
        yield ProjectionStore(
            {},
            connection,
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )


@contextmanager
def postgres_store(installation: tuple[Path, Path]) -> Iterator[ProjectionStore]:
    """Own the disposable database independently of every installed server/repair process."""
    with PostgresContainer(POSTGRES_IMAGE, driver=None) as postgres:
        extra: dict[str, str] = {"SQLBUILD_CONTROL_DATABASE_URL": postgres.get_connection_url()}
        with serve(installation, extra_environment=extra):
            pass
        with psycopg.connect(extra["SQLBUILD_CONTROL_DATABASE_URL"]) as connection:
            yield ProjectionStore(
                extra,
                connection,
                "SELECT table_name FROM information_schema.tables "
                "WHERE table_schema=current_schema() AND table_type='BASE TABLE'",
            )


def repair(
    installation: tuple[Path, Path], extra: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run the installed maintenance command with isolated database selection."""
    return subprocess.run(
        [str(installation[0]), "server", "rebuild-projections"],
        cwd=installation[1],
        env=environment(extra),
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )
