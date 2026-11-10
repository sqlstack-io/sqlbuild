"""Native fast column lineage: Python-oracle goldens, restored projects and internal failures."""

from __future__ import annotations

import logging
import random
from collections import Counter
from dataclasses import replace
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import (
    ProjectColumnLineage,
    compiled_model_record,
    project_with,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.helpers import (
    GoldenEntry,
    golden_differences,
    golden_entry,
    golden_name,
    mismatches,
    read_golden,
)
from tests.integration.src.sqlbuild.compiler.lineage._test_types import (
    FormerlyDeferredLineageTestCase,
    GeneratedLineageParityTestCase,
    RootStarLineageTestCase,
    TruncatedSqlLineageTestCase,
    UnparsedLineageTestCase,
)
from tests.integration.src.sqlbuild.compiler.lineage.helpers import (
    compiled_project,
    fast_column_lineage,
    fast_lineage_work,
    generated_lineage_files,
    lineage_log_records,
    lineage_views,
    without_compact_facts,
)

_MODEL_COUNT: int = 16


@pytest.mark.parametrize(
    "test_case",
    [
        GeneratedLineageParityTestCase(
            description="stars, nested CTEs, set operations, quoting and opted-out models",
            seed=20261008,
            count=4,
            dialects=(None, "duckdb", "postgres", "snowflake"),
            expected_minimum_native=400,
            expected_minimum_parsed=250,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_projects_when_building_fast_lineage_then_restored_projects_agree(
    test_case: GeneratedLineageParityTestCase,
    tmp_path: Path,
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    work: Counter[str] = Counter()
    differences: list[tuple[object, object, object]] = []
    golden: list[GoldenEntry] = []
    for index in range(test_case.count):
        project: CompiledProjectView = compiled_project(
            project_dir=tmp_path / f"project_{index}",
            files=generated_lineage_files(rng=rng, model_count=_MODEL_COUNT),
        )
        selected: frozenset[str] = frozenset(
            rng.sample([model.name for model in project.models], k=_MODEL_COUNT // 2)
        )
        for variant in (project, without_compact_facts(project)):
            for dialect in test_case.dialects:
                for model_names in (None, selected):
                    for _ in ("with catalog", "restored"):
                        work.update(fast_lineage_work(project=variant, model_names=model_names))
                    names, with_catalog = lineage_views(
                        project=variant, dialect=dialect, model_names=model_names
                    )
                    golden.append(
                        golden_entry(
                            "lineage",
                            {
                                f"{position}:{name}": view
                                for position, (name, view) in enumerate(
                                    zip(names, with_catalog, strict=True)
                                )
                            },
                        )
                    )
                    _, restored = lineage_views(
                        project=project_with(project=variant, binding_catalog=None),
                        dialect=dialect,
                        model_names=model_names,
                    )
                    differences.extend(
                        mismatches(inputs=names, expected=with_catalog, actual=restored)
                    )

    assert (
        golden_differences(
            read_golden(golden_name("fast_lineage_generated", test_case.description)), golden
        )
        == []
    )
    assert differences == []
    assert sum(work.values()) >= test_case.expected_minimum_native
    assert work["parse"] >= test_case.expected_minimum_parsed


@pytest.mark.parametrize(
    "test_case",
    [
        RootStarLineageTestCase(
            description=f"a root star over a model without SQL analysis, {dialect} dialect",
            dialect=dialect,
            expected_star_expansions=1,
        )
        for dialect in (None, "duckdb", "postgres", "snowflake")
    ],
    ids=lambda case: case.description,
)
def test_given_root_star_over_opted_out_model_when_building_fast_lineage_then_restored_project_agrees(
    test_case: RootStarLineageTestCase,
    tmp_path: Path,
) -> None:
    project: CompiledProjectView = compiled_project(
        project_dir=tmp_path / "project",
        files={
            **generated_lineage_files(rng=random.Random(5), model_count=_MODEL_COUNT),
            "models/orders_staged.sql": (
                'MODEL (description "Staged orders", sql_analysis false);\n\n'
                'SELECT customer_id, order_id FROM __source("raw_orders")\n'
            ),
            "models/orders_all.sql": (
                'MODEL (description "All orders");\n\nSELECT * FROM __ref("orders_staged")\n'
            ),
        },
    )

    names, with_catalog = lineage_views(
        project=project, dialect=test_case.dialect, model_names=None
    )
    _, restored = lineage_views(
        project=project_with(project=project, binding_catalog=None),
        dialect=test_case.dialect,
        model_names=None,
    )
    work: Counter[str] = fast_lineage_work(project=project, model_names=frozenset({"orders_all"}))

    assert work["star"] == test_case.expected_star_expansions
    assert mismatches(inputs=names, expected=with_catalog, actual=restored) == []


@pytest.mark.parametrize(
    "test_case",
    [
        UnparsedLineageTestCase(
            description="an unterminated subquery",
            query_sql='SELECT order_id FROM (SELECT * FROM __ref("orders_model_0")',
            expected_messages=(
                "fast column lineage not built: the native parser cannot read the model",
            ),
        ),
        UnparsedLineageTestCase(
            description="two statements",
            query_sql="SELECT 1; SELECT 2",
            expected_messages=(
                "fast column lineage not built: the native parser cannot read the model",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unparsable_model_when_building_fast_lineage_then_logs_and_omits_it(
    test_case: UnparsedLineageTestCase,
    tmp_path: Path,
    caplog: pytest.LogCaptureFixture,
) -> None:
    project: CompiledProjectView = compiled_project(
        project_dir=tmp_path / "project",
        files=generated_lineage_files(rng=random.Random(11), model_count=_MODEL_COUNT),
    )
    broken: CompiledProjectView = project_with(
        project=project,
        models=(
            replace(
                compiled_model_record(project.models[0]),
                query_sql=test_case.query_sql,
                fast_lineage_columns=None,
            ),
            *project.models[1:],
        ),
    )

    with caplog.at_level(logging.DEBUG, logger="sqlbuild.lineage"):
        native: ProjectColumnLineage | None = fast_column_lineage(
            project=broken, dialect="duckdb", model_names=None
        )
        native_records: list[tuple[str, str, object]] = lineage_log_records(caplog)

    assert native is not None
    assert not native.has_model(project.models[0].name)
    assert tuple(message for _, message, _ in native_records) == test_case.expected_messages


@pytest.mark.parametrize(
    "test_case",
    [
        FormerlyDeferredLineageTestCase(
            description="a project restored without its analysis catalog",
            dialect="duckdb",
            keeps_catalog=False,
            expected_models_with_lineage=_MODEL_COUNT,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_formerly_deferred_models_when_building_fast_lineage_then_native_answers(
    test_case: FormerlyDeferredLineageTestCase,
    tmp_path: Path,
) -> None:
    compiled: CompiledProjectView = without_compact_facts(
        compiled_project(
            project_dir=tmp_path / "project",
            files=generated_lineage_files(rng=random.Random(7), model_count=_MODEL_COUNT),
        )
    )
    project: CompiledProjectView = project_with(
        project=compiled, binding_catalog=(None, compiled.binding_catalog)[test_case.keeps_catalog]
    )

    lineage: ProjectColumnLineage = cast(
        ProjectColumnLineage,
        fast_column_lineage(project=project, dialect=test_case.dialect, model_names=None),
    )
    models_with_lineage: int = sum(lineage.has_model(model.name) for model in project.models)

    assert models_with_lineage == test_case.expected_models_with_lineage, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        FormerlyDeferredLineageTestCase(
            description="a dialect without a native parser",
            dialect="mysql",
            keeps_catalog=True,
            expected_models_with_lineage=0,
        ),
        FormerlyDeferredLineageTestCase(
            description="a dialect name no parser knows",
            dialect="nonsense",
            keeps_catalog=True,
            expected_models_with_lineage=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_without_native_parser_when_building_fast_lineage_then_no_model_is_answered(
    test_case: FormerlyDeferredLineageTestCase,
    tmp_path: Path,
) -> None:
    compiled: CompiledProjectView = without_compact_facts(
        compiled_project(
            project_dir=tmp_path / "project",
            files=generated_lineage_files(rng=random.Random(7), model_count=_MODEL_COUNT),
        )
    )
    project: CompiledProjectView = project_with(
        project=compiled, binding_catalog=(None, compiled.binding_catalog)[test_case.keeps_catalog]
    )

    lineage: ProjectColumnLineage = cast(
        ProjectColumnLineage,
        fast_column_lineage(project=project, dialect=test_case.dialect, model_names=None),
    )
    models_with_lineage: int = sum(lineage.has_model(model.name) for model in project.models)

    assert models_with_lineage == test_case.expected_models_with_lineage, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        TruncatedSqlLineageTestCase(
            description="truncated T-SQL the parser once panicked on is left out",
            dialect="tsql",
            query_sql="SELECT IF(region > 1, re",
            expected_has_model=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_truncated_sql_when_building_fast_lineage_then_omits_the_model(
    test_case: TruncatedSqlLineageTestCase, tmp_path: Path
) -> None:
    project: CompiledProjectView = compiled_project(
        project_dir=tmp_path / "project",
        files=generated_lineage_files(rng=random.Random(11), model_count=_MODEL_COUNT),
    )
    broken: CompiledProjectView = project_with(
        project=project,
        models=(
            replace(
                compiled_model_record(project.models[0]),
                query_sql=test_case.query_sql,
                fast_lineage_columns=None,
            ),
            *project.models[1:],
        ),
    )

    native: ProjectColumnLineage | None = fast_column_lineage(
        project=broken, dialect=test_case.dialect, model_names=None
    )

    assert native is not None
    assert native.has_model(project.models[0].name) is test_case.expected_has_model


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
