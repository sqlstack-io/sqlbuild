"""Repository path predicates shared by SQLBuild custom Fensu rules."""

from scripts.fensu_policy.constants import (
    ADAPTER_CLASS_BOUNDARIES,
    ADAPTER_CLASS_BYPASS_MODULES,
    ADAPTER_CLASS_MODULE_MIN_PARTS,
    ADAPTER_IMPLEMENTATION_PATH_MARKERS,
    ADAPTER_ROOT_PARTS,
    MAIN_PACKAGE_NAME,
)


def is_adapter_class_entry(*, parts: tuple[str, ...]) -> bool:
    """Return whether a path is a legacy adapter class-entry module."""

    if len(parts) < ADAPTER_CLASS_MODULE_MIN_PARTS or parts[:3] != ADAPTER_ROOT_PARTS:
        return False
    if MAIN_PACKAGE_NAME in parts[3:-1]:
        return False
    if parts[-1].startswith("_") or parts[-1] in ADAPTER_CLASS_BYPASS_MODULES:
        return False
    return not any(part in ADAPTER_CLASS_BOUNDARIES for part in parts[3:-1])


def is_adapter_implementation_path(*, path: str) -> bool:
    """Return whether a path owns adapter implementation metadata calls."""

    adapters_marker, classes_marker = ADAPTER_IMPLEMENTATION_PATH_MARKERS
    return (adapters_marker in path and classes_marker in path) or path.endswith(
        (
            "/adapter/contract/classes/base_adapter.py",
            "/adapter/contract/classes/duckdb_backed_adapter.py",
        )
    )


def allowed_prefix(*, path: str, prefixes: list[str]) -> str | None:
    """The allowed file or directory entry that covers a repository-relative path."""

    prefix: str
    for prefix in prefixes:
        if path == prefix or path.startswith(prefix.rstrip("/") + "/"):
            return prefix
    return None
