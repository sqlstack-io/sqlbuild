from collections.abc import Iterable
from itertools import chain

from sqlbuild._native import DiscoveredProjectInputs, SourceEntry


def base_repo_files() -> dict[str, str]:
    return {
        "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
    }


def named_source_entry(discovered: DiscoveredProjectInputs, *, name: str) -> SourceEntry:
    source_entries: Iterable[SourceEntry] = chain.from_iterable(
        source_file.source_entries for source_file in discovered.source_files
    )
    return {entry.name: entry for entry in source_entries}[name]
