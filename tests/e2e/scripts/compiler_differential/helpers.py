"""Harness invocations and small corpus projects for the harness tests."""

from __future__ import annotations

from pathlib import Path

from scripts.compiler_differential.constants import GENERATOR_FEATURE_BLOCKS
from scripts.compiler_differential.main.failure_cases import failure_cases

WAFFLE_SHOP_FIXTURE: Path = Path(__file__).resolve().parents[2] / "fixtures" / "waffle_shop"
_BROKEN_REF_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "broken_orders"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "broken_orders.duckdb"\n'
    ),
    "sql/models/order_totals.sql": (
        'MODEL (\n  description "Order totals",\n);\n\nSELECT * FROM __ref("nope")\n'
    ),
}


def write_broken_ref_project(directory: Path) -> Path:
    """Write a project whose only model references a model that does not exist."""

    for relative_path, contents in _BROKEN_REF_FILES.items():
        path: Path = directory / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8", newline="\n")
    return directory


def write_failure_case_project(directory: Path, *, name: str) -> Path:
    """Write one named failure-corpus project below a directory."""

    {case.name: case for case in failure_cases()}[name].write(directory)
    return directory


def harness_arguments(
    *, work_dir: Path, extra: tuple[str, ...], project: Path = WAFFLE_SHOP_FIXTURE
) -> list[str]:
    """Compare only one project (the waffle shop fixture by default), keeping run directories."""

    return [
        "--corpus",
        "--project",
        str(project),
        "--jobs",
        "1",
        "--work-dir",
        str(work_dir),
        *extra,
    ]


def every_block_sets() -> tuple[tuple[str, ...], ...]:
    """Every feature block across two projects: one without the project adapter, one with it.

    A project adapter drops the loader and load blocks, which write only to built-in adapters.
    """

    return (
        tuple(block for block in GENERATOR_FEATURE_BLOCKS if block != "project_adapter"),
        GENERATOR_FEATURE_BLOCKS,
    )
