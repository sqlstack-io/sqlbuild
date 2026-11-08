"""Fast and rich column lineage are built natively with the expected CLI output."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.fast_lineage._test_types import (
    NativeFastLineageCliTestCase,
    NativeRichLineageCliTestCase,
    NativeRichLineageFailureCliTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.fast_lineage.helpers import (
    FailedLineageRun,
    LineageRun,
    failed_rich_lineage_run,
    lineage_run,
    model_lineage_summaries,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders_lineage"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
    ),
    "sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Orders feed.\n    expression: >-\n"
        "      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount)\n"
        "    columns:\n      - name: order_id\n        type: INTEGER\n"
        "      - name: customer_id\n        type: INTEGER\n      - name: amount\n        type: DOUBLE\n"
    ),
    "models/staging/stg_orders.sql": (
        'MODEL (description "Staged orders");\n\nSELECT * FROM __source("raw_orders")\n'
    ),
    "models/marts/order_union.sql": (
        'MODEL (description "Orders twice", sql_analysis false);\n\n'
        'SELECT o.*, 1 AS copy_number FROM __ref("stg_orders") o\n'
        'UNION ALL\nSELECT *, 2 FROM __ref("stg_orders")\n'
    ),
    "models/marts/customer_totals.sql": (
        'MODEL (description "Totals per customer", sql_analysis false);\n\n'
        "WITH totals AS (\n"
        '  SELECT customer_id, SUM(amount) AS "Total_Amount" FROM __ref("order_union")\n'
        "  GROUP BY customer_id\n)\n"
        'SELECT T.customer_id, "Total_Amount", CAST("Total_Amount" AS BIGINT) AS rounded\n'
        "FROM totals AS T\n"
    ),
    "models/marts/order_copies.sql": (
        'MODEL (description "Order copies");\n\n'
        'SELECT u.*, c.rounded FROM __ref("order_union") u\n'
        'JOIN __ref("customer_totals") c ON u.customer_id = c.customer_id\n'
    ),
}

_DEEP_UNION_BRANCHES: int = 480
_DEEP_UNION_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"],
    "sources/raw.yml": _PROJECT_FILES["sources/raw.yml"],
    "models/marts/orders_union.sql": (
        'MODEL (description "Orders repeated", sql_analysis false);\n\n'
        + " UNION ALL ".join(
            ['SELECT order_id, amount FROM __source("raw_orders")'] * _DEEP_UNION_BRANCHES
        )
        + "\n"
    ),
    "models/marts/orders_second.sql": (
        'MODEL (description "Order ids", sql_analysis false);\n\n'
        'SELECT order_id FROM __source("raw_orders")\n'
    ),
}


_RICH_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"],
    "sources/raw.yml": _PROJECT_FILES["sources/raw.yml"],
    "seeds/regions.csv": "region_id,label\n1,north\n",
    "seeds/regions.yml": (
        "seeds:\n  - name: regions\n    description: Regions.\n    columns:\n"
        "      - name: region_id\n        type: INTEGER\n      - name: label\n        type: VARCHAR\n"
    ),
    "models/staging/stg_orders.sql": _PROJECT_FILES["models/staging/stg_orders.sql"],
    "models/marts/order_facts.sql": (
        'MODEL (description "Order facts");\n\n'
        "WITH ranked AS (\n"
        "  SELECT o.*, r.label, CAST(o.amount AS BIGINT) AS whole_amount\n"
        '  FROM __ref("stg_orders") AS o\n'
        '  LEFT JOIN __seed("regions") AS r ON o.customer_id = r.region_id\n'
        ")\n"
        "SELECT order_id, customer_id, COALESCE(label, 'none') AS region_label,\n"
        "  whole_amount, amount * 2 AS doubled, 'fixed' AS tag\n"
        "FROM ranked\n"
    ),
    "models/marts/customer_totals.sql": (
        'MODEL (description "Totals per customer");\n\n'
        "SELECT customer_id, SUM(doubled) AS total_doubled, COUNT(*) AS order_count,\n"
        "  MAX(region_label) AS region_label\n"
        'FROM __ref("order_facts")\nGROUP BY customer_id\n'
        "UNION ALL\n"
        'SELECT customer_id, amount, 1, NULL FROM __source("raw_orders")\n'
    ),
    "models/marts/customer_copies.sql": (
        'MODEL (description "Customer copies");\n\n'
        'SELECT t.*, f.tag FROM __ref("customer_totals") AS t\n'
        'JOIN __ref("order_facts") AS f USING (customer_id)\n'
    ),
}

_NATIVE_PANIC_ERROR: str = "NativeCompilerError: native SQL compilation panicked"
_RICH_PANIC_INJECTION: str = f"""
import sqlbuild._native as native


def _panicked(*arguments):
    raise native.NativeCompilerError({_NATIVE_PANIC_ERROR!r})


