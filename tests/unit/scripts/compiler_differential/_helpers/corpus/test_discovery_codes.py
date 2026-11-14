"""Every discovery error code has a failure-corpus case, checked without compiling the corpus."""

from __future__ import annotations

import pytest

from scripts.compiler_differential._helpers.corpus.emitted_codes import discovery_error_codes
from scripts.compiler_differential._helpers.refactor_corpus.refactor_cases import refactor_cases
from scripts.compiler_differential.constants import DISCOVERY_UNREACHABLE_CODES
from scripts.compiler_differential.main.failure_cases import failure_cases
from tests.unit.scripts.compiler_differential._helpers.corpus._test_types import (
    DiscoveryCodesTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [DiscoveryCodesTestCase(description="failure_corpus", expected_undeclared_codes=frozenset())],
    ids=lambda case: case.description,
)
def test_given_discovery_error_codes_when_reading_the_failure_corpus_then_each_has_a_case(
    test_case: DiscoveryCodesTestCase,
) -> None:
    declared: set[str] = {project.expected.error_code for project in refactor_cases()}
    for case in failure_cases():
        declared.add(case.expected_code)
        declared.update(case.expected_codes or ())

    undeclared: frozenset[str] = (
        discovery_error_codes() - frozenset(DISCOVERY_UNREACHABLE_CODES) - frozenset(declared)
    )

    assert undeclared == test_case.expected_undeclared_codes, test_case.description
