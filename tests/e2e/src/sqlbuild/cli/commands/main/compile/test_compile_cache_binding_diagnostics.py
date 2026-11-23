"""E2E coverage of binding diagnostics served from the compile analysis cache."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    CachedBindingDiagnosticCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import compile_with_binding_error
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project


@pytest.mark.parametrize(
    "test_case",
    (
        CachedBindingDiagnosticCase(
            description="unknown column before the reference placeholder",
            query_sql=(
                'SELECT\n  o.order_id,\n  o.quantiti AS quantity\nFROM __ref("orders") AS o\n'
            ),
            expected_location=("B002", 5, 5),
        ),
        CachedBindingDiagnosticCase(
            description="unknown column after the reference placeholder",
            query_sql=('SELECT\n  o.order_id\nFROM __ref("orders") AS o\nWHERE o.quantiti > 1\n'),
            expected_location=("B002", 6, 9),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_cached_model_with_binding_diagnostic_when_recompiling_then_diagnostic_is_identical(
    test_case: CachedBindingDiagnosticCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orders",
        repo_files={
            "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": (
                'MODEL (description "Orders");\n\nSELECT 1 AS order_id, 10 AS quantity\n'
            ),
            "sql/models/order_quantities.sql": (
                'MODEL (description "Order quantities");\n\n' + test_case.query_sql
            ),
        },
    )

    cold: dict[str, Any] = compile_with_binding_error(project_dir)
    # An upstream comment defeats whole-project reuse; the diagnosed model's analysis stays cached.
    upstream: Path = project_dir / "sql" / "models" / "orders.sql"
    upstream.write_text(
        upstream.read_text(encoding="utf-8") + "\n-- upstream comment\n", encoding="utf-8"
    )
    warm: dict[str, Any] = compile_with_binding_error(project_dir)

    assert warm["compile_timings"]["analysis_entry_cache_hits"] == 1
    assert warm["diagnostics"] == cold["diagnostics"]
    code, line, column = test_case.expected_location
    assert [(row["code"], row["line"], row["column"]) for row in warm["diagnostics"]] == [
        (code, line, column)
    ]


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
