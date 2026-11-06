"""Bounded, private evidence for a failed delta/full comparison, never CLI output."""

from __future__ import annotations

import hashlib
import json
import os
import time
from itertools import islice
from pathlib import Path
from typing import Any, cast

from scripts.delta_compile_series._helpers.encryption import encrypt_comparison_failure
from scripts.delta_compile_series._helpers.outcomes import _seen
from scripts.delta_compile_series.constants import (
    COMPARISON_MANIFEST_FILE,
    MAX_CACHE_PATHS,
    MAX_ENTRIES,
    MAX_FAILURES,
    MAX_FILE_BYTES,
    MAX_READ_BYTES,
    MAX_STEP_BYTES,
)
from scripts.delta_compile_series.models import DeltaCompileOutcome
from sqlbuild._native import redact_diagnostic_text, redact_diagnostic_value

_MANIFEST_RESERVE: int = 64 * 1024


def _file_state(*, path: Path, root: Path, limit: int = MAX_FILE_BYTES) -> dict[str, Any]:
    """Stat and bounded content digest, without following paths outside the project."""

    if not path.resolve().is_relative_to(root.resolve()):
        return {"outside_project": True}
    if not path.is_file():
        return {"missing": True}
    status: os.stat_result = path.stat()
    with path.open("rb") as stream:
        data: bytes = stream.read(limit)
    return {
        "size": status.st_size,
        "mtime_ns": status.st_mtime_ns,
        "ctime_ns": status.st_ctime_ns,
        "sha256": hashlib.sha256(data).hexdigest(),
        "hashed_bytes": len(data),
        "digest_truncated": len(data) != status.st_size,
    }


def cache_state(project: Path) -> dict[str, Any]:
    """Report evidence-collection errors without changing the comparison or CLI outcome."""

    try:
        return _cache_state(project)
    except (OSError, ValueError) as error:
        return {"capture_error": str(error)}


def _cache_state(project: Path) -> dict[str, Any]:
    """Snapshot bounded SQL-test records and physical reuse evidence, not inferred identities."""

    snapshots: dict[str, Any] = {}
    remaining: int = MAX_READ_BYTES
    paths: list[Path] = list(
        islice((project / "target/cache").rglob("sql-test-artifacts.json"), MAX_CACHE_PATHS + 1)
    )
    for path in sorted(paths[:MAX_CACHE_PATHS]):
        snapshot: dict[str, Any] = _file_state(path=path, root=project)
        snapshots[str(path.relative_to(project))] = snapshot
        if snapshot.get("outside_project") or snapshot.get("missing"):
            continue
        with path.open("rb") as stream:
            data: bytes = stream.read(MAX_FILE_BYTES + 1)
        snapshot["cache_text"] = data[:MAX_FILE_BYTES].decode("utf-8", errors="replace")
        try:
            payload: Any = json.loads(data) if len(data) <= MAX_FILE_BYTES else None
        except (ValueError, UnicodeError):
            payload = None
        snapshot["parseable"] = isinstance(payload, dict)
        records: Any = payload.get("records") if isinstance(payload, dict) else None
        if not isinstance(records, dict):
            continue
        snapshot["records_truncated"] = len(records) > MAX_ENTRIES
        physical: dict[str, Any] = {}
        for key, raw in islice(cast(dict[str, Any], records).items(), MAX_ENTRIES):
            if not isinstance(raw, dict):
                continue
            record: dict[str, Any] = cast(dict[str, Any], raw)
            relative_path: Any = record.get("relative_path")
            if not isinstance(relative_path, str):
                continue
            state: dict[str, Any] = _file_state(
                path=project / "target/compiled/tests" / relative_path,
                root=project,
                limit=min(MAX_FILE_BYTES, remaining),
            )
            remaining -= state.get("hashed_bytes", 0)
            state["recorded_identity"] = record.get("identity")
            state["recorded_size_matches"] = state.get("size") == record.get("size")
            state["recorded_mtime_matches"] = state.get("mtime_ns") == record.get("mtime_ns")
            state["recorded_digest_matches"] = not state.get(
                "digest_truncated", True
            ) and state.get("sha256") == record.get("content_sha256")
            physical[key] = state
        snapshot["physical_artifacts"] = physical
    return {
        "captured_at_ns": time.time_ns(),
        "caches": snapshots,
        "cache_paths_truncated": len(paths) > MAX_CACHE_PATHS,
    }


