"""The declaration scope built natively: index, lookup and grants."""

from __future__ import annotations

import random
from pathlib import Path

import pytest

from sqlbuild._native import ScopeIndex
from tests.integration.src.sqlbuild.compiler.scopes._test_types import (
    GeneratedScopeParityTestCase,
    ScopeCommandParityTestCase,
    ScopeEngineParityTestCase,
)
from tests.integration.src.sqlbuild.compiler.scopes.helpers import (
    SCOPED_PROJECT,
    ScopeOutcome,
    generated_scope_files,
    scope_command_index,
    scope_outcome,
    write_project,
)

_BROKEN_TEST: str = (
    'TEST (name "broken");\n\nWITH\n__source__raw_orders AS (\n  SELECT 1\n),\n'
    "__expected__ AS (\n  SELECT 1\n)\nSELECT 1\n"
)

_SCENARIO_HEADER: str = 'SCENARIO (\n  description "Flow"\n);\n\n'
_MACRO_TEST_HEADER: str = 'TEST (mode macro, name "eu_label");\n\n'


@pytest.mark.parametrize(
    "test_case",
    [
        ScopeEngineParityTestCase(
            description="nested scoped roots, private values, expected-model and macro-test grants",
            files=SCOPED_PROJECT,
            expected_minimum_grants=5,
        ),
        ScopeEngineParityTestCase(
            description="a global enum duplicating a scoped enum fails with Python's diagnostics",
            files={
                **SCOPED_PROJECT,
                "sql/enums/region.sql": "ENUM (name eu_region, members [EAST]);\n",
            },
            expected_error="Duplicate declaration 'enum:eu_region'",
        ),
        ScopeEngineParityTestCase(
            description="a malformed expected-model CTE fails with Python's relationship fault",
            files={**SCOPED_PROJECT, "tests/unit/test_broken.sql": _BROKEN_TEST},
            expected_error="__expected__",
        ),
        ScopeEngineParityTestCase(
            description="a scenario's bare expected prefix fails with Python's scenario fault",
            files={
                **SCOPED_PROJECT,
                "tests/scenarios/flow.sql": _SCENARIO_HEADER
                + "WITH\n__source__raw_orders AS (SELECT 1),\n__expected__ AS (SELECT 1)\n",
            },
            expected_error=(
                "SQL scenario 'tests/scenarios/flow.sql' must use __expected__<model> to "
                "identify a target"
            ),
        ),
        ScopeEngineParityTestCase(
            description="the first of several broken tests and scenarios is reported",
            files={
                **SCOPED_PROJECT,
                "tests/unit/test_a_missing_as.sql": 'TEST (name "a");\n\nWITH\n'
                "__source__raw_orders (SELECT 1),\n__expected__orders AS (SELECT 1)\n",
                "tests/unit/test_b_broken.sql": _BROKEN_TEST,
                "tests/scenarios/flow.sql": _SCENARIO_HEADER
                + "WITH\n__source__raw_orders AS (SELECT 1\n",
            },
            expected_error="SQL test 'tests/unit/test_a_missing_as.sql' expected keyword AS",
        ),
        ScopeEngineParityTestCase(
            description="a macro test whose CTE scan fails reports Python's scanner error",
            files={
                **SCOPED_PROJECT,
                "tests/unit/test_eu_label.sql": _MACRO_TEST_HEADER
                + "WITH\n__macro_actual__ AS SELECT @py.eu_label('1') AS label\n",
            },
            expected_error="SQL test 'tests/unit/test_eu_label.sql' CTE '__macro_actual__' must",
        ),
        ScopeEngineParityTestCase(
            description="a macro test materialization hint fails with the located extraction error",
            files={
                **SCOPED_PROJECT,
                "tests/unit/test_eu_label.sql": _MACRO_TEST_HEADER
                + "WITH\n__macro_actual__ AS MATERIALIZED (\n  SELECT @py.cents('1') AS label\n),\n"
                "__macro_expected__ AS (\n  SELECT 100 AS label\n)\nSELECT 1\n",
            },
            expected_error=(
                "SQL test 'tests/unit/test_eu_label.sql' CTE '__macro_actual__' must not use "
                "AS MATERIALIZED; materialization hints are not supported in SQL test CTEs"
            ),
        ),
        ScopeEngineParityTestCase(
            description="non-ASCII folders keep Python's repr order of path keys",
            files={
                **SCOPED_PROJECT,
                "sql/models/sales/r\u00e9gion/regional.sql": (
                    'MODEL (\n  description "Regional",\n);\n\n'
                    'SELECT order_id, @const("sales_cap") AS cap\nFROM __ref("orders")\n'
                ),
            },
            expected_minimum_grants=5,
        ),
        ScopeEngineParityTestCase(
            description="a test granted a deeper macro's folder through a macro test",
            files={
                **SCOPED_PROJECT,
                "sql/models/sales/eu/_macros/python/rates.py": (
                    'def eu_rate(value: str) -> str:\n    return f"{value} * 2"\n\n\n'
                    "def eu_label(value: str) -> str:\n    return eu_rate(value)\n"
                ),
                "tests/unit/test_eu_label.sql": (
                    'TEST (mode macro, name "eu_label");\n\nWITH\n'
                    "__macro_actual__ AS (\n  SELECT @py.eu_label('1') AS label\n),\n"
                    "__macro_expected__ AS (\n  SELECT 1 * 2 AS label\n)\nSELECT 1\n"
                ),
            },
            expected_minimum_grants=8,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scoped_project_when_building_scope_then_outcome_is_expected(
    test_case: ScopeEngineParityTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)

    outcome: ScopeOutcome = scope_outcome(project_dir=tmp_path)

    assert (
        (outcome.kind == "error") == bool(test_case.expected_error),
        test_case.expected_error in outcome.error,
        outcome.grants >= test_case.expected_minimum_grants,
    ) == (True, True, True), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        GeneratedScopeParityTestCase(
            description="seeded scoped declarations, grants and relationship faults",
            seed=20261007,
            count=120,
            expected_minimum_valid=40,
            expected_minimum_invalid=10,
            expected_minimum_granting=15,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_generated_scoped_projects_when_building_scope_then_outcomes_cover_each_kind(
    test_case: GeneratedScopeParityTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    project_dirs: list[Path] = [tmp_path / f"project_{index}" for index in range(test_case.count)]
    for project_dir in project_dirs:
        write_project(project_dir=project_dir, files=generated_scope_files(rng=rng))

    outcomes: list[ScopeOutcome] = [
        scope_outcome(project_dir=project_dir) for project_dir in project_dirs
    ]

    assert (
        sum(item.kind == "ok" for item in outcomes) >= test_case.expected_minimum_valid,
        sum(item.kind == "error" for item in outcomes) >= test_case.expected_minimum_invalid,
        sum(item.grants > 0 for item in outcomes) >= test_case.expected_minimum_granting,
    ) == (True, True, True), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        ScopeCommandParityTestCase(
            description="the offline scope command index with compile usages and grants",
            files=SCOPED_PROJECT,
            expected_minimum_grants=5,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scoped_project_when_scope_command_builds_index_then_grants_are_kept(
    test_case: ScopeCommandParityTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)

    index: ScopeIndex = scope_command_index(project_dir=tmp_path)

    assert len(index.grants) >= test_case.expected_minimum_grants, test_case.description


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
