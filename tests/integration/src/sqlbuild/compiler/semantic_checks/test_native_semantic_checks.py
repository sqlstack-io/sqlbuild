"""Native semantic completion matches the outputs recorded from Python's completion."""

from __future__ import annotations

import random
from collections import Counter
from dataclasses import replace
from pathlib import Path

import pytest

import sqlbuild._native as native_module
from sqlbuild._native import (
    project_with,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.helpers import (
    GoldenEntry,
    golden_differences,
    golden_entry,
    golden_name,
    read_golden,
)
from tests.integration.src.sqlbuild.compiler.semantic_checks._test_types import (
    FormerlyDeferredSemanticTestCase,
    GeneratedSemanticParityTestCase,
    MissingCatalogTestCase,
    SessionCompletionTestCase,
)
from tests.integration.src.sqlbuild.compiler.semantic_checks.helpers import (
    SemanticInputs,
    captured_semantic_inputs,
    completed_natively,
    completion_view,
    files_with_non_ascii,
    generated_semantic_files,
    has_blocking_binding,
    native_completion,
    note_kinds,
    outcome_label,
    proven_output_count,
    semantic_outcome,
    session_corpus_files,
    session_fact_models,
    with_dialect,
    without_session,
)

_NOTE_KINDS: tuple[str, ...] = ("downstream output uses", "downstream uses of", " has: ")


@pytest.mark.parametrize(
    "test_case",
    [
        GeneratedSemanticParityTestCase(
            description="typos, aliases, temporal comparisons, type errors and opt-outs",
            seed=20261009,
            count=8,
            model_count=12,
            dialects=("duckdb", "postgres", "snowflake", "bigquery"),
            expected_minimum_native_completions=32,
            expected_minimum_native_diagnostics=200,
            expected_minimum_type_recovery_plans=16,
            expected_minimum_codes={
                "B002": 100,
                "B212": 4,
                "B217": 8,
                "P009": 16,
                "downstream output uses": 16,
                "downstream uses of": 8,
                " has: ": 40,
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_failing_projects_when_completing_natively_then_matches_recorded_python(
    test_case: GeneratedSemanticParityTestCase,
    tmp_path: Path,
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    statuses: Counter[str] = Counter()
    codes: Counter[str] = Counter()
    golden: list[GoldenEntry] = []
    for index in range(test_case.count):
        project_dir: Path = tmp_path / f"project_{index}"
        captured: SemanticInputs = captured_semantic_inputs(
            project_dir=project_dir,
            files=generated_semantic_files(
                rng=rng, model_count=test_case.model_count, require_analysis=index % 2 == 0
            ),
        )
        views: dict[str, object] = {}
        for dialect in test_case.dialects:
            native: CompiledProjectView = completed_natively(with_dialect(captured, dialect))
            statuses["completion_native"] += 1
            statuses["diagnostics_native"] += len(native.diagnostics)
            statuses["type_recovery_planned"] += has_blocking_binding(captured.project)
            codes.update(item.code for item in native.diagnostics)
            codes.update(note_kinds(native, _NOTE_KINDS))
            views[dialect] = completion_view(native)
        golden.append(golden_entry("completion", views, masked=(str(project_dir),)))

    assert (
        golden_differences(
            read_golden(golden_name("semantic_generated", test_case.description)), golden
        )
        == []
    )
    assert statuses["type_recovery_planned"] >= test_case.expected_minimum_type_recovery_plans
    assert statuses["completion_native"] >= test_case.expected_minimum_native_completions
    assert statuses["diagnostics_native"] >= test_case.expected_minimum_native_diagnostics
    assert {
        code: min(codes[code], minimum)
        for code, minimum in test_case.expected_minimum_codes.items()
    } == test_case.expected_minimum_codes


@pytest.mark.parametrize(
    "test_case",
    [
        FormerlyDeferredSemanticTestCase(
            description="non-ASCII names, comments and line separators",
            seed=20261014,
            count=4,
            non_ascii=True,
            dialects=("duckdb", "postgres", "snowflake", "bigquery"),
            expected_outcomes=frozenset({"completed"}),
        ),
        FormerlyDeferredSemanticTestCase(
            description="dialects added to the native parser build",
            seed=20261015,
            count=2,
            non_ascii=False,
            dialects=("tsql", "databricks", "sqlite", "clickhouse"),
            expected_outcomes=frozenset({"completed"}),
        ),
        FormerlyDeferredSemanticTestCase(
            description="no dialect and an unknown dialect raise the wheel's errors",
            seed=20261016,
            count=2,
            non_ascii=True,
            dialects=(None, "nonsense"),
            expected_outcomes=frozenset(
                {
                    "TypeError: argument 'dialect': 'None' is not an instance of 'str'",
                    "ValueError: Unknown dialect: nonsense",
                }
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_formerly_deferred_projects_when_completing_natively_then_matches_recorded_python(
    test_case: FormerlyDeferredSemanticTestCase,
    tmp_path: Path,
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    golden: list[GoldenEntry] = []
    outcomes: set[str] = set()
    for index in range(test_case.count):
        project_dir: Path = tmp_path / f"project_{index}"
        files: dict[str, str] = generated_semantic_files(
            rng=rng, model_count=12, require_analysis=index % 2 == 0
        )
        captured: SemanticInputs = captured_semantic_inputs(
            project_dir=project_dir,
            files=files_with_non_ascii(files=files, rng=rng, non_ascii=test_case.non_ascii),
        )
        views: dict[str, object] = {}
        for dialect in test_case.dialects:
            inputs: SemanticInputs = with_dialect(captured, dialect)
            outcome: tuple[str, object] = semantic_outcome(
                lambda inputs=inputs: completion_view(completed_natively(inputs))
            )
            outcomes.add(outcome_label(outcome))
            views[str(dialect)] = outcome
        golden.append(golden_entry("completion", views, masked=(str(project_dir),)))

    assert (
        golden_differences(
            read_golden(golden_name("semantic_formerly_deferred", test_case.description)), golden
        )
        == []
    )
    assert outcomes == test_case.expected_outcomes


@pytest.mark.parametrize(
    "test_case",
    [
        MissingCatalogTestCase(
            description="a project without an analysis catalog is an internal failure",
            expected_message=(
                "NativeCompilerError: native semantic completion: "
                "the project has no analysis catalog"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_without_catalog_when_completing_then_raises_native_compiler_error(
    test_case: MissingCatalogTestCase,
    tmp_path: Path,
) -> None:
    captured: SemanticInputs = captured_semantic_inputs(
        project_dir=tmp_path / "project",
        files=generated_semantic_files(rng=random.Random(5), model_count=8, require_analysis=False),
    )

    with pytest.raises(native_module.NativeCompilerError) as raised:
        _ = native_completion(
            replace(captured, project=project_with(project=captured.project, binding_catalog=None))
        )

    assert str(raised.value) == test_case.expected_message


@pytest.mark.parametrize(
    "test_case",
    [
        SessionCompletionTestCase(
            description="failing semantics corpus",
            corpus="semantic",
            seed=20261010,
            count=6,
            model_count=12,
            expected_session_models=60,
            expected_payload_models=12,
            expected_proven_outputs=0,
            expected_minimum_codes={"B002": 1, "downstream output uses": 1},
        ),
        SessionCompletionTestCase(
            description="analysis session corpus with pivots, stars and set operations",
            corpus="analysis",
            seed=20261011,
            count=4,
            model_count=16,
            expected_session_models=78,
            expected_payload_models=18,
            expected_proven_outputs=10,
            expected_minimum_codes={"B002": 1},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compiles_when_completing_from_the_session_then_matches_payload_and_python(
    test_case: SessionCompletionTestCase,
    tmp_path: Path,
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    captured: list[SemanticInputs] = [
        captured_semantic_inputs(
            project_dir=tmp_path / f"project_{index}",
            files=session_corpus_files(
                corpus=test_case.corpus, rng=rng, model_count=test_case.model_count
            ),
        )
        for index in range(test_case.count)
    ]
    selections: list[frozenset[str]] = [session_fact_models(inputs) for inputs in captured]
    codes: Counter[str] = Counter()
    golden: list[GoldenEntry] = []
    payload_views: list[tuple[object, ...]] = []
    session_views: list[tuple[object, ...]] = []
    for index, inputs in enumerate(captured):
        completed: CompiledProjectView = completed_natively(inputs)
        codes.update(item.code for item in completed.diagnostics)
        codes.update(note_kinds(completed, _NOTE_KINDS))
        session_views.append(completion_view(completed))
        payload_views.append(completion_view(completed_natively(without_session(inputs))))
        golden.append(
            golden_entry(
                "completion", session_views[-1], masked=(str(tmp_path / f"project_{index}"),)
            )
        )
    session_models: int = sum(len(selected) for selected in selections)
    model_count: int = sum(len(inputs.project.models) for inputs in captured)

    assert all(inputs.session is not None for inputs in captured)
    assert (
        golden_differences(
            read_golden(golden_name("semantic_session", test_case.description)), golden
        )
        == []
    )
    assert payload_views == session_views
    assert (
        session_models,
        model_count - session_models,
        sum(proven_output_count(inputs.project) for inputs in captured),
    ) == (
        test_case.expected_session_models,
        test_case.expected_payload_models,
        test_case.expected_proven_outputs,
    )
    assert {
        code: min(codes[code], minimum)
        for code, minimum in test_case.expected_minimum_codes.items()
    } == test_case.expected_minimum_codes


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
