"""Print why a main comparison run did not finish, or nothing when it needs no report."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import cast

from scripts.release_performance_outcome._helpers.outcome import unfinished_reason
from scripts.release_performance_outcome.constants import (
    FAILURE_ANNOTATION_LEVEL,
    REGRESSED_OUTPUT,
)


def report_unfinished_comparison(argv: list[str] | None = None) -> int:
    """Print the reason a compare job result needs an issue; print nothing otherwise."""

    parser: argparse.ArgumentParser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--result", required=True, help="The compare job's result.")
    parser.add_argument("--regressed", default="", help="The compare job's regressed output.")
    parser.add_argument(
        "--annotations", type=Path, required=True, help="The compare job's check-run annotations."
    )
    arguments: argparse.Namespace = parser.parse_args(argv)
    annotations: list[dict[str, object]] = cast(
        list[dict[str, object]], json.loads(Path(arguments.annotations).read_text("utf-8"))
    )
    reason: str | None = unfinished_reason(
        result=str(arguments.result),
        regressed=arguments.regressed == REGRESSED_OUTPUT,
        annotations=tuple(
            str(annotation.get("message", ""))
            for annotation in annotations
            if annotation.get("annotation_level") == FAILURE_ANNOTATION_LEVEL
        ),
    )
    if reason is not None:
        print(reason)
    return 0
