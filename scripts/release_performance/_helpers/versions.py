"""Resolve and install the compared sqlbuild versions, each into its own virtual environment."""

from __future__ import annotations

import hashlib
import io
import json
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.request
from pathlib import Path
from typing import cast

from scripts.release_performance._helpers.verdict import version_key
from scripts.release_performance.constants import (
    BASELINE_PUBLICATION_POLL_SECONDS,
    PACKAGE_NAME,
    PYPI_REQUEST_TIMEOUT_SECONDS,
    PYPI_SIMPLE_JSON,
    PYPI_SIMPLE_URL,
    RELEASE_TAG_PREFIX,
    STDERR_TAIL_CHARACTERS,
    UNIVERSAL_WHEEL_MACHINE,
    WHEEL_MACHINE_ALIASES,
    WHEEL_PLATFORM_FAMILIES,
)
from scripts.release_performance.exceptions import ReleasePerformanceError

_RELEASE_VERSION: re.Pattern[str] = re.compile(r"\d+\.\d+\.\d+")
_DISTRIBUTION_VERSION: re.Pattern[str] = re.compile(
    rf"^{PACKAGE_NAME}-(\d+\.\d+\.\d+)(?:-.+\.whl|\.tar\.gz)$"
)
_LIBRARY_PATHS_PROBE: str = (
    "import json, sys, sysconfig; print(json.dumps({'version': list(sys.version_info[:2]), "
    "'paths': [sysconfig.get_path('purelib'), sysconfig.get_path('platlib')]}))"
)


def release_versions(*, index: dict[str, object]) -> tuple[str, ...]:
    """Return the X.Y.Z versions in a PEP 691 index that have a file that is not yanked."""

    return _index_versions(index=index, include_yanked=False)


def choose_baseline(*, candidate: str, index: dict[str, object], same_version: bool) -> str | None:
    """Return the highest release on PyPI below the candidate, or its own version when allowed."""

    published: tuple[str, ...] = release_versions(index=index)
    if same_version and candidate in published:
        return candidate
    return previous_version(candidate=candidate, versions=published)


def previous_version(*, candidate: str, versions: tuple[str, ...]) -> str | None:
    """Return the highest version strictly below candidate."""

    earlier: list[str] = [
        version
        for version in versions
        if version_key(version=version) < version_key(version=candidate)
    ]
    return max(earlier, key=lambda version: version_key(version=version)) if earlier else None


def compatible_wheels(
    *, index: dict[str, object], version: str, machine: str, system: str
) -> tuple[dict[str, object], ...]:
    """Return the non-yanked wheels of version whose platform tag matches this runner."""

    files: list[dict[str, object]] = cast(list[dict[str, object]], index.get("files", []))
    prefix: str = f"{PACKAGE_NAME}-{version}-"
    return tuple(
        entry
        for entry in files
        if str(entry.get("filename", "")).startswith(prefix)
        and str(entry.get("filename", "")).endswith(".whl")
        and not entry.get("yanked")
        and _platform_matches(
            platform_tag=str(entry["filename"]).removesuffix(".whl").rsplit("-", 1)[-1],
            machine=machine.lower(),
            system=system,
        )
    )


def fetch_simple_index() -> dict[str, object]:
    """Read the PyPI simple index uncompressed, which reflects new uploads before the CDN does."""

    request: urllib.request.Request = urllib.request.Request(
        PYPI_SIMPLE_URL,
        headers={
            "Accept": PYPI_SIMPLE_JSON,
            "Accept-Encoding": "identity",
            "Cache-Control": "no-cache",
        },
    )
    with urllib.request.urlopen(request, timeout=PYPI_REQUEST_TIMEOUT_SECONDS) as response:
        return cast(dict[str, object], json.loads(response.read()))


def release_source(*, version: str, repo_dir: Path, destination: Path) -> Path:
    """Extract the source tree of release tag v<version> from repo_dir into destination."""

    tag: str = RELEASE_TAG_PREFIX + version
    completed: subprocess.CompletedProcess[bytes] = subprocess.run(
        ["git", "-C", str(repo_dir), "archive", "--format=tar", tag],
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise ReleasePerformanceError(
            f"Release tag {tag} is not available in {repo_dir}, so the baseline's own benchmark "
            "generator cannot be used; fetch the release tags or pass --baseline-source: "
            + _tail(completed.stderr.decode("utf-8", errors="replace"))
        )
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)
    with tarfile.open(fileobj=io.BytesIO(completed.stdout)) as archive:
        archive.extractall(destination, filter="data")
    return destination


