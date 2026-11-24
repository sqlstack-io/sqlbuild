"""Attached-audit ordering edges must not change where singular audits attach."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import CompilePipelineResult
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    SingularAuditAttachmentIntegrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    run_compile_pipeline_for_project,
)

_ORDER_CHECK: dict[str, str] = {
    "sql/audits/generic/order_check.sql": (
        'AUDIT ();\nSELECT o.* FROM @relation o LEFT JOIN __ref("valid_codes") v USING (code)\n'
    ),
}
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "gate_demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
    ),
    "sql/models/valid_codes.sql": "MODEL (description 'Test model valid_codes.', materialized table);\nSELECT 'A' AS code\n",
    "sql/audits/singular/reconcile.sql": (
        'AUDIT ();\nSELECT o.* FROM __ref("orders") o JOIN __ref("valid_codes") v USING (code)\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        SingularAuditAttachmentIntegrationTestCase(
            description="unrelated models without a gate edge run the singular audit at the end",
            orders_header="MODEL (description 'Test model.', materialized table);",
            generic_audit_files={},
            expected_attachment=("end", None),
        ),
        SingularAuditAttachmentIntegrationTestCase(
            description="an attached audit's gate edge does not move the singular audit",
            orders_header="MODEL (description 'Test model.', materialized table, audits [order_check]);",
            generic_audit_files=_ORDER_CHECK,
            expected_attachment=("end", None),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_gate_edge_when_planning_singular_audit_then_attachment_uses_lineage_only(
    test_case: SingularAuditAttachmentIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        _PROJECT_FILES
        | test_case.generic_audit_files
        | {"sql/models/orders.sql": f"{test_case.orders_header}\nSELECT 'A' AS code\n"},
    )

    result: CompilePipelineResult = run_compile_pipeline_for_project(
        project_dir=tmp_path, adapter=DuckDbAdapter()
    )

    attachments: dict[str, tuple[str, str | None]] = {
        entry.name: (entry.attachment_kind.value, entry.attached_target_name)
        for entry in result.plan_output.audit_entries
    }
    assert attachments["reconcile"] == test_case.expected_attachment
