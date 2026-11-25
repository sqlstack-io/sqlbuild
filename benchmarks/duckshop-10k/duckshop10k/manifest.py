"""Write a generated project and fingerprint it with a manifest of file hashes."""

from __future__ import annotations

import hashlib
from pathlib import Path

from duckshop10k import GENERATOR_VERSION

# Under the project's local build output, so the project root holds only SQLBuild project files.
MANIFEST_PATH = "target/MANIFEST.sha256"


def manifest(files: dict[str, str]) -> str:
    """One `<sha256>  <path>` line per generated file, sorted by path."""
    lines = [
        f"{hashlib.sha256(content.encode('utf-8')).hexdigest()}  {path}"
        for path, content in sorted(files.items())
    ]
    return "\n".join(lines) + "\n"


def checksum(files: dict[str, str]) -> str:
    """The checksum recorded in checksum.txt: the SHA-256 of the manifest."""
    return hashlib.sha256(manifest(files).encode("utf-8")).hexdigest()


def checksum_line(files: dict[str, str]) -> str:
    return f"generator {GENERATOR_VERSION} files {len(files)} sha256 {checksum(files)}\n"


def write_project(files: dict[str, str], out: Path) -> None:
    """Write every file below `out`, which must not exist or must be empty."""
    if out.exists() and any(out.iterdir()):
        raise SystemExit(f"output directory {out} is not empty")
    for path, content in files.items():
        target = out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content.encode("utf-8"))
    manifest_file = out / MANIFEST_PATH
    manifest_file.parent.mkdir(parents=True, exist_ok=True)
    manifest_file.write_bytes(manifest(files).encode("utf-8"))


def read_project(out: Path) -> dict[str, str]:
    """Every generated file below `out`, skipping local build output, which holds the manifest."""
    local = ("target/", "logs/", ".sqlbuild/", "sqlbuild_local.toml")
    files: dict[str, str] = {}
    for path in sorted(out.rglob("*")):
        relative = path.relative_to(out).as_posix()
        if path.is_file() and not relative.startswith(local):
            files[relative] = path.read_bytes().decode("utf-8")
    return files
