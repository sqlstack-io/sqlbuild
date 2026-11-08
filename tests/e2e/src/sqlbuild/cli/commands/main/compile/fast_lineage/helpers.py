from __future__ import annotations

import json
import re
import subprocess
from itertools import product
from pathlib import Path
from typing import NamedTuple, cast

from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR as REUSE_DISABLE_ENV_VAR
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import run_installed_sqb

_TIMING: re.Pattern[str] = re.compile(r" \(\d+\.\d+s\)")
_DIRECTIONS: tuple[str, ...] = ("upstream", "downstream")


class LineageRun(NamedTuple):
    """A compile report and the lineage traces of each target."""

    compile_report: str
    compile_returncode: int
    traces: list[tuple[int, str, str]]
    analysis_cache_hits: int = 0


def lineage_run(
    *,
    project_dir: Path,
    files: dict[str, str],
    targets: tuple[str, ...],
    mode: str = "fast",
    cache_args: tuple[str, ...] = ("--no-cache",),
    environment: tuple[tuple[str, str], ...] = ((REUSE_DISABLE_ENV_VAR, "1"),),
    cold_compiles: int = 0,
) -> LineageRun:
    """Write the project, then compile and trace every target both ways in one lineage mode.

    `cold_compiles` compiles run first, so the measured compile and traces read what they stored.
    """

    for relative_path, contents in files.items():
        path: Path = project_dir / "project" / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    env: dict[str, str] = dict(environment)
    compile_args: tuple[str, ...] = ("compile", "--json", *cache_args, "--lineage-mode", mode)
    for _ in range(cold_compiles):
        cold: subprocess.CompletedProcess[str] = run_installed_sqb(
            project_dir=project_dir / "project", args=compile_args, env=env
        )
        assert cold.returncode == 0, cold.stdout + cold.stderr
    compiled: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir / "project", args=compile_args, env=env
    )
    report: dict[str, object] = json.loads(compiled.stdout.replace(str(project_dir), "<project>"))
    timings: dict[str, int] = cast(dict[str, int], report.pop("compile_timings", {}))
    traces: list[tuple[int, str, str]] = []
    for target, direction in product(targets, _DIRECTIONS):
        result: subprocess.CompletedProcess[str] = run_installed_sqb(
            project_dir=project_dir / "project",
            args=("lineage", target, "--mode", mode, "--direction", direction, "--json"),
            env=env,
        )
        stdout: str = result.stdout.replace(str(project_dir), "<project>")
        traces.append((result.returncode, stdout, _TIMING.sub("", result.stderr)))
    return LineageRun(
        compile_report=json.dumps(report, indent=2),
        compile_returncode=compiled.returncode,
        traces=traces,
        analysis_cache_hits=timings.get("analysis_batch_cache_hits", 0)
        + timings.get("analysis_entry_cache_hits", 0),
    )


class FailedLineageRun(NamedTuple):
    """A failed command's return code and last stderr line."""

    returncode: int
    last_error_line: str


def failed_rich_lineage_run(
    *, project_dir: Path, files: dict[str, str], args: tuple[str, ...], perturbation: str
) -> FailedLineageRun:
    """Run one command with `perturbation` installed as `sitecustomize` in the CLI process."""

    for relative_path, contents in files.items():
        path: Path = project_dir / "project" / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    extlib: Path = project_dir / "extlib"
    extlib.mkdir()
    _ = (extlib / "sitecustomize.py").write_text(perturbation, encoding="utf-8")
    result: subprocess.CompletedProcess[str] = run_installed_sqb(
        project_dir=project_dir / "project",
        args=args,
        env={
            REUSE_DISABLE_ENV_VAR: "1",
            "PYTHONPATH": str(extlib),
            "PYTHONDONTWRITEBYTECODE": "1",
        },
    )
    return FailedLineageRun(
        returncode=result.returncode,
        last_error_line=(result.stderr.strip().splitlines() or [""])[-1],
    )


def model_lineage_summaries(compile_report: str) -> dict[str, object]:
    """The `lineage` summary of every model in a JSON compile report, by model name."""

    payload: dict[str, object] = json.loads(compile_report)
    models: list[dict[str, object]] = cast(
        list[dict[str, object]], cast(dict[str, object], payload["resources"])["models"]
    )
    return {str(model["name"]): model["lineage"] for model in models}
