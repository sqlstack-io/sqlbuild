"""Warm compile target fixtures and artifact inspection for target reconciliation tests."""

from __future__ import annotations

import os
from itertools import compress
from pathlib import Path

from sqlbuild._native import run_cli as main

_WARM_TARGET_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "shop"\nadapter = "duckdb"\n',
    "sql/models/sales/orders.sql": (
        "MODEL (description 'Test model orders.', columns (order_id (type INTEGER, "
        "audits [not_null])));\nSELECT 1 AS order_id\n"
    ),
    "sql/models/sales/daily/totals.sql": (
        "MODEL (description 'Test model totals.');\nSELECT order_id FROM __ref(\"orders\")\n"
    ),
    "sql/models/inventory.sql": (
        "MODEL (description 'Test model inventory.');\nSELECT 2 AS item_id\n"
    ),
}


def write_warm_target_project(*, project_dir: Path) -> None:
    """Write a small project with nested models and one attached audit."""

    for relative_path, contents in _WARM_TARGET_PROJECT_FILES.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


def compile_project(*, project_dir: Path, compile_args: tuple[str, ...]) -> int:
    """Run the real compile command and return its exit code."""

    return main(["--project-dir", str(project_dir), "--no-color", "compile", *compile_args])


def compiled_artifacts(*, compiled_dir: Path) -> tuple[Path, ...]:
    """Return every regular file below target/compiled, without following directory links."""

    artifacts: list[Path] = []
    for root, _directories, names in os.walk(compiled_dir):
        artifacts.extend(Path(root, name) for name in names)
    return tuple(sorted(artifacts))


def stamp_artifacts(*, artifacts: tuple[Path, ...], mtime_ns: int) -> None:
    """Give every artifact one known modification time."""

    for path in artifacts:
        os.utime(path, ns=(mtime_ns, mtime_ns))


def changed_artifacts(*, compiled_dir: Path, mtime_ns: int) -> tuple[str, ...]:
    """Return artifacts whose modification time moved away from the stamped value."""

    artifacts: tuple[Path, ...] = compiled_artifacts(compiled_dir=compiled_dir)
    changed: list[bool] = [path.stat().st_mtime_ns != mtime_ns for path in artifacts]
    return tuple(path.relative_to(compiled_dir).as_posix() for path in compress(artifacts, changed))


def pruned_entries(*, entries: dict[str, Path], names: tuple[str, ...]) -> tuple[str, ...]:
    """Return the named stale entries that no longer exist."""

    return tuple(compress(names, [not entries[name].exists() for name in names]))


def write_stale_target_entries(*, compiled_dir: Path, outside_dir: Path) -> dict[str, Path]:
    """Add a stale nested artifact, an empty folder, and a link to an outside folder."""

    stale_file: Path = compiled_dir / "sql" / "models" / "legacy" / "archive" / "returns.sql"
    stale_file.parent.mkdir(parents=True)
    stale_file.write_text("SELECT 'stale'\n", encoding="utf-8")
    empty_directory: Path = compiled_dir / "sql" / "models" / "empty" / "nested"
    empty_directory.mkdir(parents=True)
    outside_dir.mkdir()
    (outside_dir / "kept.sql").write_text("SELECT 'kept'\n", encoding="utf-8")
    link: Path = compiled_dir / "sql" / "models" / "linked" / "shared"
    link.parent.mkdir(parents=True)
    link.symlink_to(outside_dir, target_is_directory=True)
    return {
        "legacy": compiled_dir / "sql" / "models" / "legacy",
        "empty": compiled_dir / "sql" / "models" / "empty",
        "link": link,
        "outside_file": outside_dir / "kept.sql",
    }
