from pathlib import Path

import pytest

from scripts.parse_guard._helpers.scan import get_parse_guard_errors, unguarded_sites
from tests.unit.scripts.parse_guard._helpers._test_types import (
    RepositoryParseGuardTestCase,
    UnguardedSiteTestCase,
)

_REPOSITORY_ROOT: Path = Path(__file__).resolve().parents[5]


@pytest.mark.parametrize(
    "test_case",
    [
        UnguardedSiteTestCase(
            description="a bare Polyglot parse across lines",
            text="fn a() {\n    Dialect::get(d)\n        .parse(sql)\n}\n",
            expected_sites=(
                r"x.rs:2 Dialect::get(?:_by_name)?\([^;]*?\)\s*(?:\.[a-z_]+\([^;]*?\)\??\s*)*?"
                r"\.parse\(",
            ),
        ),
        UnguardedSiteTestCase(
            description="SQLQuack's default options",
            text="fn b() { parse_strict(sql, d, &ParseOptions::default()); }",
            expected_sites=(r"x.rs:1 ParseOptions::default\(\)",),
        ),
        UnguardedSiteTestCase(
            description="a local depth instead of the shared constant",
            text="fn c() { Guard { max_function_call_depth: Some(512) } }",
            expected_sites=(
                r"x.rs:1 max_function_call_depth: Some\((?!MAX_FUNCTION_CALL_DEPTH\))",
            ),
        ),
        UnguardedSiteTestCase(
            description="an analysis options literal without a guard",
            text="fn d() { let o = AnalyzeQueryOptions { dialect, schema: Some(S { t }) }; }",
            expected_sites=("x.rs:1 AnalyzeQueryOptions {",),
        ),
        UnguardedSiteTestCase(
            description="the shared options and constant",
            text=(
                "fn e() -> AnalyzeQueryOptions { AnalyzeQueryOptions { complexity_guard: g } }\n"
                "fn f() { Dialect::get(d).parse_with_options(sql, &polyglot_parse_options()); }\n"
                "fn g() { R { max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH) } }\n"
            ),
            expected_sites=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_parse_text_when_scanning_then_reports_each_default_guarded_site(
    test_case: UnguardedSiteTestCase,
) -> None:
    assert unguarded_sites(label="x.rs", text=test_case.text) == test_case.expected_sites


@pytest.mark.parametrize(
    "test_case",
    [
        RepositoryParseGuardTestCase(
            description="every workspace parse uses the shared guard",
            expected_errors=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_repository_when_checking_parse_guard_then_returns_expected_errors(
    test_case: RepositoryParseGuardTestCase,
) -> None:
    assert get_parse_guard_errors(_REPOSITORY_ROOT) == test_case.expected_errors
