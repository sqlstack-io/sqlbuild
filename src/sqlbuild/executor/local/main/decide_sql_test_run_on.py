"""Where each unit test runs: locally in DuckDB when its SQL converts, else on the warehouse."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import asdict
from pathlib import Path
from typing import Any

from sqlbuild._native import SqlTestPlanEntry, render_sql_test_comparison_segments
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.local._helpers.binding import EmptyDuckDb
from sqlbuild.executor.local._helpers.conversion import convert_all, converted
from sqlbuild.executor.local._helpers.positions import Segment, TestSources, placed_refusal
from sqlbuild.executor.local._helpers.schema import project_schema
from sqlbuild.executor.local._helpers.session import session_time_zone
from sqlbuild.executor.local.constants import (
    CONVERTED_DIALECT,
    DIALECT_REASON_CATEGORY,
    FUNCTION_REASON_CATEGORY,
    LOCAL_DIALECT,
    RELATION_REASON_CATEGORY,
    RUN_ON_LOCAL,
    RUN_ON_WAREHOUSE,
)
from sqlbuild.executor.local.models import LocalRefusal, RunOnDecision


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

    decisions: list[RunOnDecision] = []
    dialect: str | None = adapter.sql_analysis_dialect()
    schema: dict[str, Any] | None = None
    time_zone: str | None = session_time_zone(project)
    functions: dict[str, Any] = {
        function.name: function for function in getattr(project, "functions", ())
    }

    def warehouse_functions(entry: SqlTestPlanEntry) -> list[str]:
        """Functions the test calls whose temporary copy cannot be made in an empty DuckDB."""

        return [
            key.name
            for key in entry.function_deps
            if dialect != LOCAL_DIALECT
            or key.name not in functions
            or functions[key.name].references
        ]

    rendered: dict[str, tuple[str, list[tuple[int, int, str]]]] = {}
    if run_on != RUN_ON_WAREHOUSE and dialect == CONVERTED_DIALECT:
        schema = project_schema(project)
        for entry in (entry for entry in test_entries if not warehouse_functions(entry)):
            rendered[entry.name] = render_sql_test_comparison_segments(
                entry,
                set_difference_operator=adapter.render_set_difference_operator(),
                sql_analysis_dialect=dialect,
            )
        convert_all(sqls=[sql for sql, _ in rendered.values()], schema=schema, time_zone=time_zone)
    empty: EmptyDuckDb = EmptyDuckDb()
    for entry in test_entries:
        blocking: list[str] = warehouse_functions(entry)
        if run_on == RUN_ON_WAREHOUSE:
            decisions.append(RunOnDecision(name=entry.name, run_on=RUN_ON_WAREHOUSE))
        elif blocking:
            decisions.append(
                RunOnDecision(
                    name=entry.name,
                    run_on=RUN_ON_WAREHOUSE,
                    refusals=tuple(
                        LocalRefusal(
                            category=FUNCTION_REASON_CATEGORY,
                            construct=name,
                            reason=(
                                f"the test calls project function {name}, whose temporary copy "
                                "needs the warehouse"
                            ),
                        )
                        for name in blocking
                    ),
                )
            )
        elif dialect == LOCAL_DIALECT:
            sql: str = render_sql_test_comparison_segments(
                entry,
                set_difference_operator=adapter.render_set_difference_operator(),
                sql_analysis_dialect=dialect,
            )[0]
            decisions.append(
                _bound_decision(name=entry.name, missing=empty.missing_relation(sql=sql, macros=()))
            )
        elif dialect != CONVERTED_DIALECT:
            decisions.append(
                RunOnDecision(
                    name=entry.name,
                    run_on=RUN_ON_WAREHOUSE,
                    refusals=(
                        LocalRefusal(
                            category=DIALECT_REASON_CATEGORY,
                            construct=str(dialect),
                            reason=f"SQLQuack converts Snowflake SQL only, not {dialect}",
                        ),
                    ),
                )
            )
        else:
            decisions.append(
                _converted_decision(
                    entry=entry,
                    rendered=rendered[entry.name],
                    project=project,
                    schema=schema or {},
                    time_zone=time_zone,
                    location=(project_dir, compiled_paths.get(entry.name, "")),
                    empty=empty,
                )
            )
    empty.close()
    return json.dumps(
        {
            "decisions": [
                {
                    "name": decision.name,
                    "run_on": decision.run_on,
                    "reason": decision.reason,
                    "refusals": [
                        {**asdict(refusal), "title": refusal.title} for refusal in decision.refusals
                    ],
                }
                for decision in decisions
            ]
        }
    )


def _converted_decision(
    *,
    entry: SqlTestPlanEntry,
    rendered: tuple[str, list[tuple[int, int, str]]],
    project: Any,
    schema: dict[str, Any],
    time_zone: str | None,
    location: tuple[Path, str],
    empty: EmptyDuckDb,
) -> RunOnDecision:
    project_dir, compiled_path = location
    sql, segments = rendered
    test_path: str | None = None
    test_text: str | None = None
    if entry.source_path is not None:
        source: Path = Path(entry.source_path)
        absolute: Path = source if source.is_absolute() else project_dir / source
        test_path = _relative(absolute, project_dir)
        try:
            test_text = absolute.read_text(encoding="utf-8")
        except OSError:
            test_text = None
    conversion: Any = converted(sql=sql, schema=schema, time_zone=time_zone, path=compiled_path)
    if conversion.converted:
        return _bound_decision(
            name=entry.name,
            missing=empty.missing_relation(sql=conversion.sql, macros=conversion.macros),
        )
    sources: TestSources = TestSources(
        sql=sql,
        segments=tuple(
            Segment(start=start, end=end, origin=origin) for start, end, origin in segments
        ),
        test_path=test_path,
        test_text=test_text,
        compiled_path=compiled_path,
        mocked_models=frozenset(entry.mock_ref_names),
        models={model.name: model for model in getattr(project, "models", ())},
        chain_sql={step.model_name: step.resolved_sql for step in entry.chain},
        dialect=CONVERTED_DIALECT,
    )
    return RunOnDecision(
        name=entry.name,
        run_on=RUN_ON_WAREHOUSE,
        refusals=tuple(
            placed_refusal(refusal=refusal, sources=sources)
            for refusal in conversion.report["refusals"]
        ),
    )


def _bound_decision(*, name: str, missing: str | None) -> RunOnDecision:
    """Local, unless the test reads a relation only the warehouse has."""

    if missing is None:
        return RunOnDecision(name=name, run_on=RUN_ON_LOCAL)
    return RunOnDecision(
        name=name,
        run_on=RUN_ON_WAREHOUSE,
        refusals=(
            LocalRefusal(
                category=RELATION_REASON_CATEGORY,
                construct=missing,
                reason=f"the test reads {missing}, which only the warehouse has",
            ),
        ),
    )


def _relative(path: Path, project_dir: Path) -> str:
    try:
        return path.relative_to(project_dir).as_posix()
    except ValueError:
        return path.as_posix()
