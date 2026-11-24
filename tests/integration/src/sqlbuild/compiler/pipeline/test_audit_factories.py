"""Integration coverage for generated audit attachments through real compilation."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    AuditFactoryCompileIntegrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    audit_input_projection,
    compile_audits_for_project,
)

if TYPE_CHECKING:
    from sqlbuild.compiler.compile.types import AuditInput


@pytest.mark.parametrize(
    "test_case",
    [AuditFactoryCompileIntegrationTestCase("direct attachment equivalence", 2)],
    ids=lambda case: case.description,
)
def test_given_generated_and_direct_attachments_when_compiled_then_audit_inputs_are_equivalent(
    test_case: AuditFactoryCompileIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "audit_factory"\nadapter = "duckdb"\n',
            "sql/audits/generic/expression_is_true.sql": """
AUDIT ();
SELECT * FROM __ref("@model") WHERE NOT (@expression)
""",
            "python/nodes/factories/quality.py": """
from sqlbuild.compiler.compile.types import AuditInput
from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory

@audit_factory
def order_quality():
    return [
        AuditCase(name="positive_amount", definition="expression_is_true", arguments={"expression": "amount > 0"}, severity=AuditSeverity.ERROR),
        AuditCase(name="bounded_amount", definition="expression_is_true", arguments={"expression": "amount < 1000"}, severity=AuditSeverity.WARN),
    ]
""",
            "sql/models/orders.sql": """
MODEL (description "Test model orders.", audit_factories [order_quality]);
SELECT 10 AS amount
""",
        },
    )
    generated: tuple[AuditInput, ...] = compile_audits_for_project(project_dir=tmp_path)

    write_repo_files(
        tmp_path,
        {
            "sql/models/orders.sql": """
MODEL (description "Test model orders.",
  audits [
    expression_is_true (name "positive_amount", expression "amount > 0", severity error),
    expression_is_true (name "bounded_amount", expression "amount < 1000", severity warn),
  ],
);
SELECT 10 AS amount
""",
        },
    )
    direct: tuple[AuditInput, ...] = compile_audits_for_project(project_dir=tmp_path)

    assert len(generated) == test_case.expected_audit_count
    assert audit_input_projection(audits=generated) == audit_input_projection(audits=direct)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
