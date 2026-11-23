"""Unit tests mapping real expanded SQL offsets back to authored positions."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    ExpansionSpan,
    LoadedMacro,
    MappedOffset,
    expand_authored_sql_with_spans,
    map_expanded_offset,
)
from sqlbuild.compiler.compile.models import MacroContext
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import ExpandWithSpansTestCase
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
    build_loaded_macros,
)

_MACRO_CONTEXT: MacroContext = MacroContext(
    adapter_name="duckdb",
    sql_analysis_enabled=False,
    target_name="dev",
    vars={},
)
_DOLLARS_MACRO: str = "def dollars(ctx, column):\n    return f'ROUND({column} / 100.0, 2)'\n"


@pytest.mark.parametrize(
    "test_case",
    [
        ExpandWithSpansTestCase(
            description="offset inside macro output is attributed to the call site",
            macro_file_contents=_DOLLARS_MACRO,
            sql="SELECT @py.dollars('cents') AS d FROM t",
            effective_vars={},
            expected_expanded_sql="SELECT ROUND(cents / 100.0, 2) AS d FROM t",
            probe_expanded_offset=15,
            expected_authored_offset=7,
            expected_generated=True,
        ),
        ExpandWithSpansTestCase(
            description="offset after macro output maps to the authored tail",
            macro_file_contents=_DOLLARS_MACRO,
            sql="SELECT @py.dollars('cents') AS d FROM t",
            expected_expanded_sql="SELECT ROUND(cents / 100.0, 2) AS d FROM t",
            effective_vars={},
            probe_expanded_offset=30,
            expected_authored_offset=27,
            expected_generated=False,
        ),
        ExpandWithSpansTestCase(
            description="offset before any expansion is unchanged",
            macro_file_contents=_DOLLARS_MACRO,
            sql="SELECT @py.dollars('cents') AS d FROM t",
            effective_vars={},
            expected_expanded_sql="SELECT ROUND(cents / 100.0, 2) AS d FROM t",
            probe_expanded_offset=0,
            expected_authored_offset=0,
            expected_generated=False,
        ),
        ExpandWithSpansTestCase(
            description="variable interpolation before a macro composes both deltas",
            macro_file_contents=_DOLLARS_MACRO,
            sql="SELECT @@col_name, @py.dollars('cents') AS d FROM t",
            effective_vars={"col_name": "customer_id"},
            expected_expanded_sql=("SELECT customer_id, ROUND(cents / 100.0, 2) AS d FROM t"),
            probe_expanded_offset=43,
            expected_authored_offset=39,
            expected_generated=False,
        ),
        ExpandWithSpansTestCase(
            description="offset inside interpolated variable text is attributed to the token",
            macro_file_contents=_DOLLARS_MACRO,
            sql="SELECT @@col_name, @py.dollars('cents') AS d FROM t",
            effective_vars={"col_name": "customer_id"},
            expected_expanded_sql=("SELECT customer_id, ROUND(cents / 100.0, 2) AS d FROM t"),
            probe_expanded_offset=12,
            expected_authored_offset=7,
            expected_generated=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_expanded_offset_when_mapping_then_authored_position_matches(
    test_case: ExpandWithSpansTestCase, tmp_path: Path
) -> None:
    loaded_macros: dict[str, LoadedMacro] = build_loaded_macros(
        tmp_path, test_case.macro_file_contents
    )
    expanded_sql: str
    passes: tuple[tuple[ExpansionSpan, ...], ...]
    expanded_sql, passes = expand_authored_sql_with_spans(
        sql=test_case.sql,
        file_path=tmp_path / "sql" / "models" / "orders.sql",
        effective_vars=test_case.effective_vars,
        loaded_macros=loaded_macros,
        macro_context=_MACRO_CONTEXT,
        value_renderer=DUCKDB_COMPILE_ADAPTER_CONTEXT.value_renderer,
        collection_rendering=DUCKDB_COMPILE_ADAPTER_CONTEXT.collection_rendering,
    )
    assert expanded_sql == test_case.expected_expanded_sql
    resolved: MappedOffset = map_expanded_offset(
        offset=test_case.probe_expanded_offset, passes=passes
    )
    assert resolved.offset == test_case.expected_authored_offset
    assert resolved.generated == test_case.expected_generated