def _artifacts(project: Path) -> dict[str, Any]:
    """Bounded per-file digests of actual compiled SQL, without copying query text."""

    paths: list[Path] = list(islice((project / "target/compiled").rglob("*.sql"), MAX_ENTRIES + 1))
    files: dict[str, Any] = {}
    remaining: int = MAX_READ_BYTES
    for path in sorted(paths[:MAX_ENTRIES]):
        state: dict[str, Any] = _file_state(
            path=path, root=project, limit=min(MAX_FILE_BYTES, remaining)
        )
        remaining -= state.get("hashed_bytes", 0)
        files[str(path.relative_to(project))] = state
    return {
        "files": files,
        "truncated": len(paths) > MAX_ENTRIES,
    }


def _saved(*, destination: Path, name: str, text: str) -> dict[str, Any]:
    """Redact named credentials, preserve stream order, and explicitly mark any byte limit."""

    original: bytes = text.encode("utf-8", errors="surrogateescape")
    data: bytes = redact_diagnostic_text(text).encode("utf-8", errors="surrogateescape")
    used: int = sum(path.stat().st_size for path in destination.iterdir())
    limit: int = (
        min(_MANIFEST_RESERVE, max(0, MAX_STEP_BYTES - used))
        if name == COMPARISON_MANIFEST_FILE
        else min(MAX_FILE_BYTES, max(0, MAX_STEP_BYTES - _MANIFEST_RESERVE - used))
    )
    kept: bytes = data[:limit]
    with (destination / name).open("xb") as stream:
        os.chmod(stream.name, 0o600)
        _ = stream.write(kept)
    return {
        "original_sha256": hashlib.sha256(original).hexdigest(),
        "original_bytes": len(original),
        "redacted": data != original,
        "retained_bytes": len(kept),
        "truncated": len(kept) != len(data),
    }


def keep_comparison_failure(
    *,
    destination: Path,
    seed: int,
    step: str,
    case: str,
    project: Path,
    copy: Path,
    outcome: DeltaCompileOutcome,
    full: DeltaCompileOutcome,
    before: dict[str, Any],
    after: dict[str, Any],
) -> None:
    """Keep at most four 4-MiB private failures; nothing is printed or uploaded."""

    destination.mkdir(parents=True, exist_ok=True, mode=0o700)
    destination.chmod(0o700)
    failures: list[Path] = list(destination.iterdir())
    if len(failures) >= MAX_FAILURES:
        return
    directory: Path = destination / f"failure-{len(failures):02d}"
    directory.mkdir(mode=0o700)
    manifest: dict[str, Any] = {"seed": seed, "step": step, "case": case, "files": {}, "runs": {}}
    for label, result, root in (("delta", outcome, project), ("full", full, copy)):
        normalized: tuple[str, ...] = _seen(outcome=result, project_dir=project, other_dir=root)
        try:
            report: Any = json.loads(normalized[1])
        except ValueError:
            report = None
        manifest["runs"][label] = {
            "returncode": result.returncode,
            "command": result.command,
            "environment": [
                (name, redact_diagnostic_value(name=name, value=value))
                for name, value in result.environment
            ],
            "project": str(root),
            "semantic_json_sha256": hashlib.sha256(
                json.dumps(report, sort_keys=True).encode()
            ).hexdigest()
            if report is not None
            else None,
        }
        for stream in ("stdout", "stderr"):
            name = f"{label}.{stream}"
            manifest["files"][name] = _saved(
                destination=directory, name=name, text=getattr(result, stream)
            )
        name = f"{label}.artifacts.json"
        manifest["files"][name] = _saved(
            destination=directory, name=name, text=json.dumps(_artifacts(root), sort_keys=True)
        )
    for name, state in (("cache-before.json", before), ("cache-after.json", after)):
        manifest["files"][name] = _saved(
            destination=directory, name=name, text=json.dumps(state, sort_keys=True)
        )
    _ = _saved(
        destination=directory,
        name=COMPARISON_MANIFEST_FILE,
        text=json.dumps(manifest, indent=2, sort_keys=True),
    )
    encrypt_comparison_failure(directory)
