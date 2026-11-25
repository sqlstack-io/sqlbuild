"""Each unit test's decision: local when DuckDB can run it, else the warehouse and why."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import asdict, replace
from pathlib import Path
from typing import Any

from sqlbuild._native import SqlTestPlanEntry, render_sql_test_comparison_segments
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.local._helpers.conversion import (
    check_all,
    local_check,
    use_conversion_cache,
)
from sqlbuild.executor.local._helpers.positions import placed_refusal
from sqlbuild.executor.local._helpers.schema import project_schema
from sqlbuild.executor.local._helpers.session import session_time_zone
from sqlbuild.executor.local.constants import (
    CONVERSION_CACHE_DIR,
    CONVERTED_DIALECT,
    DIALECT_REASON_CATEGORY,
    FUNCTION_REASON_CATEGORY,
    LOCAL_DIALECT,
    RELATION_REASON_CATEGORY,
    RUN_ON_LOCAL,
    RUN_ON_WAREHOUSE,
)
from sqlbuild.executor.local.models import (
    DecisionContext,
    LocalConversion,
    LocalRefusal,
    LocalSqlCheck,
    RunOnDecision,
    Segment,
    TestSources,
)


def decision_context(
    *,
    project: Any,
    test_entries: Sequence[SqlTestPlanEntry],
    adapter: BaseAdapter,
    run_on: str,
    location: tuple[Path, Mapping[str, str]],
) -> DecisionContext:
    """The project's functions and schema, and every convertible test's SQL, converted up front."""

    dialect: str | None = adapter.sql_analysis_dialect()
    context: DecisionContext = DecisionContext(
        project=project,
        dialect=dialect,
        run_on=run_on,
        set_difference_operator=adapter.render_set_difference_operator(),
        functions={function.name: function for function in getattr(project, "functions", ())},
        schema={},
        time_zone=session_time_zone(project),
        rendered={},
        project_dir=location[0],
        compiled_paths=location[1],
    )
    if run_on == RUN_ON_WAREHOUSE or dialect not in (CONVERTED_DIALECT, LOCAL_DIALECT):
        return context
    use_conversion_cache(location[0] / CONVERSION_CACHE_DIR)
    rendered: dict[str, tuple[str, list[tuple[int, int, str]]]] = {
        entry.name: _rendered(entry=entry, context=context)
        for entry in test_entries
        if not _blocking_functions(entry=entry, context=context)
    }
    schema: dict[str, Any] = project_schema(project) if dialect == CONVERTED_DIALECT else {}
    _ = check_all(
        sqls=[sql for sql, _segments in rendered.values()],
        schema=schema,
        time_zone=context.time_zone,
        convert=dialect == CONVERTED_DIALECT,
    )
    return replace(context, schema=schema, rendered=rendered)


def decide_each(
    *, context: DecisionContext, test_entries: Sequence[SqlTestPlanEntry]
) -> tuple[RunOnDecision, ...]:
    """Every test's decision from its checked SQL: converted, and bound in an empty DuckDB."""

    return tuple(_decision(entry=entry, context=context) for entry in test_entries)


def decisions_json(decisions: Sequence[RunOnDecision]) -> str:
    """The decisions as the JSON the test command reads."""

    return json.dumps({"decisions": [_decision_payload(decision) for decision in decisions]})


def _decision(*, entry: SqlTestPlanEntry, context: DecisionContext) -> RunOnDecision:
    blocking: list[str] = _blocking_functions(entry=entry, context=context)
    if context.run_on == RUN_ON_WAREHOUSE:
        return RunOnDecision(name=entry.name, run_on=RUN_ON_WAREHOUSE)
    if blocking:
        return _function_decision(name=entry.name, blocking=blocking)
    if context.dialect == LOCAL_DIALECT:
        sql: str = context.rendered[entry.name][0]
        missing: str | None = local_check(
            sql=sql, schema=None, time_zone=context.time_zone, path="", convert=False
        ).missing
        return _bound_decision(name=entry.name, missing=missing)
    if context.dialect != CONVERTED_DIALECT:
        return _dialect_decision(name=entry.name, dialect=context.dialect)
    return _converted_decision(entry=entry, context=context)


