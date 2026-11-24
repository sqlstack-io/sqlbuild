"""P010, P011 and C216 computed by the native compiled project over its attached resources."""

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project._test_types import (
    NativeResourceDiagnosticTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    cursor_model_files,
    resource_diagnostics_of,
)

_CURSOR_MESSAGE: str = (
    "incremental model 'daily_revenue' reads no inputs, so builds after the first "
    "cannot work out their cursor window"
)
_UDF_AFTER_MENTIONS: str = (
    '"""Mentions @udf before the decorator."""\n'
    "# @udf in a comment\n"
    "from sqlbuild.functions import udf\n\n\n"
    '@udf(arguments={"amount": "INTEGER"}, returns="BOOLEAN", runtime_version="3.11")\n'
    "def main(amount):\n"
    "    return amount > 100\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        NativeResourceDiagnosticTestCase(
            description="MODEL header after leading blank lines",
            files={
                "sql/models/order_totals.sql": "\n \t\n\n    MODEL (materialized table);\nSELECT 1 AS id\n"
            },
            expected_diagnostics=(
                (
                    "P010",
                    "model 'order_totals' has no description",
                    "sql/models/order_totals.sql",
                    4,
                    5,
                    None,
                ),
            ),
        ),
        NativeResourceDiagnosticTestCase(
            description="FUNCTION header after a leading blank line",
            files={
                "sql/functions/sql/is_large_order.sql": "\nFUNCTION (returns BOOLEAN);\n\nSELECT TRUE\n"
            },
            expected_diagnostics=(
                (
                    "P010",
                    "function 'is_large_order' has no description",
                    "sql/functions/sql/is_large_order.sql",
                    2,
                    1,
                    None,
                ),
            ),
        ),
        NativeResourceDiagnosticTestCase(
            description="HOOK header on the first line",
            files={"sql/hooks/sql/analyze_orders.sql": "HOOK ();\nSELECT 1\n"},
            expected_diagnostics=(
                (
                    "P010",
                    "hook 'analyze_orders' has no description",
                    "sql/hooks/sql/analyze_orders.sql",
                    1,
                    1,
                    None,
                ),
            ),
        ),
        NativeResourceDiagnosticTestCase(
            description="@udf decorator after docstring and comment mentions",
            files={"sql/functions/python/is_large_order.py": _UDF_AFTER_MENTIONS},
            expected_diagnostics=(
                (
                    "P010",
                    "function 'is_large_order' has no description",
                    "sql/functions/python/is_large_order.py",
                    6,
                    1,
                    None,
                ),
            ),
        ),
        NativeResourceDiagnosticTestCase(
            description="second seed entry of its YAML file",
            files={
                "sql/seeds/product_labels.csv": "product_type_id\n1\n",
                "sql/seeds/product_types.csv": "product_type_id\n1\n",
                "sql/seeds/product_types.yml": (
                    "seeds:\n  - name: product_labels\n    description: Labels\n    columns:\n"
                    "      - name: product_type_id\n        type: INTEGER\n"
                    "  - name: product_types\n    columns:\n"
                    "      - name: product_type_id\n        type: INTEGER\n"
                ),
            },
            expected_diagnostics=(
                (
                    "P010",
                    "seed 'product_types' has no description",
                    "sql/seeds/product_types.yml",
                    7,
                    1,
                    None,
                ),
            ),
        ),
        NativeResourceDiagnosticTestCase(
            description="cursor model reading only constants is located at its cursor entry",
            files=cursor_model_files(),
            expected_diagnostics=(
                ("P011", _CURSOR_MESSAGE, "sql/models/finance/daily_revenue.sql", 5, 3, 22),
            ),
        ),
        NativeResourceDiagnosticTestCase(
            description="cursor model reading a ref",
            files=cursor_model_files(read=' FROM __ref("payments")'),
            expected_diagnostics=(),
        ),
        NativeResourceDiagnosticTestCase(
            description="audit factory only one model attaches",
            files={
                "sql/models/orders.sql": (
                    'MODEL (description "Orders", audit_factories [quality]);\nSELECT 1 AS id\n'
                ),
                "python/nodes/factories/quality.py": (
                    "from sqlbuild.audits import audit_factory\n\n\n"
                    "@audit_factory\ndef quality():\n    return []\n\n\n"
                    "@audit_factory\ndef unused_quality():\n    return []\n"
                ),
            },
            expected_diagnostics=(
                (
                    "C216",
                    "Audit factory 'unused_quality' is not referenced by any model and will "
                    "not produce audits",
                    "python/nodes/factories/quality.py",
                    9,
                    1,
                    None,
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_resources_when_compiling_then_native_owner_reports_their_diagnostics(
    test_case: NativeResourceDiagnosticTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    diagnostics: tuple[tuple[object, ...], ...] = resource_diagnostics_of(
        project_dir=tmp_path, files=test_case.files, monkeypatch=monkeypatch
    )

    assert diagnostics == test_case.expected_diagnostics
