"""Real compiler diagnostics, cache recovery, and SQL-test snippets."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
from _pytest.capture import CaptureResult

import sqlbuild._native as _native
from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    DiagnosticUxCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DiagnosticUxCase(
            "root typo",
            (("sql/models/marts/fact_orders.sql", "  o.quantity,", "  o.qty,"),),
            expected_fragments=(
                "Unknown column 'qty' in stg_orders (as o)",
                "^^^",
                "did you mean 'quantity'?",
                "stg_orders has: quantity",
                "2 downstream uses of fact_orders.quantity",
            ),
        ),
        DiagnosticUxCase(
            "both root errors",
            (
                ("sql/models/marts/fact_orders.sql", "  o.quantity,", "  o.qty,"),
                (
                    "sql/models/marts/fact_orders.sql",
                    "p ON o.order_id = p.order_id",
                    "p ON o.order_id = p.order_id\nWHERE o.ordered_at > 5",
                ),
            ),
            expected_errors=2,
            expected_fragments=(
                "between TIMESTAMP and INTEGER",
                "o.ordered_at is TIMESTAMP, 5 is INTEGER",
                "TIMESTAMP '2026-04-01'",
                "^^^^^^^^^^^^^^^^",
                "2 downstream uses",
            ),
        ),
        DiagnosticUxCase(
            "independent same input error",
            (
                ("sql/models/marts/fact_orders.sql", "  o.quantity,", "  o.qty,"),
                ("sql/models/marts/hourly_order_activity.sql", "SUM(o.quantity)", "SUM(o.missing)"),
            ),
            expected_errors=2,
            expected_fragments=(
                "Unknown column 'missing' in fact_orders (as o)",
                "1 downstream uses",
            ),
        ),
        DiagnosticUxCase(
            "independent closed input error",
            (("sql/models/marts/fact_orders.sql", "  o.quantity,", "  o.qty,"),),
            extra_files=(
                (
                    "sql/models/independent.sql",
                    'MODEL (description "Test model independent.", materialized view);\nSELECT w.missing FROM __seed("waffle_types") w CROSS JOIN __ref("fact_orders") o\n',
                ),
            ),
            expected_errors=2,
            expected_fragments=(
                "Unknown column 'missing' in waffle_types (as w)",
                "2 downstream uses",
            ),
        ),
        DiagnosticUxCase(
            "explicit output alias remains authoritative",
            (("sql/models/marts/fact_orders.sql", "  o.quantity,", "  o.qty AS qty,"),),
            expected_errors=3,
            expected_fragments=("Unknown column 'quantity' in fact_orders", "error[B302]"),
        ),
        DiagnosticUxCase(
            "predicate error does not poison another input projection",
            (),
            extra_files=(
                (
                    "sql/models/available.sql",
                    "MODEL (description 'Test model available.', materialized view); SELECT 1 AS qty",
                ),
                (
                    "sql/models/predicate_bad.sql",
                    'MODEL (description "Test model predicate_bad.", materialized view); SELECT a.qty FROM __ref("available") a CROSS JOIN __ref("stg_orders") o WHERE o.qty > 5',
                ),
                (
                    "sql/models/independent_use.sql",
                    'MODEL (description "Test model independent_use.", materialized view); SELECT quantity FROM __ref("predicate_bad")',
                ),
            ),
            expected_errors=2,
            expected_fragments=(
                "Unknown column 'qty' in stg_orders",
                "Unknown column 'quantity' in predicate_bad",
            ),
        ),
        DiagnosticUxCase(
            "open source detail",
            (("sql/models/marts/fact_orders.sql", "  o.quantity,", "  o.qty,"),),
            extra_files=(
                (
                    "sql/sources/optional.yml",
                    "sources:\n  - name: external_orders\n    description: Test source external_orders.\n    table: external_orders\n",
                ),
                (
                    "sql/models/open_read.sql",
                    'MODEL (description "Test model open_read.", materialized view);\nSELECT id FROM __source("external_orders")\n',
                ),
            ),
            expected_fragments=(
                "open_read: reads open source external_orders",
                "sqb contract generate --from <target> --select source:external_orders --write",
            ),
        ),
        DiagnosticUxCase(
            "SQL test snippet",
            (("tests/unit/test_fact_orders.sql", "quantity", "quantitty"),),
            expected_fragments=(
                "error[B302]",
                "sql test: test_fact_orders",
                "quantitty",
                "^^^^^^^^^",
            ),
            expected_partial=False,
        ),
        DiagnosticUxCase(
            "wide schema evidence is bounded",
            (),
            extra_files=(
                (
                    "sql/models/wide.sql",
                    "MODEL (description 'Test model wide.', materialized view);\nSELECT 1 AS quantity, "
                    + ", ".join(f"1 AS field_{index}" for index in range(20)),
                ),
                (
                    "sql/models/wide_bad.sql",
                    'MODEL (description "Test model wide_bad.", materialized view);\nSELECT w.qty FROM __ref("wide") w',
                ),
            ),
            expected_fragments=("wide has: quantity", "and 11 more", "did you mean 'quantity'?"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_semantic_failure_when_compiling_cold_and_warm_then_reports_root_evidence(
    test_case: DiagnosticUxCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project: Path = tmp_path / "orders"
    assert main(["playground", str(project)]) == 0
    capsys.readouterr()
    for relative, old, new in test_case.edits:
        path: Path = project / relative
        text: str = path.read_text()
        assert old in text
        path.write_text(text.replace(old, new, 1))
    for relative, text in test_case.extra_files:
        (project / relative).write_text(text)
    args: list[str] = ["--no-color", "--project-dir", str(project), "compile"]
    assert main([*args, "--json"]) == 1
    cold_output: CaptureResult[str] = capsys.readouterr()
    cold: dict[str, Any] = json.loads(cold_output.out)
    assert main([*args, "--json"]) == 1
    warm_output: CaptureResult[str] = capsys.readouterr()
    warm: dict[str, Any] = json.loads(warm_output.out)
    assert cold["summary"]["errors"] == test_case.expected_errors
    assert warm["diagnostics"] == cold["diagnostics"]
    assert warm["semantic_checks_partial"] == cold["semantic_checks_partial"]
    assert bool(cold["semantic_checks_partial"]) == test_case.expected_partial
    assert (
        cold_output.err.split("Semantic checks were partial")[-1]
        == warm_output.err.split("Semantic checks were partial")[-1]
        or not test_case.expected_partial
    )
    assert main(args) == 1
    human: str = capsys.readouterr().out
    assert all(fragment in human for fragment in test_case.expected_fragments), human
    assert "review the SQL expression, input types, and authoritative schema" not in human
    assert "resource: sql_test:" not in human
    assert "(context:" not in human


@pytest.mark.parametrize(
    "test_case",
    [DiagnosticUxCase("unique help", (), expected_errors=0)],
    ids=lambda case: case.description,
)
def test_given_semantic_codes_when_explaining_then_each_code_owns_distinct_help(
    test_case: DiagnosticUxCase,
) -> None:
    help_by_code: dict[str, str] = {
        **dict(_native.semantic_help_catalogue()),
        "B002": "did you mean 'quantity'?",
    }
    assert len(help_by_code) - len(set(help_by_code.values())) == test_case.expected_errors
    assert all(
        "review the SQL expression, input types, and authoritative schema" not in value
        for value in help_by_code.values()
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
