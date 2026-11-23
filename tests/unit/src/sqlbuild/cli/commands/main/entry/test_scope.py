"""Scope CLI parser and dispatch tests."""

from __future__ import annotations

from dataclasses import FrozenInstanceError

import pytest

from sqlbuild._native import (
    ScopeCommandRequest,
)
from sqlbuild._native import run_cli as _main_with_dependencies
from tests.unit.src.sqlbuild.cli.commands.main.entry._test_types import (
    ScopeArgumentMisuseCase,
    ScopeEntryCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.entry.helpers import build_handlers


@pytest.mark.parametrize(
    "test_case", (ScopeEntryCase("all_flags", 7),), ids=lambda case: case.description
)
def test_given_all_scope_flags_when_dispatching_then_builds_typed_frozen_request(
    test_case: ScopeEntryCase,
) -> None:
    received: list[ScopeCommandRequest] = []

    def record(request: ScopeCommandRequest) -> int:
        received.append(request)
        return 7

    exit_code: int = _main_with_dependencies(
        argv=(
            "--no-color",
            "scope",
            "model:orders",
            "--as-path",
            "sql/models/marts/orders.sql",
            "--defined-under",
            "sql/macros/shared/",
            "--kind",
            "macro",
            "--kind",
            "enum",
            "--match",
            "normal*",
            "--used-only",
            "--include-nearby",
            "--nearby-depth",
            "2",
            "--dependency-depth",
            "3",
            "--explain",
            "macro:normalize",
            "--globals",
            "all",
            "--page-size",
            "5",
            "--after",
            "macro:before",
            "--paths",
            "compact",
            "--json",
            "--no-cache",
            "--verbose",
        ),
        handlers=build_handlers(run_scope=record),
    )

    assert exit_code == test_case.expected_exit_code
    assert received == [
        ScopeCommandRequest(
            target="model:orders",
            as_path="sql/models/marts/orders.sql",
            defined_under="sql/macros/shared/",
            kinds=("macro", "enum"),
            match="normal*",
            used_only=True,
            include_nearby=True,
            nearby_depth=2,
            dependency_depth=3,
            explain="macro:normalize",
            globals="all",
            page_size=5,
            after="macro:before",
            paths="compact",
            json_output=True,
            no_cache=True,
            verbose=True,
            no_color=True,
        )
    ]
    with pytest.raises(FrozenInstanceError):
        received[0].verbose = False


@pytest.mark.parametrize(
    "test_case",
    (
        ScopeArgumentMisuseCase("missing_target", ("scope",)),
        ScopeArgumentMisuseCase("target_and_at", ("scope", "model:x", "--at", "sql/models/x.sql")),
        ScopeArgumentMisuseCase(
            "move_without_target",
            ("scope", "--at", "sql/models/x.sql", "--as-path", "sql/models/y.sql"),
        ),
        ScopeArgumentMisuseCase("zero_page", ("scope", "model:x", "--page-size", "0")),
        ScopeArgumentMisuseCase("negative_depth", ("scope", "model:x", "--nearby-depth", "-1")),
        ScopeArgumentMisuseCase(
            "unqualified_explanation", ("scope", "model:x", "--explain", "normalize")
        ),
        ScopeArgumentMisuseCase(
            "browse_with_filter", ("scope", "model:x", "--browse", ".", "--kind", "macro")
        ),
        ScopeArgumentMisuseCase(
            "list_with_report_option",
            ("scope", "model:x", "--list", "global", "--include-nearby"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_scope_argument_misuse_when_parsing_then_returns_argparse_exit(
    test_case: ScopeArgumentMisuseCase,
) -> None:
    assert (
        _main_with_dependencies(argv=test_case.argv, handlers=build_handlers())
        == test_case.expected_exit_code
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
