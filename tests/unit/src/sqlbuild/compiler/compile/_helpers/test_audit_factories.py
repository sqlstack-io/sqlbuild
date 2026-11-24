"""Tests for model audit-factory attachment."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild._native import (
    SchemaModelEntry,
    SourceLocation,
    build_compile_inputs,
    discover_project_inputs,
)
from sqlbuild.compiler.compile.exceptions import CompileInputError
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    AuditFactoryAttachmentTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
)
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

_PROJECT_FILE: str = 'name = "demo"\nadapter = "duckdb"\n'
_AUDIT_FILE: str = 'AUDIT (); SELECT * FROM __ref("@model") WHERE NOT (@expression)'


@pytest.mark.parametrize(
    "test_case",
    [
        AuditFactoryAttachmentTestCase(
            "provenance",
            "audit_factories [quality]",
            "",
            expected_audit_names=("positive_amount",),
            expected_warning_code="C216",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_factory_attachment_when_building_inputs_then_cases_append_with_provenance(
    test_case: AuditFactoryAttachmentTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/models/orders.sql": f"MODEL (description 'Test model orders.', {test_case.model_header}); SELECT 1 AS amount",
            "sql/audits/generic/expression_is_true.sql": _AUDIT_FILE,
            "python/nodes/factories/quality.py": """
from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory

@audit_factory
def quality():
    return [AuditCase(name="positive_amount", definition="expression_is_true", arguments={"expression": "amount > 0"}, severity=AuditSeverity.ERROR)]

@audit_factory
def unused_quality():
    return []
""",
        },
    )

    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        run_id="test_run",
    )

    schema_entry: SchemaModelEntry | None = inputs.model_inputs[0].schema_entry
    assert schema_entry is not None
    assert tuple(audit.name for audit in schema_entry.audits) == test_case.expected_audit_names
    location: SourceLocation | None = schema_entry.audits[0].location
    assert location is not None
    assert location.path == Path("python/nodes/factories/quality.py")
    assert location.line > 0
    assert "audit_factories" not in inputs.model_inputs[0].config.values
    assert test_case.expected_warning_code in tuple(
        diagnostic.code for diagnostic in inputs.diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AuditFactoryAttachmentTestCase(
            "unknown factory",
            "audit_factories [missing]",
            "return []",
            "unknown audit factory 'missing'",
        ),
        AuditFactoryAttachmentTestCase(
            "duplicate reference",
            "audit_factories [quality, quality]",
            "return []",
            "more than once",
        ),
        AuditFactoryAttachmentTestCase(
            "duplicate case",
            "audit_factories [quality]",
            'return [AuditCase(name="same", definition="expression_is_true"), AuditCase(name="same", definition="expression_is_true")]',
            "duplicate audit case name 'same'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_factory_attachment_when_building_inputs_then_compile_error_is_raised(
    test_case: AuditFactoryAttachmentTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/models/orders.sql": f"MODEL (description 'Test model orders.', {test_case.model_header}); SELECT 1 AS amount",
            **node_module_files(
                "python/nodes/factories/quality.py",
                f"""
from sqlbuild.audits import AuditCase, audit_factory

@audit_factory
def quality():
    {test_case.factory_cases}
""",
            ),
        },
    )

    with pytest.raises(CompileInputError, match=test_case.expected_error_fragment):
        build_compile_inputs(
            discovered_inputs=discover_project_inputs(project_dir=tmp_path),
            adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
            run_id="test_run",
        )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
