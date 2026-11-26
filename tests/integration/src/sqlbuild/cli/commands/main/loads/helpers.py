from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main

DATABASE_FILE: str = "shop.duckdb"


@dataclass(frozen=True)
class CliRun:
    """One in-process sqb run: its exit code, combined output and standard output."""

    exit_code: int
    output: str
    stdout: str


def write_project(*, root: Path, sources_yml: str, models: dict[str, str]) -> Path:
    """A DuckDB project whose `landing` connection reads `root / "landing"`, beside the project."""

    project_dir: Path = root / "project"
    landing: Path = root / "landing" / "orders"
    landing.mkdir(parents=True, exist_ok=True)
    (project_dir / "sql" / "sources").mkdir(parents=True)
    (project_dir / "sql" / "models").mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        dedent(
            f"""
            name = "shop_project"
            adapter = "duckdb"
            default_target = "dev"

            [connections.local]
            database = "{DATABASE_FILE}"

            [connections.landing]
            kind = "filesystem"
            root = "{(root / "landing").as_posix()}"

            [targets.dev]
            connection = "local"
            schema = "dev"
            loader_schema = "raw"
            """
        ).lstrip()
    )
    (project_dir / "sql" / "sources" / "shop.yml").write_text(sources_yml)
    for name, sql in models.items():
        (project_dir / "sql" / "models" / f"{name}.sql").write_text(sql)
    return project_dir


def land_file(*, root: Path, name: str, text: str) -> None:
    """Put one CSV file where the load reads orders from."""

    (root / "landing" / "orders" / name).write_text(text)


def run_sqb(
    *, project_dir: Path, args: tuple[str, ...], capsys: pytest.CaptureFixture[str]
) -> CliRun:
    """Run sqb in-process and capture combined output."""

    _ = capsys.readouterr()
    exit_code: int = main(["--project-dir", str(project_dir), "--no-color", *args])
    captured: CaptureResult[str] = capsys.readouterr()
    return CliRun(exit_code=exit_code, output=captured.out + captured.err, stdout=captured.out)


def query(*, project_dir: Path, sql: str) -> list[tuple[Any, ...]]:
    """Rows of `sql` in the project's DuckDB database."""

    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / DATABASE_FILE))
    try:
        return connection.sql(sql).fetchall()
    finally:
        connection.close()
