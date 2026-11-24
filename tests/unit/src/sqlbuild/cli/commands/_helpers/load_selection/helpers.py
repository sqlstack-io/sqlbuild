"""Helpers for load-selection unit tests."""

from __future__ import annotations

from pathlib import Path

from sqlbuild._native import (
    DiscoveredLoaderFunction,
    DiscoveredProjectInputs,
    DiscoveredSourceFile,
    LocalConfig,
    ProjectConfig,
    SourceEntry,
)


def fetch_orders(_ctx: object) -> list[dict[str, object]]:
    return []


def refresh_orders(_ctx: object) -> list[dict[str, object]]:
    return []


def load_raw_orders(_ctx: object) -> list[dict[str, object]]:
    return []


def build_load_selection_inputs() -> DiscoveredProjectInputs:
    """Build reachable load-selection inputs with terminal and intermediate loaders."""

    return DiscoveredProjectInputs(
        project_config=ProjectConfig(name="demo", adapter="duckdb"),
        local_config=LocalConfig(),
        source_files=(
            DiscoveredSourceFile(
                file_path=Path("sql/sources/raw.yml"),
                relative_path=Path("sql/sources/raw.yml"),
                contents="",
                source_entries=(SourceEntry(name="raw_orders", loader="load_raw_orders"),),
            ),
        ),
        loader_functions=(
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/loaders.py"),
                relative_path=Path("python/nodes/loaders/loaders.py"),
                name="fetch_orders",
                function=fetch_orders,
            ),
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/loaders.py"),
                relative_path=Path("python/nodes/loaders/loaders.py"),
                name="refresh_orders",
                function=refresh_orders,
            ),
            DiscoveredLoaderFunction(
                file_path=Path("python/nodes/loaders/loaders.py"),
                relative_path=Path("python/nodes/loaders/loaders.py"),
                name="load_raw_orders",
                function=load_raw_orders,
                depends_on=(fetch_orders,),
            ),
        ),
    )