native.build_project_column_lineage = _panicked
"""


@pytest.mark.parametrize(
    "test_case",
    [
        NativeFastLineageCliTestCase(
            description="stars, opted-out unions, nested CTEs and quoted identifiers",
            files=_PROJECT_FILES,
            lineage_targets=(
                "customer_totals.Total_Amount",
                "order_union.amount",
                "stg_orders.amount",
            ),
            expected_lineage={
                "order_union": {
                    "available": True,
                    "column_count": 0,
                    "edge_count": 3,
                    "has_star": True,
                }
            },
            expected_minimum_traced_edges=4,
        ),
        NativeFastLineageCliTestCase(
            description="a 480-branch UNION ALL chain beside another opted-out model",
            files=_DEEP_UNION_FILES,
            lineage_targets=("orders_union.amount", "orders_second.order_id"),
            expected_lineage={
                "orders_union": {
                    "available": True,
                    "column_count": 0,
                    "edge_count": 2,
                    "has_star": False,
                },
                "orders_second": {
                    "available": True,
                    "column_count": 0,
                    "edge_count": 1,
                    "has_star": False,
                },
            },
            expected_minimum_traced_edges=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_when_compiling_then_fast_lineage_is_traced(
    test_case: NativeFastLineageCliTestCase, tmp_path: Path
) -> None:
    native: LineageRun = lineage_run(
        project_dir=tmp_path, files=test_case.files, targets=test_case.lineage_targets
    )
    summaries: dict[str, object] = model_lineage_summaries(native.compile_report)

    assert native.compile_returncode == 0, native.compile_report
    assert [code for code, _, _ in native.traces] == [0] * len(native.traces)
    assert {name: summaries[name] for name in test_case.expected_lineage} == (
        test_case.expected_lineage
    )
    assert sum('"source"' in stdout for _, stdout, _ in native.traces) >= (
        test_case.expected_minimum_traced_edges
    )


@pytest.mark.parametrize("warm", ["analysis cache", "compile reuse"])
def test_given_a_warm_store_when_tracing_fast_lineage_then_it_matches_the_uncached_run(
    warm: str, tmp_path: Path
) -> None:
    targets: tuple[str, ...] = (
        "customer_totals.Total_Amount",
        "order_copies.rounded",
        "stg_orders.amount",
    )
    uncached: LineageRun = lineage_run(
        project_dir=tmp_path / "uncached", files=_PROJECT_FILES, targets=targets
    )
    restored: LineageRun = lineage_run(
        project_dir=tmp_path / "warm", files=_PROJECT_FILES, targets=targets, warm=warm
    )

    assert restored.compile_returncode == 0, restored.compile_report
    assert (restored.compile_report, restored.traces) == (uncached.compile_report, uncached.traces)
    assert warm != "analysis cache" or restored.analysis_cache_hits > 0


@pytest.mark.parametrize(
    "test_case",
    [
        NativeRichLineageCliTestCase(
            description="stars, CTEs, joins, seeds, set operations and aggregates",
            files=_RICH_FILES,
            lineage_targets=(
                "customer_copies.total_doubled",
                "order_facts.whole_amount",
                "stg_orders.amount",
            ),
            expected_lineage={
                "customer_copies": {
                    "available": True,
                    "column_count": 5,
                    "edge_count": 8,
                    "has_star": True,
                },
                "customer_totals": {
                    "available": True,
                    "column_count": 4,
                    "edge_count": 5,
                    "has_star": False,
                },
                "order_facts": {
                    "available": True,
                    "column_count": 6,
                    "edge_count": 5,
                    "has_star": False,
                },
                "stg_orders": {
                    "available": True,
                    "column_count": 3,
                    "edge_count": 3,
                    "has_star": True,
                },
            },
            expected_minimum_traced_edges=4,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_when_tracing_rich_lineage_then_it_is_traced(
    test_case: NativeRichLineageCliTestCase, tmp_path: Path
) -> None:
    native: LineageRun = lineage_run(
        project_dir=tmp_path, files=test_case.files, targets=test_case.lineage_targets, mode="rich"
    )

    assert native.compile_returncode == 0, native.compile_report
    assert model_lineage_summaries(native.compile_report) == test_case.expected_lineage
    assert [code for code, _, _ in native.traces] == [0] * len(native.traces)
    assert sum('"source"' in stdout for _, stdout, _ in native.traces) >= (
        test_case.expected_minimum_traced_edges
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NativeRichLineageFailureCliTestCase(
            description="lineage --mode rich",
            files=_RICH_FILES,
            command=(
                "lineage",
                "order_facts.whole_amount",
                "--mode",
                "rich",
                "--json",
            ),
            expected_returncode=1,
            expected_error_line=f"_native.NativeCompilerError: {_NATIVE_PANIC_ERROR}",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_native_rich_lineage_panic_when_running_then_command_fails(
    test_case: NativeRichLineageFailureCliTestCase, tmp_path: Path
) -> None:
    run: FailedLineageRun = failed_rich_lineage_run(
        project_dir=tmp_path,
        files=test_case.files,
        args=test_case.command,
        perturbation=_RICH_PANIC_INJECTION,
    )

    assert run == FailedLineageRun(
        returncode=test_case.expected_returncode,
        last_error_line=test_case.expected_error_line,
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
