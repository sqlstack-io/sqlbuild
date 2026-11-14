"""Every discovery error code has a failure-corpus case with a golden, checked without compiling the corpus."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.compiler_differential._helpers.corpus.emitted_codes import discovery_error_codes
from scripts.compiler_differential._helpers.refactor_corpus.refactor_cases import refactor_cases
from scripts.compiler_differential.constants import (
    DISCOVERY_UNREACHABLE_CODES,
    GOLDEN_DIRECTORY,
    GOLDEN_SUFFIX,
)
from scripts.compiler_differential.main.failure_cases import failure_cases
from tests.unit.scripts.compiler_differential._helpers.corpus._test_types import (
    DiscoveryCodesTestCase,
    FailureGoldensTestCase,
)

_REPO_ROOT: Path = Path(__file__).resolve().parents[6]


@pytest.mark.parametrize(
    "test_case",
    [DiscoveryCodesTestCase(description="failure_corpus", expected_undeclared_codes=frozenset())],
    ids=lambda case: case.description,
)
def test_given_discovery_error_codes_when_reading_the_failure_corpus_then_each_has_a_case(
    test_case: DiscoveryCodesTestCase,
) -> None:
    declared: set[str | None] = {project.expected.error_code for project in refactor_cases()}
    for case in failure_cases():
        declared.add(case.expected_code)
        declared.update(case.expected_codes or ())

    undeclared: frozenset[str] = (
        discovery_error_codes() - frozenset(DISCOVERY_UNREACHABLE_CODES) - frozenset(declared)
    )

    assert undeclared == test_case.expected_undeclared_codes, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        FailureGoldensTestCase(
            description="failure_and_refactor_cases",
            golden_dir=_REPO_ROOT / GOLDEN_DIRECTORY,
            expected_missing=frozenset(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_failure_corpus_when_reading_the_goldens_then_each_case_has_one(
    test_case: FailureGoldensTestCase,
) -> None:
    names: list[str] = [f"failure/{case.name}" for case in failure_cases()]
    names.extend(project.name for project in refactor_cases())

    expected: frozenset[Path] = frozenset(
        test_case.golden_dir / f"{name}{GOLDEN_SUFFIX}" for name in names
    )
    missing: frozenset[str] = frozenset(
        path.relative_to(test_case.golden_dir).as_posix()
        for path in expected - frozenset(test_case.golden_dir.rglob("*"))
    )

    assert missing == test_case.expected_missing, test_case.description
