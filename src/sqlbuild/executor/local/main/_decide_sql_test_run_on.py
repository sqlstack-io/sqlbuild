"""Where each unit test runs: locally in DuckDB when its SQL converts, else on the warehouse."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from sqlbuild._native import SqlTestPlanEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.local._helpers.decisions import decide_each, decision_context, decisions_json
from sqlbuild.executor.local.models import DecisionContext, RunOnDecision


def decide_sql_test_run_on(
    *,
    project: Any,
    test_entries: Sequence[SqlTestPlanEntry],
    adapter: BaseAdapter,
    project_dir: Path,
    compiled_paths: Mapping[str, str],
    run_on: str,
) -> str:
    """Each test's decision as JSON: `run_on`, `reason` and the refusals behind it."""

    context: DecisionContext = decision_context(
        project=project,
        test_entries=test_entries,
        adapter=adapter,
        run_on=run_on,
        location=(project_dir, compiled_paths),
    )
    decisions: tuple[RunOnDecision, ...] = decide_each(context=context, test_entries=test_entries)
    return decisions_json(decisions)
