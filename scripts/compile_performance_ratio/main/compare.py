"""Fail when head compiles slower than base by more than a ratio on the same runner."""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
from pathlib import Path

from scripts.compile_performance_ratio._helpers.measure import (
    compare_builds,
    write_base_benchmark_project,
    write_benchmark_project,
)
from scripts.compile_performance_ratio._helpers.report import append_summary, comparison_markdown
from scripts.compile_performance_ratio._helpers.verdict import phase_failures, ratio_failures
from scripts.compile_performance_ratio.constants import (
    COMPILE_MODES,
    DEFAULT_MAX_RATIO,
    DEFAULT_RUNS,
    EDIT_MODE,
    NOISE_FLOOR_SECONDS,
    PHASE_NOISE_FLOOR_MS,
    PROJECT_KINDS,
    REPORTED_PHASES,
)
from scripts.compile_performance_ratio.models import CompileComparison


def compare_compile_performance(argv: list[str] | None = None) -> int:
    """Compare base and head compiles per mode and enforce the wall and CPU ratio limit."""

    args: argparse.Namespace = _parse_args(argv)
    summary_value: str | None = os.environ.get("GITHUB_STEP_SUMMARY")
    per_side_projects: bool = args.base_root is not None
    with tempfile.TemporaryDirectory(prefix="sqlbuild-ratio-") as root:
        project_name: str = f"{args.kind}_{args.models}"
        head_project_dir: Path = Path(root) / "head" / project_name
        base_project_dir: Path = (
            Path(root) / "base" / project_name if per_side_projects else head_project_dir
        )
        print(f"Generating {args.kind} project with {args.models} models", file=sys.stderr)
        write_benchmark_project(kind=args.kind, project_dir=head_project_dir, models=args.models)
        if args.base_root is not None:
            print(f"Generating the base project with {args.base_root}'s generator", file=sys.stderr)
            write_base_benchmark_project(
                base_root=args.base_root.absolute(),
                python=args.base_python.absolute(),
                kind=args.kind,
                project_dir=base_project_dir,
                models=args.models,
            )
        modes: tuple[str, ...] = tuple(dict.fromkeys(args.modes))
        comparisons: tuple[CompileComparison, ...] = compare_builds(
            kind=args.kind,
            models=args.models,
            base_project_dir=base_project_dir,
            head_project_dir=head_project_dir,
            base_python=args.base_python,
            head_python=args.head_python,
            runs=args.runs,
            modes=modes,
            compile_args=tuple(args.compile_args),
        )
    mode_max_ratios: dict[str, float] = (
        {} if args.edit_max_ratio is None else {EDIT_MODE: args.edit_max_ratio}
    )
    gate_phases: tuple[str, ...] = tuple(dict.fromkeys(args.gate_phases))
    mode_cpu_max_ratios: dict[str, float] = (
        {} if args.edit_max_cpu_ratio is None else {EDIT_MODE: args.edit_max_cpu_ratio}
    )
    failures: tuple[str, ...] = (
        phase_failures(
            comparisons=comparisons,
            modes=modes,
            phases=gate_phases,
            max_ratio=args.max_ratio,
            noise_floor_ms=args.phase_noise_floor_ms,
        )
        if gate_phases
        else ratio_failures(
            comparisons=comparisons,
            modes=modes,
            max_ratio=args.max_ratio,
            noise_floor_seconds=args.noise_floor_seconds,
            mode_max_ratios=mode_max_ratios,
            mode_cpu_max_ratios=mode_cpu_max_ratios,
        )
    )
    markdown: str = comparison_markdown(
        comparisons=comparisons,
        runs=args.runs,
        max_ratio=args.max_ratio,
        per_side_projects=per_side_projects,
        failures=failures,
        mode_max_ratios=mode_max_ratios,
        mode_cpu_max_ratios=mode_cpu_max_ratios,
        gate_phases=gate_phases,
        compile_args=tuple(args.compile_args),
    )
    print(markdown)
    _ = append_summary(path=Path(summary_value) if summary_value else None, markdown=markdown)
    if failures:
        print("Compile performance regression: " + "; ".join(failures), file=sys.stderr)
        return 1
    print("Compile performance within the same-runner limit", file=sys.stderr)
    return 0


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser: argparse.ArgumentParser = argparse.ArgumentParser(
        description="Compare base and head cold, warm and one-model edit compiles on one machine."
    )
    parser.add_argument("--kind", choices=PROJECT_KINDS, required=True)
    parser.add_argument("--models", type=int, required=True)
    parser.add_argument("--base-python", type=Path, required=True)
    parser.add_argument("--head-python", type=Path, required=True)
    parser.add_argument(
        "--base-root",
        type=Path,
        default=None,
        help="Base checkout whose own generator writes the project the base build compiles.",
    )
    parser.add_argument(
        "--modes",
        nargs="+",
        choices=COMPILE_MODES,
        default=list(COMPILE_MODES),
        help="Compile modes to compare, in order; warm and edit reuse the cold project.",
    )
    parser.add_argument("--runs", type=int, default=DEFAULT_RUNS)
    parser.add_argument("--max-ratio", type=float, default=DEFAULT_MAX_RATIO)
    parser.add_argument(
        "--edit-max-cpu-ratio",
        type=float,
        default=None,
        help="CPU ratio limit for edit mode only; defaults to the mode's ratio limit.",
    )
    parser.add_argument(
        "--edit-max-ratio",
        type=float,
        default=None,
        help="Ratio limit of the one-model edit mode only; defaults to --max-ratio.",
    )
    parser.add_argument(
        "--noise-floor-seconds",
        type=float,
        default=NOISE_FLOOR_SECONDS,
        help="Seconds of slack added to the ratio allowance before a slowdown fails.",
    )
    parser.add_argument(
        "--compile-arg",
        dest="compile_args",
        action="append",
        default=[],
        help="Extra `sqb compile` argument for both builds, e.g. `--compile-arg=--select=*`.",
    )
    parser.add_argument(
        "--gate-phase",
        dest="gate_phases",
        action="append",
        choices=REPORTED_PHASES,
        default=[],
        help="Gate only this reported phase's median instead of whole-compile wall and CPU.",
    )
    parser.add_argument(
        "--phase-noise-floor-ms",
        type=float,
        default=PHASE_NOISE_FLOOR_MS,
        help="Milliseconds of slack added to a gated phase's ratio allowance.",
    )
    return parser.parse_args(argv)
