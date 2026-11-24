"""Scope browse and list text presentation tests."""

from __future__ import annotations

from io import StringIO

import pytest

from sqlbuild._native import ScopeCommandRequest, render_scope_result, run_scope_command
from tests.unit.src.sqlbuild.cli.commands._helpers.scope._test_types import (
    ScopeColourCase,
    ScopeDetailLevelCase,
    ScopeOutputCase,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.scope.helpers import orders_move_report
from tests.unit.src.sqlbuild.compiler.scopes.helpers import report_scope_lookup

_BOLD: str = "\033[1m"
_DIM: str = "\033[2m"
_GREEN: str = "\033[32m"
_YELLOW: str = "\033[33m"
_RED: str = "\033[38;5;167m"
_RESET: str = "\033[0m"


@pytest.mark.parametrize(
    "test_case",
    (
        ScopeDetailLevelCase(
            description="default is compact with a legend and no empty optional sections",
            verbose=False,
            expected_fragments=(
                "  Status: 1 match\n",
                "  ├─ ● enum:mart_status  sql/models/marts/_enums/mart_status.sql:1\n",
                "  … 1 global collapsed; run sqb scope model:orders "
                "--as-path sql/models/marts/orders.sql --globals all\n",
                "  Lost (1)\n    └─ ● enum:order_status  sql/models/staging/enums/order_status.sql:1\n",
                "  Invalidated usages (1)\n    - enum:order_status\n",
                "● used by this resource   ○ not used by this resource   --verbose for scope details\n",
                "Completeness: complete\n",
            ),
            unexpected_fragments=(
                "[",
                ":1:1",
                "Private retained",
                "Nearby unavailable",
                "Diagnostics",
                "\033[",
            ),
        ),
        ScopeDetailLevelCase(
            description="verbose restores bracketed details, columns, and empty sections",
            verbose=True,
            expected_fragments=(
                "  Status: 1 match\n",
                "  ├─ ● enum:mart_status  [enum; exact-owner-private; expected_model through "
                "model:expected_orders; role sql/models/marts/_enums]  "
                "sql/models/marts/_enums/mart_status.sql:1:1\n",
                "Nearby unavailable (1 of 1)\n  (none)\n",
                "  Private retained (0)\n    (none)\n",
                "Diagnostics (0)\n  (none)\nCompleteness: complete\n",
            ),
            unexpected_fragments=("used by this resource", "\033["),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_move_preview_when_rendering_then_detail_level_follows_verbose(
    test_case: ScopeDetailLevelCase,
) -> None:
    stream: StringIO = StringIO()

    exit_code: int = run_scope_command(
        request=ScopeCommandRequest(
            target="model:orders",
            as_path="sql/models/marts/orders.sql",
            verbose=test_case.verbose,
        ),
        load_scope_index=lambda **_kwargs: report_scope_lookup().index,
        output_stream=stream,
    )

    assert exit_code == 0
    assert all(fragment in stream.getvalue() for fragment in test_case.expected_fragments), (
        stream.getvalue()
    )
    assert not any(fragment in stream.getvalue() for fragment in test_case.unexpected_fragments)


@pytest.mark.parametrize(
    "test_case",
    (
        ScopeColourCase(
            description="colour marks headers, used markers, losses, gains, and completeness",
            use_color=True,
            expected_fragments=(
                f"{_BOLD}Scope{_RESET}\n  {_DIM}Target:{_RESET} model:orders\n",
                f"{_BOLD}Used{_RESET} {_DIM}(3){_RESET}\n",
                f"  {_DIM}├─{_RESET} {_GREEN}●{_RESET} enum:mart_status  "
                f"{_DIM}sql/models/marts/_enums/mart_status.sql:1{_RESET}\n",
                f"{_BOLD}Available{_RESET} {_DIM}(2 of 3, 1 collapsed){_RESET}\n",
                f"    {_DIM}├─{_RESET} {_DIM}○{_RESET} constant:warehouse_password",
                f"    {_DIM}└─{_RESET} {_GREEN}●{_RESET} {_GREEN}enum:mart_status{_RESET}",
                f"  {_YELLOW}{_BOLD}Lost{_RESET} {_YELLOW}(1){_RESET}\n"
                f"    {_DIM}└─{_RESET} {_RED}●{_RESET} enum:order_status",
                f"  {_RED}{_BOLD}Invalidated usages{_RESET} {_RED}(1){_RESET}\n"
                f"    {_RED}- enum:order_status{_RESET}\n",
                f"{_DIM}Completeness:{_RESET} {_GREEN}complete{_RESET}\n",
            ),
            unexpected_fragments=(),
        ),
        ScopeColourCase(
            description="colour off renders the same report without ANSI sequences",
            use_color=False,
            expected_fragments=(
                "Used (3)\n",
                "  Lost (1)\n    └─ ● enum:order_status",
                "Completeness: complete\n",
            ),
            unexpected_fragments=("\033[",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_colour_setting_when_rendering_move_preview_then_applies_colour_roles(
    test_case: ScopeColourCase,
) -> None:
    rendered: str = render_scope_result(
        result=orders_move_report(),
        request=ScopeCommandRequest(target="model:orders", as_path="sql/models/marts/orders.sql"),
        use_color=test_case.use_color,
    )

    assert all(fragment in rendered for fragment in test_case.expected_fragments), rendered
    assert not any(fragment in rendered for fragment in test_case.unexpected_fragments)


@pytest.mark.parametrize(
    "test_case", (ScopeOutputCase("browse", 0),), ids=lambda case: case.description
)
def test_given_browse_result_when_rendering_then_counts_and_follow_ups_are_exact(
    test_case: ScopeOutputCase,
) -> None:
    stream: StringIO = StringIO()

    exit_code: int = run_scope_command(
        request=ScopeCommandRequest(target="model:orders", browse="global"),
        load_scope_index=lambda **_kwargs: report_scope_lookup().index,
        output_stream=stream,
    )

    assert exit_code == test_case.expected_exit_code
    assert "Scope folders" in stream.getvalue()
    assert "sql/constants/  1 declaration, 0 used, 0 children; constant 1" in stream.getvalue()
    assert "sql/macros/  1 declaration, 1 used, 1 child; macro 1" in stream.getvalue()
    assert "sqb scope model:orders --browse" in stream.getvalue()
    assert "sqb scope model:orders --list" in stream.getvalue()


@pytest.mark.parametrize(
    "test_case", (ScopeOutputCase("list", 0),), ids=lambda case: case.description
)
def test_given_paginated_list_when_rendering_then_continuation_repeats_filters(
    test_case: ScopeOutputCase,
) -> None:
    stream: StringIO = StringIO()
    request: ScopeCommandRequest = ScopeCommandRequest(
        target="model:orders",
        list_path="global/constants",
        kinds=("constant",),
        match="global_*",
        page_size=1,
        no_cache=True,
    )

    exit_code: int = run_scope_command(
        request=request,
        load_scope_index=lambda **_kwargs: report_scope_lookup(extra_globals=3).index,
        output_stream=stream,
    )

    assert exit_code == test_case.expected_exit_code
    assert (
        "Continue: sqb scope model:orders --list global/constants --kind constant "
        "--match 'global_*' --page-size 1 --after constant:global_00000 --no-cache"
        in stream.getvalue()
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
