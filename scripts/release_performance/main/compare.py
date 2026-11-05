"""Fail when a release candidate is materially slower or larger than the previous release."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.release_performance._helpers.workflow import run_release_comparison
from scripts.release_performance.constants import (
    DEFAULT_BUILD_MODELS,
    DEFAULT_DENSE_MODELS,
    DEFAULT_INSPECTION_MODELS,
    DEFAULT_PYTHON,
    DEFAULT_RUNS,
)
from scripts.release_performance.models import ComparisonOptions


def compare_release_performance(argv: list[str] | None = None) -> int:
    """Compare the candidate with the baseline release and enforce the release limits."""

    return run_release_comparison(options=_parse_args(argv))


def _parse_args(argv: list[str] | None) -> ComparisonOptions:
    parser: argparse.ArgumentParser = argparse.ArgumentParser(
        description=(
            "Time sqb commands for a release candidate and the previous published release on "
            "this machine, and fail when the candidate is materially slower or larger. Each "
            "version is a PyPI version, a wheel path, or an existing virtual environment."
        )
    )
    parser.add_argument("--candidate", required=True)
    parser.add_argument(
        "--baseline",
        default=None,
        help="Defaults to the highest version on PyPI below the candidate.",
    )
    parser.add_argument(
        "--same-version-baseline",
        action="store_true",
        help=(
            "The candidate is an unreleased commit carrying the latest release's version, so "
            "that release is the baseline once it is on PyPI."
        ),
    )
    parser.add_argument("--runs", type=int, default=DEFAULT_RUNS)
    parser.add_argument("--python", default=DEFAULT_PYTHON)
    parser.add_argument("--inspection-models", type=int, default=DEFAULT_INSPECTION_MODELS)
    parser.add_argument("--build-models", type=int, default=DEFAULT_BUILD_MODELS)
    parser.add_argument("--dense-models", type=int, default=DEFAULT_DENSE_MODELS)
    parser.add_argument(
        "--baseline-source",
        type=Path,
        default=None,
        help=(
            "Source tree whose benchmark generator writes the baseline's projects. Defaults to "
            "the baseline's release tag extracted from this repository."
        ),
    )
    parser.add_argument("--work-dir", type=Path, default=None)
    parser.add_argument("--output", type=Path, default=None)
    args: argparse.Namespace = parser.parse_args(argv)
    if args.runs < 1:
        parser.error("--runs must be at least 1")
    return ComparisonOptions(
        candidate=args.candidate,
        baseline=args.baseline,
        runs=args.runs,
        python=args.python,
        inspection_models=args.inspection_models,
        build_models=args.build_models,
        dense_models=args.dense_models,
        work_dir=args.work_dir,
        output=args.output,
        baseline_source=args.baseline_source,
        same_version_baseline=args.same_version_baseline,
    )
