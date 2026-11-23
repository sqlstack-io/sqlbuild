"""Tests for audit-factory filesystem discovery."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredPythonNodeFunctions,
    PythonNodeDiscoveryError,
    discover_python_node_functions,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    AuditFactoryDiscoveryTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        AuditFactoryDiscoveryTestCase(
            description="audit and node factories coexist",
            files={
                "python/nodes/factories/quality.py": """
from sqlbuild.audits import AuditCase, audit_factory
from sqlbuild.factories import factory
from sqlbuild.assets import asset

@audit_factory
def quality_checks():
    return [AuditCase(name="positive_amount", definition="expression_is_true", arguments={"expression": "amount > 0"})]

@factory
def generated_nodes():
    @asset(name="refresh_quality")
    def refresh(ctx):
        return None
    return [refresh]
""",
            },
            expected_factory_names=("quality_checks",),
            expected_task_names=("refresh_quality",),
        ),
        AuditFactoryDiscoveryTestCase(
            description="audit factory beside a direct task outside a factories folder",
            files={
                "python/nodes/quality.py": """
from sqlbuild.audits import AuditCase, audit_factory
from sqlbuild.assets import asset

@audit_factory
def quality_checks():
    return [AuditCase(name="positive_amount", definition="expression_is_true", arguments={"expression": "amount > 0"})]

@asset(name="refresh_quality")
def refresh(ctx):
    return None
""",
            },
            expected_factory_names=("quality_checks",),
            expected_task_names=("refresh_quality",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_and_node_factories_when_discovering_then_both_are_collected(
    test_case: AuditFactoryDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.files)

    discovered: DiscoveredPythonNodeFunctions = discover_python_node_functions(project_dir=tmp_path)

    assert tuple(factory.name for factory in discovered.audit_factories) == (
        test_case.expected_factory_names
    )
    assert discovered.audit_factories[0].cases[0].name == "positive_amount"
    assert discovered.audit_factories[0].relative_path == Path(next(iter(test_case.files)))
    assert discovered.audit_factories[0].line > 0
    assert tuple(task.name for task in discovered.assets) == test_case.expected_task_names


@pytest.mark.parametrize(
    "test_case",
    [
        AuditFactoryDiscoveryTestCase(
            "bad shape", {}, expected_error_fragment="must return a list or tuple"
        ),
        AuditFactoryDiscoveryTestCase(
            "bad item", {}, expected_error_fragment="item 0 that is not an AuditCase"
        ),
        AuditFactoryDiscoveryTestCase(
            "exception", {}, expected_error_fragment="failed during discovery: boom"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_audit_factory_when_discovering_then_clear_error_is_raised(
    test_case: AuditFactoryDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    body_by_description: dict[str, str] = {
        "bad shape": "return 'bad'",
        "bad item": "return [object()]",
        "exception": "raise RuntimeError('boom')",
    }
    write_repo_files(
        tmp_path,
        {
            "python/nodes/factories/quality.py": f"""
from sqlbuild.audits import audit_factory

@audit_factory
def broken_factory():
    {body_by_description[test_case.description]}
""",
        },
    )

    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        discover_python_node_functions(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        AuditFactoryDiscoveryTestCase(
            "duplicate names", {}, expected_error_fragment="Duplicate audit factory name"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_duplicate_audit_factory_names_when_discovering_then_error_is_raised(
    test_case: AuditFactoryDiscoveryTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    source = """
from sqlbuild.audits import audit_factory

@audit_factory
def duplicate_factory():
    return []
"""
    write_repo_files(
        tmp_path,
        {"python/nodes/factories/a.py": source, "python/nodes/factories/nested/b.py": source},
    )

    with pytest.raises(PythonNodeDiscoveryError, match=test_case.expected_error_fragment):
        discover_python_node_functions(project_dir=tmp_path)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
