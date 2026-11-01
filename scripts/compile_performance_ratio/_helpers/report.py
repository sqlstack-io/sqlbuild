"""Render same-runner comparison results for logs and job summaries."""

from __future__ import annotations

import platform
from collections.abc import Mapping
from pathlib import Path

from scripts.compile_performance_ratio.constants import MODE_TITLES, REPORTED_PHASES
from scripts.compile_performance_ratio.models import CompileComparison


def comparison_markdown(
    *,
    comparisons: tuple[CompileComparison, ...],
    runs: int,
    max_ratio: float,
    per_side_projects: bool,
    failures: tuple[str, ...],
    mode_max_ratios: Mapping[str, float] | None = None,
    mode_cpu_max_ratios: Mapping[str, float] | None = None,
    gate_phases: tuple[str, ...] = (),
    compile_args: tuple[str, ...] = (),
) -> str:
    """Summarize medians, ratios and per-phase timings of every mode as Markdown tables."""

    generation: str = (
        "Projects generated per side: base with the base checkout's generator, head with head's."
        if per_side_projects
        else "One project generated with head's generator and compiled by both builds."
    )
    lines: list[str] = []
    if gate_phases:
        lines.extend((f"Gated phases: {', '.join(gate_phases)}.", ""))
    if compile_args:
        lines.extend((f"Compile arguments: `{' '.join(compile_args)}`.", ""))
    for comparison in comparisons:
        lines.extend(
            (
                f"### {comparison.kind} {comparison.models} models, "
                f"{MODE_TITLES[comparison.mode]}: head vs base (same runner)",
                "",
                f"Runner CPU: {cpu_model()}; {runs} alternating runs each; "
                f"limit {(mode_max_ratios or {}).get(comparison.mode, max_ratio):.2f}x.",
                "",
                generation,
                "",
                "| Metric | Base | Head | Ratio |",
                "|---|---:|---:|---:|",
                f"| Wall (s) | {comparison.base_wall_seconds:.2f} | "
                f"{comparison.head_wall_seconds:.2f} | {comparison.wall_ratio:.3f} |",
                f"| CPU (s) | {comparison.base_cpu_seconds:.2f} | "
                f"{comparison.head_cpu_seconds:.2f} | {comparison.cpu_ratio:.3f} |",
            )
        )
        for phase in REPORTED_PHASES:
            base: float = comparison.base_timings_ms.get(phase, 0)
            head: float = comparison.head_timings_ms.get(phase, 0)
            ratio: str = f"{head / base:.3f}" if base else "n/a"
            lines.append(f"| {phase} | {base:.0f} | {head:.0f} | {ratio} |")
        lines.append("")
        if comparison.mode in (mode_cpu_max_ratios or {}):
            lines.append(
                f"CPU limit for {comparison.mode}: "
                f"{(mode_cpu_max_ratios or {})[comparison.mode]:.2f}x (explicit override)."
            )
    if failures:
        lines.append("**Result: failed.** " + "; ".join(failures) + ".")
    else:
        lines.append("**Result: passed.** Every mode is within the same-runner limit.")
    return "\n".join(lines) + "\n"


def append_summary(*, path: Path | None, markdown: str) -> None:
    """Append to the CI job summary when one is configured."""

    if path is None:
        return
    with path.open("a", encoding="utf-8") as summary:
        summary.write(markdown + "\n")


def cpu_model() -> str:
    """Return the runner CPU model name."""

    cpuinfo: Path = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text(encoding="utf-8").splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor() or "unknown"
