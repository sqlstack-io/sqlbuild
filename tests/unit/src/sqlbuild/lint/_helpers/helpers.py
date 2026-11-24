"""Shared builders for lint helper unit tests."""

from __future__ import annotations

from dataclasses import astuple
from pathlib import Path

from sqlbuild._native import (
    InterpolationSite,
    LintBody,
    LintConfig,
    LintViolation,
    lint_native_sql_bodies,
    neutralize_lint_interpolation,
    restore_lint_interpolation,
)


def unused_cte_orders_file(
    *, tmp_path: Path, prefix: str, file_index: int, unused_cte_count: int
) -> tuple[Path, str, LintBody]:
    """Build one multi-line file, offset by its index, whose body declares many unused CTEs."""

    target: Path = tmp_path / f"orders_{file_index}.sql"
    sql: str = (
        "WITH\n"
        + "".join(
            f"  unused_{index:03d} AS (\n    SELECT {index} AS order_id\n  ),\n"
            for index in range(unused_cte_count)
        )
        + "  final AS (SELECT 1 AS order_id)\nSELECT order_id\nFROM final\n"
    )
    header: str = prefix + "-- order history\n" * file_index
    contents: str = header + sql
    body: LintBody = LintBody(
        file_path=target,
        body_start=len(header),
        body_end=len(contents),
        lint_text=sql,
        passes=(),
    )
    return target, contents, body


def authored_positions(*, contents: str, needles: tuple[str, ...]) -> list[tuple[int, int]]:
    """One-based line and column of each needle's first occurrence."""

    positions: list[tuple[int, int]] = []
    for needle in needles:
        offset: int = contents.index(needle)
        positions.append(
            (contents.count("\n", 0, offset) + 1, offset - contents.rfind("\n", 0, offset))
        )
    return positions


def write_lint_project(*, root: Path, files: dict[str, str]) -> None:
    """Write a DuckDB project containing the given relative files."""

    (root / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    for relative_path, contents in files.items():
        path: Path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


def write_orders_lint_project(*, project_dir: Path, model_bytes: bytes) -> Path:
    """Write a DuckDB project with one model and one SQL test; return the model path."""

    _ = (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    model_path: Path = project_dir / "sql" / "models" / "orders.sql"
    model_path.parent.mkdir(parents=True)
    _ = model_path.write_bytes(model_bytes)
    test_path: Path = project_dir / "tests" / "test_orders.sql"
    test_path.parent.mkdir(parents=True)
    _ = test_path.write_text(
        "TEST (name orders_keep_ids);\n"
        "WITH orders AS (SELECT 1 AS order_id),\n"
        "expected AS (SELECT 1 AS order_id)\n"
        "SELECT * FROM orders\n",
        encoding="utf-8",
    )
    return model_path


def write_linked_orders_project(*, project_dir: Path) -> None:
    """Write models reached both directly and through file and directory links."""

    (project_dir / "sql" / "models" / "orders").mkdir(parents=True)
    (project_dir / "shared").mkdir(parents=True)
    (project_dir / "sql" / "models" / "orders" / "orders.sql").write_text("SELECT 1 AS order_id\n")
    (project_dir / "shared" / "customers.sql").write_text("SELECT 1 AS customer_id\n")
    (project_dir / "sql" / "models" / "orders" / "customers_link.sql").symlink_to(
        project_dir / "shared" / "customers.sql"
    )
    (project_dir / "sql" / "models" / "shared_link").symlink_to(project_dir / "shared")


def files_resolving_to(*, files: dict[Path, str], selected: frozenset[Path]) -> dict[Path, str]:
    """Return the files whose fully resolved path is one of the selected paths."""

    return dict(filter(lambda item: item[0].resolve() in selected, files.items()))


def neutralize_interpolation(
    *, body: str, dialect: str, for_formatting: bool = False
) -> tuple[str, tuple[InterpolationSite, ...]]:
    """Neutralize interpolation natively and return the sites as lint models."""

    neutralized, sites = neutralize_lint_interpolation(body, dialect, for_formatting=for_formatting)
    return neutralized, tuple(InterpolationSite(*site) for site in sites)


def restore_interpolation(*, fixed: str, sites: tuple[InterpolationSite, ...]) -> tuple[bool, str]:
    """Restore neutralized interpolation natively; returns whether it succeeded and the text."""

    return restore_lint_interpolation(fixed, [astuple(site) for site in sites])


def run_native_sql_lint(
    *, bodies: tuple[LintBody, ...], contents_by_path: dict[Path, str], config: LintConfig
) -> dict[Path, tuple[LintViolation, ...]]:
    """Lint bodies natively and group the violations by file."""

    grouped: dict[Path, list[LintViolation]] = {}
    for violation in lint_native_sql_bodies(list(bodies), contents_by_path, config):
        grouped.setdefault(violation.file_path, []).append(violation)
    return {path: tuple(violations) for path, violations in grouped.items()}
