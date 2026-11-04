"""Check that every release version source names the same version."""

from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

from scripts.native_layering.constants import (
    CARGO_LOCKFILE,
    CARGO_MANIFEST,
    CLI_DISTRIBUTION_PIN_PREFIX,
    CRATES_DIRECTORY,
    LOCK_SOURCE_KEY,
    PACKAGE_VERSION_VARIABLE,
    PYPROJECT_LOCK_SOURCE,
    PYPROJECT_LOCKFILE,
    PYPROJECT_MANIFEST,
    RELEASE_CRATES,
    RELEASE_MANIFEST,
    RELEASE_MANIFEST_PACKAGE,
)


def get_native_version_errors(root: Path) -> tuple[str, ...]:
    """Return errors when the Cargo, Python, lockfile and release versions disagree."""
    with (root / CARGO_MANIFEST).open("rb") as handle:
        cargo: dict[str, Any] = tomllib.load(handle)
    with (root / PYPROJECT_MANIFEST).open("rb") as handle:
        pyproject: dict[str, Any] = tomllib.load(handle)
    release: dict[str, Any] = json.loads((root / RELEASE_MANIFEST).read_text(encoding="utf-8"))
    workspace_version: Any = cargo.get("workspace", {}).get("package", {}).get("version")
    versions: dict[str, Any] = {}
    for name in RELEASE_CRATES:
        manifest: str = f"{CRATES_DIRECTORY}/{name}/{CARGO_MANIFEST}"
        with (root / manifest).open("rb") as handle:
            versions[f"{manifest} version"] = tomllib.load(handle).get("package", {}).get("version")
    versions |= {
        f"{PYPROJECT_MANIFEST} [project] version": pyproject.get("project", {}).get("version"),
        f"{RELEASE_MANIFEST} version": release.get(RELEASE_MANIFEST_PACKAGE),
        f"{PYPROJECT_MANIFEST} sqlbuild-cli dependency": next(
            (
                dependency.removeprefix(CLI_DISTRIBUTION_PIN_PREFIX)
                for dependency in pyproject.get("project", {}).get("dependencies", ())
                if dependency.startswith(CLI_DISTRIBUTION_PIN_PREFIX)
            ),
            None,
        ),
    }
    errors: list[str] = []
    if len(set(versions.values())) != 1:
        listed: str = ", ".join(f"{source} is {version}" for source, version in versions.items())
        errors.append(f"Release versions differ: {listed}.")
    errors.extend(
        _cargo_lockfile_errors(root=root, cargo=cargo, workspace_version=workspace_version)
    )
    errors.extend(_python_lockfile_errors(root=root, pyproject=pyproject))
    errors.extend(_fixed_version_reader_errors(root=root, cargo=cargo))
    return tuple(errors)


def _fixed_version_reader_errors(*, root: Path, cargo: dict[str, Any]) -> tuple[str, ...]:
    """Report Rust sources outside the release crates that read their fixed crate version."""
    readers: list[str] = []
    for pattern in cargo.get("workspace", {}).get("members", ()):
        for directory in sorted(root.glob(pattern)):
            with (directory / CARGO_MANIFEST).open("rb") as handle:
                name: Any = tomllib.load(handle).get("package", {}).get("name")
            if name in RELEASE_CRATES:
                continue
            readers.extend(
                source.relative_to(root).as_posix()
                for source in sorted(directory.rglob("*.rs"))
                if PACKAGE_VERSION_VARIABLE in source.read_text(encoding="utf-8")
            )
    release_crates: str = " and ".join(RELEASE_CRATES)
    return tuple(
        f"{reader} reads {PACKAGE_VERSION_VARIABLE}, which is the fixed workspace version "
        f"outside {release_crates}; take the release version from those crates."
        for reader in readers
    )


def _python_lockfile_errors(*, root: Path, pyproject: dict[str, Any]) -> tuple[str, ...]:
    """Report a uv.lock entry for the project that carries a version other than pyproject's."""
    project: dict[str, Any] = pyproject.get("project", {})
    with (root / PYPROJECT_LOCKFILE).open("rb") as handle:
        lock: dict[str, Any] = tomllib.load(handle)
    locked: list[Any] = [
        entry.get("version")
        for entry in lock.get("package", ())
        if entry.get("name") == project.get("name")
        and entry.get(LOCK_SOURCE_KEY) == PYPROJECT_LOCK_SOURCE
    ]
    if locked == [project.get("version")]:
        return ()
    listed: str = ", ".join(map(str, locked)) or "no version"
    return (
        f"{PYPROJECT_LOCKFILE} locks {project.get('name')} at {listed}, "
        f"not {PYPROJECT_MANIFEST} {project.get('version')}. Run `uv lock`.",
    )


def _cargo_lockfile_errors(
    *, root: Path, cargo: dict[str, Any], workspace_version: Any
) -> tuple[str, ...]:
    """Report workspace crates whose Cargo.lock entry differs from their manifest version."""
    expected: dict[str, Any] = {}
    for pattern in cargo.get("workspace", {}).get("members", ()):
        for directory in sorted(root.glob(pattern)):
            with (directory / CARGO_MANIFEST).open("rb") as handle:
                package: dict[str, Any] = tomllib.load(handle).get("package", {})
            version: Any = package.get("version")
            if isinstance(version, dict) and version.get("workspace") is True:
                version = workspace_version
            expected[package.get("name")] = version
    with (root / CARGO_LOCKFILE).open("rb") as handle:
        lock: dict[str, Any] = tomllib.load(handle)
    stale: list[str] = sorted(
        f"{entry['name']} is {entry.get('version')}, not {expected[entry['name']]}"
        for entry in lock.get("package", ())
        if LOCK_SOURCE_KEY not in entry
        and entry.get("name") in expected
        and entry.get("version") != expected[entry["name"]]
    )
    if not stale:
        return ()
    return (
        f"{CARGO_LOCKFILE} workspace crate versions are stale: {'; '.join(stale)}. "
        "Run `cargo update --workspace`.",
    )
