"""Native semantic diagnostics survive the SQLBuild boundary and dialect gate."""

from __future__ import annotations

import json
from typing import Any

import pytest

from sqlbuild._native import BindingCatalog, SqlBindingResult
from tests.unit.src.sqlbuild.compiler.sql_analysis.main._test_types import (
    SemanticDiagnosticMappingCase,
)
from tests.unit.src.sqlbuild.compiler.sql_analysis.main.helpers import mapped_binding_result

DIALECTS: tuple[str, ...] = ("duckdb", "snowflake", "postgresql", "bigquery")


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticDiagnosticMappingCase(
            description="semantic, prepared type, and implicit-conversion diagnostics",
            expected_codes=(
                "B102",
                "B004",
                "B005",
                "B210",
                "B211",
                "B212",
                "B213",
                "B214",
                "B215",
                "B216",
                "B217",
                "B218",
                "B219",
                "B230",
                "B231",
                "B232",
                "B233",
                "B234",
                "B217",
                "B218",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_native_semantic_errors_when_validating_then_codes_and_locations_survive(
    test_case: SemanticDiagnosticMappingCase,
) -> None:
    codes: dict[str, str] = {"E203": "B102", "E222": "B004", "E223": "B005"}
    codes.update(
        {f"E{number}": f"B{number}" for number in (*range(210, 220), 230, 231, 232, 233, 234)}
    )
    codes.update({"W210": "B217", "W213": "B218"})
    rows: list[tuple[str, str, int | None, int | None, int | None, int | None, str]] = [
        (
            code,
            "Invalid expression",
            2,
            8,
            None,
            None,
            {True: "warning", False: "error"}[code.startswith("W")],
        )
        for code in codes
    ]
    payloads: list[dict[str, Any]] = [
        json.loads(
            BindingCatalog(
                dialect=dialect,
                quoted_ignore_case=False,
                known_functions=(),
                known_types=(),
                relations={},
            ).native.validation_payload([("SELECT\n  invalid", [], {})])
        )[0]
        for dialect in DIALECTS
    ]
    results: list[SqlBindingResult] = [
        mapped_binding_result(sql="SELECT\n  invalid", dialect=dialect, rows=rows)
        for dialect in DIALECTS
    ]

    assert all(payload["options"]["semantic"] for payload in payloads)
    assert all(payload["options"]["check_types"] for payload in payloads)
    for result in results:
        assert (
            tuple(diagnostic.code for diagnostic in result.diagnostics) == test_case.expected_codes
        )
        assert all(
            (diagnostic.line, diagnostic.column) == (2, 8) for diagnostic in result.diagnostics
        )
        assert all(diagnostic.severity == "error" for diagnostic in result.diagnostics)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
