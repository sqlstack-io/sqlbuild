"""Build feature alignment command entrypoint."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from scripts.build_features._helpers.drift import find_feature_drift
from scripts.build_features._helpers.tree import parse_cargo_tree, run_cargo_tree
from scripts.build_features.constants import (
    EXTENSION_FEATURES,
    EXTENSION_PACKAGE,
    FEATURE_CRATE,
    SQB_FEATURES,
    SQB_PACKAGE,
    WHEEL_TARGETS,
)
from scripts.build_features.models import FeatureDrift


def run_build_features_check(argv: list[str] | None = None) -> int:
    """Fail when the sqb build and the extension build would compile a shared crate twice."""
    parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    arguments: argparse.Namespace = parser.parse_args(argv)
    drift: list[FeatureDrift] = []
    for target in WHEEL_TARGETS:
        drift.extend(
            find_feature_drift(
                target=target,
                sqb=parse_cargo_tree(
                    run_cargo_tree(
                        root=arguments.root,
                        selection=("--package", SQB_PACKAGE, "--features", SQB_FEATURES),
                        target=target,
                    )
                ),
                extension=parse_cargo_tree(
                    run_cargo_tree(
                        root=arguments.root,
                        selection=(
                            "--package",
                            EXTENSION_PACKAGE,
                            "--features",
                            EXTENSION_FEATURES,
                        ),
                        target=target,
                    )
                ),
            )
        )
    if drift:
        print(
            f"The {SQB_PACKAGE} and {EXTENSION_PACKAGE} builds resolve shared crates with "
            f"different features, so Cargo compiles them twice. Enable the missing features "
            f"in {FEATURE_CRATE}:",
            file=sys.stderr,
        )
        for entry in drift:
            print(
                f"- {entry.target} {entry.crate.side} {entry.crate.package}: "
                f"sqb {_features(entry.sqb_features)}, "
                f"extension {_features(entry.extension_features)}",
                file=sys.stderr,
            )
        return 1
    print(f"The {SQB_PACKAGE} and {EXTENSION_PACKAGE} builds share every common crate.")
    return 0


def _features(features: frozenset[str]) -> str:
    return " | ".join(sorted(entry or "(none)" for entry in features))