def _blocking_functions(*, entry: SqlTestPlanEntry, context: DecisionContext) -> list[str]:
    """Functions the test calls whose temporary copy cannot be made in an empty DuckDB."""

    return [
        key.name
        for key in entry.function_deps
        if context.dialect != LOCAL_DIALECT
        or key.name not in context.functions
        or context.functions[key.name].references
    ]


def _rendered(
    *, entry: SqlTestPlanEntry, context: DecisionContext
) -> tuple[str, list[tuple[int, int, str]]]:
    return render_sql_test_comparison_segments(
        entry,
        set_difference_operator=context.set_difference_operator,
        sql_analysis_dialect=context.dialect,
    )


def _function_decision(*, name: str, blocking: Sequence[str]) -> RunOnDecision:
    return RunOnDecision(
        name=name,
        run_on=RUN_ON_WAREHOUSE,
        refusals=tuple(
            LocalRefusal(
                category=FUNCTION_REASON_CATEGORY,
                construct=function,
                reason=(
                    f"the test calls project function {function}, whose temporary copy "
                    "needs the warehouse"
                ),
            )
            for function in blocking
        ),
    )


def _dialect_decision(*, name: str, dialect: str | None) -> RunOnDecision:
    return RunOnDecision(
        name=name,
        run_on=RUN_ON_WAREHOUSE,
        refusals=(
            LocalRefusal(
                category=DIALECT_REASON_CATEGORY,
                construct=str(dialect),
                reason=f"SQLQuack converts Snowflake SQL only, not {dialect}",
            ),
        ),
    )


def _converted_decision(*, entry: SqlTestPlanEntry, context: DecisionContext) -> RunOnDecision:
    sql: str = context.rendered[entry.name][0]
    check: LocalSqlCheck = local_check(
        sql=sql,
        schema=context.schema,
        time_zone=context.time_zone,
        path=context.compiled_paths.get(entry.name, ""),
    )
    conversion: LocalConversion | None = check.conversion
    if conversion is None or conversion.converted:
        return _bound_decision(name=entry.name, missing=check.missing)
    sources: TestSources = _test_sources(entry=entry, context=context)
    return RunOnDecision(
        name=entry.name,
        run_on=RUN_ON_WAREHOUSE,
        refusals=tuple(
            placed_refusal(refusal=refusal, sources=sources) for refusal in conversion.refusals
        ),
    )


def _test_sources(*, entry: SqlTestPlanEntry, context: DecisionContext) -> TestSources:
    """What the test's comparison SQL was built from, to place refusals back in those files."""

    sql, segments = context.rendered[entry.name]
    test_path, test_text = _test_file(entry=entry, project_dir=context.project_dir)
    return TestSources(
        sql=sql,
        segments=tuple(
            Segment(start=start, end=end, origin=origin) for start, end, origin in segments
        ),
        test_path=test_path,
        test_text=test_text,
        compiled_path=context.compiled_paths.get(entry.name, ""),
        mocked_models=frozenset(entry.mock_ref_names),
        models={model.name: model for model in getattr(context.project, "models", ())},
        chain_sql={step.model_name: step.resolved_sql for step in entry.chain},
        dialect=CONVERTED_DIALECT,
    )


def _test_file(*, entry: SqlTestPlanEntry, project_dir: Path) -> tuple[str | None, str | None]:
    """The test file's project-relative path and text, when it can be read."""

    if entry.source_path is None:
        return None, None
    source: Path = Path(entry.source_path)
    absolute: Path = source if source.is_absolute() else project_dir / source
    relative: str = (
        absolute.relative_to(project_dir).as_posix()
        if absolute.is_relative_to(project_dir)
        else absolute.as_posix()
    )
    try:
        return relative, absolute.read_text(encoding="utf-8")
    except OSError:
        return relative, None


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


def _decision_payload(decision: RunOnDecision) -> dict[str, Any]:
    return {
        "name": decision.name,
        "run_on": decision.run_on,
        "reason": decision.reason,
        "refusals": [_refusal_payload(refusal) for refusal in decision.refusals],
    }


def _refusal_payload(refusal: LocalRefusal) -> dict[str, Any]:
    return {**asdict(refusal), "title": refusal.title}