def resolve_baseline(*, candidate: str, same_version: bool) -> str:
    """Return the baseline release that PyPI actually serves for the candidate."""

    baseline: str | None = choose_baseline(
        candidate=candidate, index=fetch_simple_index(), same_version=same_version
    )
    if baseline is None:
        raise ReleasePerformanceError(f"No published sqlbuild release precedes {candidate}")
    return baseline


def install_published(*, version: str, venv_dir: Path, python: str, wait_seconds: float) -> Path:
    """Install a published version, falling back to its wheel URL while the CDN lags."""

    _create_venv(venv_dir=venv_dir, python=python)
    deadline: float = time.monotonic() + wait_seconds
    while True:
        attempt: subprocess.CompletedProcess[str] = _uv_pip_install(
            venv_dir=venv_dir, requirements=(f"{PACKAGE_NAME}=={version}",)
        )
        if attempt.returncode == 0:
            return _sqb(venv_dir=venv_dir)
        print(
            f"Index install of {PACKAGE_NAME}=={version} failed; trying the wheel URL: "
            f"{_tail(attempt.stderr)}",
            file=sys.stderr,
        )
        if _install_from_wheel_urls(version=version, venv_dir=venv_dir):
            return _sqb(venv_dir=venv_dir)
        if time.monotonic() >= deadline:
            raise ReleasePerformanceError(
                f"{PACKAGE_NAME} {version} is not installable from PyPI; if the release is still "
                "publishing, rerun once its wheels are available"
            )
        print(
            f"{PACKAGE_NAME} {version} is not on PyPI yet; retrying in "
            f"{BASELINE_PUBLICATION_POLL_SECONDS:.0f}s",
            file=sys.stderr,
        )
        time.sleep(BASELINE_PUBLICATION_POLL_SECONDS)


def install_wheel(*, wheel: Path, venv_dir: Path, python: str) -> Path:
    """Install a locally built wheel into a fresh virtual environment."""

    _create_venv(venv_dir=venv_dir, python=python)
    completed: subprocess.CompletedProcess[str] = _uv_pip_install(
        venv_dir=venv_dir,
        requirements=("--find-links", str(wheel.resolve().parent), str(wheel.resolve())),
    )
    if completed.returncode != 0:
        raise ReleasePerformanceError(f"Installing {wheel} failed: {_tail(completed.stderr)}")
    return _sqb(venv_dir=venv_dir)


def installed_version(*, sqb: Path) -> str:
    """Return the version an installed sqb reports."""

    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [str(sqb), "--version"], capture_output=True, text=True, check=False
    )
    match: re.Match[str] | None = _RELEASE_VERSION.search(completed.stdout)
    if completed.returncode != 0 or match is None:
        raise ReleasePerformanceError(f"{sqb} --version failed: {_tail(completed.stderr)}")
    return match.group(0)


def library_paths(*, python: Path) -> tuple[Path, ...]:
    """Return the package directories of `python`'s environment, which must match this Python."""

    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [str(python), "-c", _LIBRARY_PATHS_PROBE], capture_output=True, text=True, check=False
    )
    if completed.returncode != 0:
        raise ReleasePerformanceError(
            f"Reading the package directories of {python} failed: {_tail(completed.stderr)}"
        )
    probe: dict[str, list[object]] = cast(dict[str, list[object]], json.loads(completed.stdout))
    version: tuple[object, ...] = tuple(probe["version"])
    if version != tuple(sys.version_info[:2]):
        raise ReleasePerformanceError(
            f"{python} runs Python {'.'.join(map(str, version))}, but its packages are imported "
            f"by Python {sys.version_info.major}.{sys.version_info.minor}; pass --python "
            f"{sys.version_info.major}.{sys.version_info.minor}"
        )
    return tuple(dict.fromkeys(Path(str(path)) for path in probe["paths"]))


def existing_sqb(*, venv_dir: Path) -> Path:
    """Return the sqb entry point of an existing virtual environment."""

    return _sqb(venv_dir=venv_dir)


def _index_versions(*, index: dict[str, object], include_yanked: bool) -> tuple[str, ...]:
    versions: set[str] = set()
    files: list[dict[str, object]] = cast(list[dict[str, object]], index.get("files", []))
    for entry in files:
        match: re.Match[str] | None = _DISTRIBUTION_VERSION.match(str(entry.get("filename", "")))
        if match is not None and (include_yanked or not entry.get("yanked")):
            versions.add(match.group(1))
    return tuple(sorted(versions, key=lambda version: version_key(version=version)))


def _install_from_wheel_urls(*, version: str, venv_dir: Path) -> bool:
    try:
        index: dict[str, object] = fetch_simple_index()
    except OSError as error:
        print(f"Reading {PYPI_SIMPLE_URL} failed: {error}", file=sys.stderr)
        return False
    wheels: tuple[dict[str, object], ...] = compatible_wheels(
        index=index, version=version, machine=platform.machine(), system=sys.platform
    )
    if not wheels:
        return False
    download_dir: Path = venv_dir.parent / f"{venv_dir.name}-wheels"
    download_dir.mkdir(parents=True, exist_ok=True)
    for entry in wheels:
        _download_wheel(entry=entry, download_dir=download_dir)
    completed: subprocess.CompletedProcess[str] = _uv_pip_install(
        venv_dir=venv_dir,
        requirements=("--find-links", str(download_dir), f"{PACKAGE_NAME}=={version}"),
    )
    if completed.returncode != 0:
        print(f"Wheel URL install failed: {_tail(completed.stderr)}", file=sys.stderr)
    return completed.returncode == 0


def _download_wheel(*, entry: dict[str, object], download_dir: Path) -> None:
    request: urllib.request.Request = urllib.request.Request(
        str(entry["url"]), headers={"Accept-Encoding": "identity"}
    )
    with urllib.request.urlopen(request, timeout=PYPI_REQUEST_TIMEOUT_SECONDS) as response:
        content: bytes = response.read()
    hashes: dict[str, str] = cast(dict[str, str], entry.get("hashes", {}))
    expected: str | None = hashes.get("sha256")
    if expected is not None and hashlib.sha256(content).hexdigest() != expected:
        raise ReleasePerformanceError(f"Downloaded {entry['filename']} does not match its sha256")
    _ = (download_dir / str(entry["filename"])).write_bytes(content)


def _platform_matches(*, platform_tag: str, machine: str, system: str) -> bool:
    family: str | None = WHEEL_PLATFORM_FAMILIES.get(system)
    machines: tuple[str, ...] = (
        machine,
        WHEEL_MACHINE_ALIASES.get(machine, machine),
        UNIVERSAL_WHEEL_MACHINE,
    )
    if family is None or family not in platform_tag:
        return False
    return any(name in platform_tag for name in machines)


def _create_venv(*, venv_dir: Path, python: str) -> None:
    shutil.rmtree(venv_dir, ignore_errors=True)
    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [_uv(), "venv", "--quiet", "--python", python, str(venv_dir)],
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise ReleasePerformanceError(
            f"Creating a Python {python} environment failed: {_tail(completed.stderr)}"
        )


def _uv_pip_install(
    *, venv_dir: Path, requirements: tuple[str, ...]
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            _uv(),
            "pip",
            "install",
            "--quiet",
            "--python",
            str(venv_dir / "bin" / "python"),
            *requirements,
        ],
        capture_output=True,
        text=True,
        check=False,
    )


def _sqb(*, venv_dir: Path) -> Path:
    sqb: Path = venv_dir / "bin" / "sqb"
    if not sqb.exists():
        raise ReleasePerformanceError(f"{venv_dir} has no sqb entry point")
    return sqb


def _uv() -> str:
    uv: str | None = shutil.which("uv")
    if uv is None:
        raise ReleasePerformanceError("uv is required to create the compared environments")
    return uv


def _tail(text: str) -> str:
    return text.strip()[-STDERR_TAIL_CHARACTERS:]
